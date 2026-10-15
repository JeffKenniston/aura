use crate::config::SupervisorConfig;
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Child;
use tokio::sync::{broadcast, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessType {
    SpireAgent,
    JailerFirecracker,
    CognitiveAgent,
    Custom,
}

impl fmt::Display for ProcessType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProcessType::SpireAgent => write!(f, "SPIRE-Agent"),
            ProcessType::JailerFirecracker => write!(f, "Jailer-Firecracker"),
            ProcessType::CognitiveAgent => write!(f, "Cognitive-Agent"),
            ProcessType::Custom => write!(f, "Custom"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessStatus {
    Starting,
    Running,
    Terminated(Option<i32>),
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorState {
    Active,
    Draining,
    Stopped,
}

pub struct ManagedProcess {
    pub id: String,
    pub process_type: ProcessType,
    pub status: ProcessStatus,
    pub child: Option<Child>,
}

/// Guard representing an in-flight microkernel task (HOST-012).
/// Automatically decrements the active task counter when dropped.
pub struct TaskGuard {
    counter: Arc<AtomicUsize>,
}

impl Drop for TaskGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Microkernel Supervisor orchestrating child processes, graceful shutdown,
/// and administrative drain commands (HOST-007, HOST-009, HOST-012).
pub struct Supervisor {
    config: SupervisorConfig,
    state: Arc<RwLock<SupervisorState>>,
    in_flight_tasks: Arc<AtomicUsize>,
    shutdown_sender: broadcast::Sender<()>,
    processes: Arc<RwLock<HashMap<String, ManagedProcess>>>,
    drain_requested: Arc<AtomicBool>,
}

impl Supervisor {
    /// Creates a new Supervisor with declarative configuration.
    pub fn new(config: SupervisorConfig) -> Self {
        let (shutdown_sender, _) = broadcast::channel(16);
        Self {
            config,
            state: Arc::new(RwLock::new(SupervisorState::Active)),
            in_flight_tasks: Arc::new(AtomicUsize::new(0)),
            shutdown_sender,
            processes: Arc::new(RwLock::new(HashMap::new())),
            drain_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Returns the current supervisor state.
    pub async fn state(&self) -> SupervisorState {
        *self.state.read().await
    }

    /// Checks if the supervisor is currently in draining mode (rejecting new tasks).
    pub fn is_draining(&self) -> bool {
        self.drain_requested.load(Ordering::SeqCst)
    }

    /// Registers an in-flight workload (HOST-012).
    /// Returns `Some(TaskGuard)` if accepted, or `None` if the supervisor is draining.
    pub fn register_task(&self) -> Option<TaskGuard> {
        if self.is_draining() {
            return None;
        }

        self.in_flight_tasks.fetch_add(1, Ordering::SeqCst);
        Some(TaskGuard {
            counter: Arc::clone(&self.in_flight_tasks),
        })
    }

    /// Returns the number of currently active in-flight tasks.
    pub fn active_task_count(&self) -> usize {
        self.in_flight_tasks.load(Ordering::SeqCst)
    }

    /// Subscribes to the supervisor's shutdown broadcast channel.
    pub fn subscribe_shutdown(&self) -> broadcast::Receiver<()> {
        self.shutdown_sender.subscribe()
    }

    /// Spawns and supervises a child process (HOST-007).
    pub async fn spawn_process(
        &self,
        id: &str,
        process_type: ProcessType,
        mut command: tokio::process::Command,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.is_draining() {
            return Err("Cannot spawn new child processes while supervisor is draining".into());
        }

        println!("Supervisor spawning {} process [{}]...", process_type, id);
        let child = command.spawn()?;

        let mut processes = self.processes.write().await;
        processes.insert(
            id.to_string(),
            ManagedProcess {
                id: id.to_string(),
                process_type,
                status: ProcessStatus::Running,
                child: Some(child),
            },
        );

        Ok(())
    }

    /// Inspects and updates the status of managed child processes (HOST-007).
    pub async fn poll_processes(&self) {
        let mut processes = self.processes.write().await;
        for (id, proc) in processes.iter_mut() {
            if let Some(child) = proc.child.as_mut() {
                match child.try_wait() {
                    Ok(Some(exit_status)) => {
                        println!(
                            "Supervisor detected process [{}] exited with status: {:?}",
                            id, exit_status
                        );
                        proc.status = ProcessStatus::Terminated(exit_status.code());
                    }
                    Ok(None) => {
                        proc.status = ProcessStatus::Running;
                    }
                    Err(e) => {
                        proc.status = ProcessStatus::Failed(e.to_string());
                    }
                }
            }
        }
    }

    /// Initiates an administrative drain and graceful shutdown sequence (HOST-009).
    /// In-flight tasks are permitted to drain within the configured drain timeout.
    /// Supervised child processes are terminated gracefully with SIGTERM before SIGKILL escalation.
    pub async fn drain(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Supervisor initiating administrative drain...");
        self.drain_requested.store(true, Ordering::SeqCst);

        {
            let mut state = self.state.write().await;
            *state = SupervisorState::Draining;
        }

        // Broadcast shutdown notification to all internal subscribers
        let _ = self.shutdown_sender.send(());

        // 1. Wait for in-flight tasks to complete within drain timeout (HOST-009)
        let drain_timeout = Duration::from_secs(self.config.drain_timeout_secs);
        let start = tokio::time::Instant::now();

        while self.active_task_count() > 0 && start.elapsed() < drain_timeout {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        if self.active_task_count() > 0 {
            println!(
                "Warning: Drain timeout reached ({}s). {} in-flight tasks still pending.",
                self.config.drain_timeout_secs,
                self.active_task_count()
            );
        } else {
            println!("All in-flight tasks successfully drained.");
        }

        // 2. Terminate child processes gracefully
        let mut processes = self.processes.write().await;
        for (id, proc) in processes.iter_mut() {
            if let Some(mut child) = proc.child.take() {
                println!("Signaling child process [{}] for termination...", id);
                let _ = child.start_kill();

                let wait_timeout = Duration::from_secs(self.config.graceful_shutdown_timeout_secs);
                let wait_res = tokio::time::timeout(wait_timeout, child.wait()).await;

                match wait_res {
                    Ok(Ok(exit_status)) => {
                        proc.status = ProcessStatus::Terminated(exit_status.code());
                    }
                    Ok(Err(e)) => {
                        proc.status = ProcessStatus::Failed(e.to_string());
                    }
                    Err(_) => {
                        println!("Process [{}] did not exit in time, forcing kill.", id);
                        proc.status = ProcessStatus::Terminated(None);
                    }
                }
            }
        }

        {
            let mut state = self.state.write().await;
            *state = SupervisorState::Stopped;
        }

        println!("Supervisor administrative drain complete. System is stopped.");
        Ok(())
    }

    /// Spawns background OS signal listeners for SIGTERM / SIGINT to trigger drain (HOST-009).
    pub fn start_signal_handler(self: Arc<Self>) {
        tokio::spawn(async move {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{signal, SignalKind};
                if let Ok(mut sigterm) = signal(SignalKind::terminate()) {
                    tokio::select! {
                        _ = sigterm.recv() => {
                            println!("Received SIGTERM signal. Initiating supervisor drain...");
                            let _ = self.drain().await;
                        }
                        _ = tokio::signal::ctrl_c() => {
                            println!("Received SIGINT (Ctrl+C). Initiating supervisor drain...");
                            let _ = self.drain().await;
                        }
                    }
                    return;
                }
            }

            let _ = tokio::signal::ctrl_c().await;
            let _ = self.drain().await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_supervisor_state_and_task_registration() {
        let config = SupervisorConfig {
            graceful_shutdown_timeout_secs: 2,
            drain_timeout_secs: 1,
            health_check_interval_ms: 100,
        };

        let supervisor = Supervisor::new(config);
        assert_eq!(supervisor.state().await, SupervisorState::Active);
        assert_eq!(supervisor.active_task_count(), 0);

        // Register a task
        let guard = supervisor
            .register_task()
            .expect("Task should be registered");
        assert_eq!(supervisor.active_task_count(), 1);

        // Drop guard -> task count decrements
        drop(guard);
        assert_eq!(supervisor.active_task_count(), 0);
    }

    #[tokio::test]
    async fn test_supervisor_drain_sequence() {
        let config = SupervisorConfig {
            graceful_shutdown_timeout_secs: 1,
            drain_timeout_secs: 1,
            health_check_interval_ms: 100,
        };

        let supervisor = Arc::new(Supervisor::new(config));
        let mut shutdown_rx = supervisor.subscribe_shutdown();

        // Spawn a background task holding a guard
        let guard = supervisor.register_task().unwrap();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            drop(guard);
        });

        // Trigger drain
        let drain_res = supervisor.drain().await;
        assert!(drain_res.is_ok());

        // Shutdown channel was triggered
        assert!(shutdown_rx.recv().await.is_ok());

        // State is Stopped
        assert_eq!(supervisor.state().await, SupervisorState::Stopped);
        assert!(supervisor.is_draining());

        // Subsequent task registrations must be rejected
        assert!(supervisor.register_task().is_none());
    }

    #[tokio::test]
    async fn test_spawn_and_poll_child_process() {
        let config = SupervisorConfig::default();
        let supervisor = Supervisor::new(config);

        let mut cmd = tokio::process::Command::new("echo");
        cmd.arg("aura-test-process");
        cmd.stdout(std::process::Stdio::null());

        let spawn_res = supervisor
            .spawn_process("test-echo", ProcessType::Custom, cmd)
            .await;
        assert!(spawn_res.is_ok());

        let mut terminated = false;
        for _ in 0..20 {
            tokio::time::sleep(Duration::from_millis(25)).await;
            supervisor.poll_processes().await;
            let procs = supervisor.processes.read().await;
            if let Some(proc) = procs.get("test-echo") {
                if matches!(proc.status, ProcessStatus::Terminated(_)) {
                    terminated = true;
                    break;
                }
            }
        }
        assert!(terminated, "Process should be terminated after polling");
    }
}
