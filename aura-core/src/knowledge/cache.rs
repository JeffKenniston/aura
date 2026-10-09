use rusqlite::{ffi::sqlite3_auto_extension, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, Once};
use zenoh::prelude::r#async::*;
use zenoh::Session;

static INIT_VEC_ONCE: Once = Once::new();

/// Registers sqlite-vec as an auto-extension for all rusqlite connections.
#[allow(clippy::missing_transmute_annotations)]
pub fn register_sqlite_vec() {
    INIT_VEC_ONCE.call_once(|| unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    });
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub id: i64,
    pub tenant_id: String,
    pub prompt: String,
    pub prompt_hash: String,
    pub completion: String,
    pub embedding: Option<Vec<f32>>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheLookupRequest {
    #[serde(default = "default_id")]
    pub query_id: String,
    #[serde(default = "default_tenant")]
    pub tenant_id: String,
    pub prompt: String,
    pub prompt_hash: Option<String>,
    pub embedding: Option<Vec<f32>>,
    #[serde(default = "default_threshold")]
    pub threshold: f64,
    pub reply_topic: Option<String>,
    pub action: Option<String>,
    pub completion: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

fn default_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn default_tenant() -> String {
    "default".to_string()
}

fn default_threshold() -> f64 {
    0.85
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CacheLookupResponse {
    pub query_id: String,
    pub hit: bool,
    pub completion: Option<String>,
    pub similarity: f64,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStoreRequest {
    #[serde(default = "default_tenant")]
    pub tenant_id: String,
    pub prompt: String,
    pub completion: String,
    pub embedding: Option<Vec<f32>>,
    pub metadata: Option<serde_json::Value>,
}

/// Host-side SQLite vector database for semantic prompt caching (COG-013).
pub struct SemanticCache {
    conn: Arc<Mutex<Connection>>,
}

impl SemanticCache {
    /// Opens an in-memory SQLite database with sqlite-vec registered.
    pub fn open_in_memory() -> Result<Self, Box<dyn std::error::Error>> {
        register_sqlite_vec();
        let conn = Connection::open_in_memory()?;
        let cache = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        cache.init_schema()?;
        Ok(cache)
    }

    /// Opens a file-backed SQLite database with sqlite-vec registered.
    pub fn open_file<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        register_sqlite_vec();
        let conn = Connection::open(path)?;
        let cache = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        cache.init_schema()?;
        Ok(cache)
    }

    /// Returns the sqlite-vec extension version string.
    pub fn vec_version(&self) -> Result<String, Box<dyn std::error::Error>> {
        let conn = self.conn.lock().unwrap();
        let version: String = conn.query_row("SELECT vec_version()", [], |r| r.get(0))?;
        Ok(version)
    }

    /// Initializes tables and indexes for semantic prompt caching.
    pub fn init_schema(&self) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS semantic_cache (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tenant_id TEXT NOT NULL,
                prompt TEXT NOT NULL,
                prompt_hash TEXT NOT NULL,
                completion TEXT NOT NULL,
                embedding TEXT,
                metadata TEXT,
                created_at INTEGER NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_cache_tenant_hash ON semantic_cache(tenant_id, prompt_hash)",
            [],
        )?;
        Ok(())
    }

    /// Hashes prompt text using BLAKE3.
    pub fn hash_prompt(prompt: &str) -> String {
        blake3::hash(prompt.trim().as_bytes()).to_hex().to_string()
    }

    /// Stores a prompt and completion into the cache.
    pub fn put(
        &self,
        tenant_id: &str,
        prompt: &str,
        completion: &str,
        embedding: Option<&[f32]>,
        metadata: Option<&serde_json::Value>,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        let prompt_hash = Self::hash_prompt(prompt);
        let embedding_json = embedding.map(|emb| serde_json::to_string(emb).unwrap_or_default());
        let metadata_json = metadata.map(|m| serde_json::to_string(m).unwrap_or_default());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO semantic_cache (tenant_id, prompt, prompt_hash, completion, embedding, metadata, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                tenant_id,
                prompt,
                prompt_hash,
                completion,
                embedding_json,
                metadata_json,
                now as i64
            ],
        )?;
        let id = conn.last_insert_rowid();
        Ok(id)
    }

    /// Exact hash lookup.
    pub fn get_exact(
        &self,
        tenant_id: &str,
        prompt: &str,
    ) -> Result<Option<CacheLookupResponse>, Box<dyn std::error::Error>> {
        let hash = Self::hash_prompt(prompt);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT completion, metadata FROM semantic_cache WHERE tenant_id = ?1 AND prompt_hash = ?2 ORDER BY id DESC LIMIT 1"
        )?;
        let mut rows = stmt.query(rusqlite::params![tenant_id, hash])?;
        if let Some(row) = rows.next()? {
            let completion: String = row.get(0)?;
            let metadata_str: Option<String> = row.get(1)?;
            let metadata = metadata_str.and_then(|s| serde_json::from_str(&s).ok());
            return Ok(Some(CacheLookupResponse {
                query_id: String::new(),
                hit: true,
                completion: Some(completion),
                similarity: 1.0,
                metadata,
            }));
        }
        Ok(None)
    }

    /// Vector similarity search using sqlite-vec's vec_distance_cosine.
    pub fn search_semantic(
        &self,
        tenant_id: &str,
        query_embedding: &[f32],
        threshold: f64,
    ) -> Result<Option<CacheLookupResponse>, Box<dyn std::error::Error>> {
        let embedding_json = serde_json::to_string(query_embedding)?;
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT completion, metadata, vec_distance_cosine(embedding, ?1) as dist
             FROM semantic_cache
             WHERE tenant_id = ?2 AND embedding IS NOT NULL
             ORDER BY dist ASC LIMIT 1",
        )?;

        let mut rows = stmt.query(rusqlite::params![embedding_json, tenant_id])?;
        if let Some(row) = rows.next()? {
            let completion: String = row.get(0)?;
            let metadata_str: Option<String> = row.get(1)?;
            let dist: f64 = row.get(2)?;
            let similarity = 1.0 - dist;

            if similarity >= threshold {
                let metadata = metadata_str.and_then(|s| serde_json::from_str(&s).ok());
                return Ok(Some(CacheLookupResponse {
                    query_id: String::new(),
                    hit: true,
                    completion: Some(completion),
                    similarity,
                    metadata,
                }));
            }
        }
        Ok(None)
    }

    /// Evaluates a lookup request: first exact match, then vector similarity if embedding provided.
    pub fn lookup(&self, req: &CacheLookupRequest) -> CacheLookupResponse {
        // 1. Exact match attempt
        if let Ok(Some(mut resp)) = self.get_exact(&req.tenant_id, &req.prompt) {
            resp.query_id = req.query_id.clone();
            return resp;
        }

        // 2. Vector search if embedding provided
        if let Some(ref emb) = req.embedding {
            if let Ok(Some(mut resp)) = self.search_semantic(&req.tenant_id, emb, req.threshold) {
                resp.query_id = req.query_id.clone();
                return resp;
            }
        }

        // Cache miss
        CacheLookupResponse {
            query_id: req.query_id.clone(),
            hit: false,
            completion: None,
            similarity: 0.0,
            metadata: None,
        }
    }

    /// Starts listening on Zenoh topic(s) for cache lookup and store requests.
    pub async fn start_zenoh_listener(
        self: Arc<Self>,
        session: Arc<Session>,
        topic_pattern: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pattern = topic_pattern.unwrap_or("aura/**/cache/lookup").to_string();
        let subscriber = session
            .declare_subscriber(&pattern)
            .res_async()
            .await
            .map_err(|e| e.to_string())?;

        let cache = Arc::clone(&self);
        let session_clone = Arc::clone(&session);
        let pattern_display = pattern.clone();

        tokio::spawn(async move {
            println!(
                "Semantic cache listening on Zenoh topic: {}",
                pattern_display
            );
            while let Ok(sample) = subscriber.recv_async().await {
                let payload_bytes = sample.payload.contiguous();
                let payload_str = String::from_utf8_lossy(&payload_bytes);

                if let Ok(req) = serde_json::from_str::<CacheLookupRequest>(&payload_str) {
                    if req.action.as_deref() == Some("store") {
                        if let Some(ref comp) = req.completion {
                            let _ = cache.put(
                                &req.tenant_id,
                                &req.prompt,
                                comp,
                                req.embedding.as_deref(),
                                req.metadata.as_ref(),
                            );
                        }
                        continue;
                    }

                    let resp = cache.lookup(&req);

                    if let Ok(resp_json) = serde_json::to_string(&resp) {
                        let reply_topic = req
                            .reply_topic
                            .unwrap_or_else(|| format!("aura/{}/cache/result", req.tenant_id));
                        let _ = session_clone.put(reply_topic, resp_json).res_async().await;
                    }
                }
            }
        });

        // Also subscribe to store topic
        let store_subscriber = session
            .declare_subscriber("aura/**/cache/store")
            .res_async()
            .await
            .map_err(|e| e.to_string())?;

        let cache_store = Arc::clone(&self);
        tokio::spawn(async move {
            while let Ok(sample) = store_subscriber.recv_async().await {
                let payload_bytes = sample.payload.contiguous();
                let payload_str = String::from_utf8_lossy(&payload_bytes);
                if let Ok(store_req) = serde_json::from_str::<CacheStoreRequest>(&payload_str) {
                    let _ = cache_store.put(
                        &store_req.tenant_id,
                        &store_req.prompt,
                        &store_req.completion,
                        store_req.embedding.as_deref(),
                        store_req.metadata.as_ref(),
                    );
                }
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqlite_vec_version() {
        let cache = SemanticCache::open_in_memory().expect("Failed to open cache");
        let version = cache.vec_version().expect("Failed to get vec_version");
        assert!(
            !version.is_empty(),
            "sqlite-vec version should not be empty"
        );
    }

    #[test]
    fn test_exact_cache_lookup() {
        let cache = SemanticCache::open_in_memory().expect("Failed to open cache");
        cache
            .put(
                "tenant-1",
                "What is the airspeed velocity of an unladen swallow?",
                "African or European swallow?",
                None,
                None,
            )
            .expect("Put failed");

        let req = CacheLookupRequest {
            query_id: "q1".into(),
            tenant_id: "tenant-1".into(),
            prompt: "What is the airspeed velocity of an unladen swallow?".into(),
            prompt_hash: None,
            embedding: None,
            threshold: 0.85,
            reply_topic: None,
            action: None,
            completion: None,
            metadata: None,
        };

        let resp = cache.lookup(&req);
        assert!(resp.hit);
        assert_eq!(resp.completion.unwrap(), "African or European swallow?");
        assert_eq!(resp.similarity, 1.0);

        // Test miss on different prompt
        let req_miss = CacheLookupRequest {
            query_id: "q2".into(),
            tenant_id: "tenant-1".into(),
            prompt: "Something entirely different".into(),
            prompt_hash: None,
            embedding: None,
            threshold: 0.85,
            reply_topic: None,
            action: None,
            completion: None,
            metadata: None,
        };
        let resp_miss = cache.lookup(&req_miss);
        assert!(!resp_miss.hit);
    }

    #[test]
    fn test_vector_similarity_search() {
        let cache = SemanticCache::open_in_memory().expect("Failed to open cache");
        let emb1 = vec![1.0, 0.0, 0.0, 0.0];
        cache
            .put(
                "tenant-v",
                "Hello world prompt",
                "Hello world completion",
                Some(&emb1),
                None,
            )
            .expect("Put failed");

        // Query with very close embedding (cosine distance near 0, similarity near 1)
        let query_emb = vec![0.99, 0.01, 0.0, 0.0];
        let req = CacheLookupRequest {
            query_id: "q_vec".into(),
            tenant_id: "tenant-v".into(),
            prompt: "A syntactically different hello world".into(),
            prompt_hash: None,
            embedding: Some(query_emb),
            threshold: 0.90,
            reply_topic: None,
            action: None,
            completion: None,
            metadata: None,
        };

        let resp = cache.lookup(&req);
        assert!(resp.hit);
        assert_eq!(resp.completion.unwrap(), "Hello world completion");
        assert!(resp.similarity >= 0.90);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_zenoh_cache_lookup_flow() -> Result<(), Box<dyn std::error::Error>> {
        let cache = Arc::new(SemanticCache::open_in_memory()?);
        cache.put(
            "tenant-zenoh",
            "Cached query over Zenoh",
            "Cached response over Zenoh",
            None,
            None,
        )?;

        let zenoh_session = Arc::new(
            zenoh::open(zenoh::config::Config::default())
                .res_async()
                .await
                .map_err(|e| e.to_string())?,
        );

        cache
            .clone()
            .start_zenoh_listener(
                zenoh_session.clone(),
                Some("aura/tenant-zenoh/cache/lookup"),
            )
            .await?;

        let result_sub = zenoh_session
            .declare_subscriber("aura/tenant-zenoh/cache/result")
            .res_async()
            .await
            .map_err(|e| e.to_string())?;

        let req = CacheLookupRequest {
            query_id: "zenoh-q1".into(),
            tenant_id: "tenant-zenoh".into(),
            prompt: "Cached query over Zenoh".into(),
            prompt_hash: None,
            embedding: None,
            threshold: 0.85,
            reply_topic: Some("aura/tenant-zenoh/cache/result".into()),
            action: None,
            completion: None,
            metadata: None,
        };

        let req_json = serde_json::to_string(&req)?;
        zenoh_session
            .put("aura/tenant-zenoh/cache/lookup", req_json)
            .res_async()
            .await
            .map_err(|e| e.to_string())?;

        let sample = result_sub.recv_async().await?;
        let binding = sample.payload.contiguous();
        let resp: CacheLookupResponse = serde_json::from_slice(&binding)?;

        assert_eq!(resp.query_id, "zenoh-q1");
        assert!(resp.hit);
        assert_eq!(resp.completion.unwrap(), "Cached response over Zenoh");

        Ok(())
    }
}
