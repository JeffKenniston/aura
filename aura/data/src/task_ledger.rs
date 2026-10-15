//! Task Lifecycle Ledger backed by EncryptedDatabase (Feature 16)

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use super::encrypted::{derive_storage_key, EncryptedDatabase, StorageError};
use aura_iam::Svid;

/// Possible lifecycle states for an Aura task execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Pending,
    Submitted,
    Dispatched,
    Running,
    Completed,
    Failed,
    Cancelled,
    Custom(String),
}

impl TaskState {
    pub fn as_str(&self) -> &str {
        match self {
            TaskState::Pending => "pending",
            TaskState::Submitted => "submitted",
            TaskState::Dispatched => "dispatched",
            TaskState::Running => "running",
            TaskState::Completed => "completed",
            TaskState::Failed => "failed",
            TaskState::Cancelled => "cancelled",
            TaskState::Custom(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for TaskState {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "pending" => TaskState::Pending,
            "submitted" => TaskState::Submitted,
            "dispatched" => TaskState::Dispatched,
            "running" => TaskState::Running,
            "completed" => TaskState::Completed,
            "failed" => TaskState::Failed,
            "cancelled" => TaskState::Cancelled,
            other => TaskState::Custom(other.to_string()),
        })
    }
}

/// An immutable event record capturing a state transition or milestone in a task's lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEvent {
    pub task_id: String,
    pub state: TaskState,
    pub timestamp: u64,
    pub metadata: Option<String>,
    pub transition_reason: Option<String>,
}

impl TaskEvent {
    /// Creates a new TaskEvent with required fields.
    pub fn new(task_id: impl Into<String>, state: TaskState, timestamp: u64) -> Self {
        Self {
            task_id: task_id.into(),
            state,
            timestamp,
            metadata: None,
            transition_reason: None,
        }
    }

    /// Sets raw string metadata on the event.
    pub fn with_metadata(mut self, metadata: impl Into<String>) -> Self {
        self.metadata = Some(metadata.into());
        self
    }

    /// Sets JSON-serialized metadata on the event.
    pub fn with_json_metadata<T: Serialize>(mut self, val: &T) -> Self {
        self.metadata = serde_json::to_string(val).ok();
        self
    }

    /// Sets the transition reason for this event.
    pub fn with_transition_reason(mut self, reason: impl Into<String>) -> Self {
        self.transition_reason = Some(reason.into());
        self
    }
}

/// Encrypted append-only ledger for task lifecycle events.
/// Persisted inside an authenticated `EncryptedDatabase`.
pub struct TaskLedger {
    db: Arc<EncryptedDatabase>,
}

impl std::fmt::Debug for TaskLedger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskLedger")
            .field("db", &self.db)
            .finish()
    }
}

impl TaskLedger {
    /// Initializes a `TaskLedger` backed by an `EncryptedDatabase`, ensuring schema existence.
    pub fn new(db: Arc<EncryptedDatabase>) -> Result<Self, StorageError> {
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS task_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT NOT NULL,
                state TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                metadata TEXT,
                transition_reason TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_task_events_task_id ON task_events(task_id);",
        )?;
        Ok(Self { db })
    }

    /// Opens or initializes an encrypted task ledger on disk with the provided 256-bit key.
    pub fn open<P: AsRef<Path>>(path: P, key: [u8; 32]) -> Result<Self, StorageError> {
        let db = EncryptedDatabase::open(path, key)?;
        Self::new(Arc::new(db))
    }

    /// Opens an encrypted task ledger on disk locked to an attested SPIFFE SVID.
    pub fn open_with_svid<P: AsRef<Path>>(path: P, svid: &Svid) -> Result<Self, StorageError> {
        let key = derive_storage_key(svid);
        Self::open(path, key)
    }

    /// Creates an in-memory encrypted task ledger.
    pub fn in_memory(key: [u8; 32]) -> Result<Self, StorageError> {
        let db = EncryptedDatabase::open_in_memory(key)?;
        Self::new(Arc::new(db))
    }

    /// Appends a new lifecycle transition event and immediately flushes ciphertext to disk.
    pub fn append_event(&self, event: &TaskEvent) -> Result<(), StorageError> {
        self.db.execute(
            "INSERT INTO task_events (task_id, state, timestamp, metadata, transition_reason)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                &event.task_id,
                event.state.as_str(),
                event.timestamp as i64,
                &event.metadata,
                &event.transition_reason,
            ],
        )?;

        self.db.flush()?;
        Ok(())
    }

    /// Queries all lifecycle transition events for a given task ID ordered chronologically by `id`.
    pub fn get_events(&self, task_id: &str) -> Result<Vec<TaskEvent>, StorageError> {
        self.db.query(
            "SELECT task_id, state, timestamp, metadata, transition_reason
             FROM task_events
             WHERE task_id = ?1
             ORDER BY id ASC",
            rusqlite::params![task_id],
            |row| {
                let task_id: String = row.get(0)?;
                let state_str: String = row.get(1)?;
                let timestamp_i64: i64 = row.get(2)?;
                let metadata: Option<String> = row.get(3)?;
                let transition_reason: Option<String> = row.get(4)?;

                let state = TaskState::from_str(&state_str).unwrap_or(TaskState::Custom(state_str));

                Ok(TaskEvent {
                    task_id,
                    state,
                    timestamp: timestamp_i64 as u64,
                    metadata,
                    transition_reason,
                })
            },
        )
    }

    /// Flushes the underlying encrypted database to disk.
    pub fn flush(&self) -> Result<(), StorageError> {
        self.db.flush()
    }

    /// Returns a reference to the backing `EncryptedDatabase`.
    pub fn db(&self) -> &Arc<EncryptedDatabase> {
        &self.db
    }
}
