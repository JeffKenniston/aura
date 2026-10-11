pub mod registry;

use std::sync::RwLock;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::runtime::{Builder, Handle, Runtime, RuntimeMetrics};
use tokio::task::JoinHandle;

use crate::config::RuntimeConfig;

static RUNTIME_HANDLE: RwLock<Option<Handle>> = RwLock::new(None);

/// Returns the detected hardware parallelism for the host system (M0-SPIKE-03).
///
/// Falls back to 4 if hardware parallelism cannot be determined.
pub fn detect_hardware_parallelism() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

/// Initializes a dedicated multi-threaded Tokio runtime pool (M0-SPIKE-03, HOST-002).
///
/// If `worker_threads` is `None`, automatically detects available hardware parallelism.
/// Configures worker thread naming as `"aura-core-worker"`.
/// Enables all I/O and timer drivers and registers the runtime handle for health monitoring.
pub fn init_tokio_pool(worker_threads: Option<usize>) -> Result<Runtime, Box<dyn std::error::Error>> {
    let threads = worker_threads.unwrap_or_else(detect_hardware_parallelism);
    let mut config = RuntimeConfig::default();
    config.worker_threads = threads;
    init_tokio_pool_with_config(&config)
}

/// Initializes a dedicated multi-threaded Tokio runtime pool from declarative `RuntimeConfig` (HOST-002, HOST-008).
pub fn init_tokio_pool_with_config(config: &RuntimeConfig) -> Result<Runtime, Box<dyn std::error::Error>> {
    config.validate()?;

    let runtime = Builder::new_multi_thread()
        .worker_threads(config.worker_threads)
        .max_blocking_threads(config.max_blocking_threads)
        .thread_keep_alive(Duration::from_secs(config.thread_keep_alive_secs))
        .thread_name("aura-core-worker")
        .enable_all()
        .build()?;

    set_runtime_handle(runtime.handle().clone());
    println!(
        "Tokio multi-threaded runtime pool initialized ({} worker threads, max {} blocking threads).",
        config.worker_threads, config.max_blocking_threads
    );
    Ok(runtime)
}

/// Sets the active runtime handle for global metrics and supervisor monitoring.
pub fn set_runtime_handle(handle: Handle) {
    if let Ok(mut lock) = RUNTIME_HANDLE.write() {
        *lock = Some(handle);
    }
}

/// Retrieves the active runtime handle, checking the ambient Tokio context first,
/// then falling back to the registered global handle.
pub fn get_runtime_handle() -> Option<Handle> {
    Handle::try_current().ok().or_else(|| {
        RUNTIME_HANDLE
            .read()
            .ok()
            .and_then(|lock| lock.clone())
    })
}

/// Obtains a reference to the active runtime metrics (HOST-002).
///
/// Returns `Some(RuntimeMetrics)` if running within an active runtime context or if
/// a handle was registered via `init_tokio_pool` / `set_runtime_handle`, otherwise `None`.
pub fn metrics() -> Option<RuntimeMetrics> {
    get_runtime_handle().map(|h| h.metrics())
}

/// Spawns a blocking task onto the dedicated blocking thread pool (HOST-002).
///
/// Ensures compute-intensive or synchronous blocking operations (e.g., Wasmtime JIT compilation,
/// disk I/O, synchronous cryptography) do not block or starve latency-sensitive async worker threads.
pub fn spawn_blocking<F, R>(f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    tokio::task::spawn_blocking(f)
}

/// Offloads a CPU-bound or synchronous blocking computation (e.g., Cranelift JIT compilation,
/// BLAKE3 Merkle tree hashing, Firecracker Unix domain socket I/O, or Cedar policy parsing) to
/// Tokio's dedicated blocking thread pool (HOST-002).
///
/// This ensures Tokio async worker threads are never blocked on heavy computations or synchronous
/// syscalls, preserving worker availability for latency-sensitive tasks like QUIC stream
/// multiplexing and Zenoh event dispatch.
pub fn spawn_blocking_compute<F, R>(f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    tokio::task::spawn_blocking(f)
}

/// Executes a blocking or compute-heavy task on the blocking pool and awaits its
/// completion, mapping Tokio's `JoinError` into a standard thread-safe error.
pub async fn run_blocking_compute<F, R, E>(
    f: F,
) -> Result<R, Box<dyn std::error::Error + Send + Sync>>
where
    F: FnOnce() -> Result<R, E> + Send + 'static,
    R: Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    let handle = tokio::task::spawn_blocking(f);
    match handle.await {
        Ok(res) => res.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
        Err(join_err) => Err(Box::new(join_err) as Box<dyn std::error::Error + Send + Sync>),
    }
}

/// Comprehensive health and performance metrics snapshot of the Tokio runtime pool (HOST-002).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeHealthSnapshot {
    /// Number of worker threads configured and active.
    pub worker_threads: usize,
    /// Number of active asynchronous tasks currently alive.
    pub alive_tasks: usize,
    /// Depth of the runtime's global injection queue.
    pub global_queue_depth: usize,
    /// Depth of the blocking thread pool task queue.
    pub blocking_queue_depth: usize,
    /// Total number of blocking threads currently allocated.
    pub blocking_threads: usize,
    /// Number of idle threads in the blocking pool.
    pub idle_blocking_threads: usize,
    /// Number of worker threads currently actively executing tasks.
    pub active_workers: usize,
    /// Number of worker threads currently parked waiting for work.
    pub parked_workers: usize,
    /// Total number of tasks stolen across all worker threads.
    pub total_steal_count: u64,
    /// Total number of steal operations performed.
    pub total_steal_operations: u64,
    /// Cumulative busy duration across all worker threads in milliseconds.
    pub total_busy_duration_ms: u64,
    /// Total number of tasks spawned in this runtime since creation.
    pub spawned_tasks_count: u64,
    /// Per-worker thread health and utilization metrics.
    pub workers: Vec<WorkerHealthSnapshot>,
}

impl RuntimeHealthSnapshot {
    /// Returns true if the runtime is operating under healthy load thresholds:
    /// - At least one worker thread is configured
    /// - Global injection queue depth is below 10,000 tasks
    pub fn is_healthy(&self) -> bool {
        self.worker_threads > 0 && self.global_queue_depth < 10_000
    }

    /// Average busy duration per worker thread in milliseconds.
    pub fn average_busy_duration_ms(&self) -> f64 {
        if self.worker_threads == 0 {
            0.0
        } else {
            self.total_busy_duration_ms as f64 / self.worker_threads as f64
        }
    }
}

/// Utilization snapshot for an individual worker thread (HOST-002).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerHealthSnapshot {
    /// Worker thread index (0 .. worker_threads).
    pub worker_index: usize,
    /// True if the worker is currently executing a task, false if parked.
    pub is_active: bool,
    /// Total busy duration for this worker in milliseconds.
    pub busy_duration_ms: u64,
    /// Total times this worker has parked.
    pub park_count: u64,
    /// Total tasks stolen from other workers' local queues.
    pub steal_count: u64,
    /// Current depth of this worker's local run queue.
    pub local_queue_depth: usize,
}

/// Collects an immediate snapshot of runtime health and thread pool metrics (HOST-002).
pub fn health_snapshot() -> Option<RuntimeHealthSnapshot> {
    let m = metrics()?;
    let num_workers = m.num_workers();
    let mut workers = Vec::with_capacity(num_workers);
    let mut active_workers = 0;
    let mut parked_workers = 0;
    let mut total_steal_count = 0;
    let mut total_steal_operations = 0;
    let mut total_busy_duration_ms = 0;

    for i in 0..num_workers {
        let park_unpark = m.worker_park_unpark_count(i);
        let is_active = park_unpark % 2 == 0;
        if is_active {
            active_workers += 1;
        } else {
            parked_workers += 1;
        }

        let busy_ms = m.worker_total_busy_duration(i).as_millis() as u64;
        total_busy_duration_ms += busy_ms;

        let steal_count = m.worker_steal_count(i);
        total_steal_count += steal_count;

        let steal_ops = m.worker_steal_operations(i);
        total_steal_operations += steal_ops;

        #[cfg(tokio_unstable)]
        let local_queue_depth = m.worker_local_queue_depth(i);
        #[cfg(not(tokio_unstable))]
        let local_queue_depth = 0;

        workers.push(WorkerHealthSnapshot {
            worker_index: i,
            is_active,
            busy_duration_ms: busy_ms,
            park_count: m.worker_park_count(i),
            steal_count,
            local_queue_depth,
        });
    }

    #[cfg(tokio_unstable)]
    let blocking_queue_depth = m.blocking_queue_depth();
    #[cfg(not(tokio_unstable))]
    let blocking_queue_depth = 0;

    #[cfg(tokio_unstable)]
    let blocking_threads = m.num_blocking_threads();
    #[cfg(not(tokio_unstable))]
    let blocking_threads = 0;

    #[cfg(tokio_unstable)]
    let idle_blocking_threads = m.num_idle_blocking_threads();
    #[cfg(not(tokio_unstable))]
    let idle_blocking_threads = 0;

    #[cfg(tokio_unstable)]
    let spawned_tasks_count = m.spawned_tasks_count();
    #[cfg(not(tokio_unstable))]
    let spawned_tasks_count = 0;

    Some(RuntimeHealthSnapshot {
        worker_threads: num_workers,
        alive_tasks: m.num_alive_tasks(),
        global_queue_depth: m.global_queue_depth(),
        blocking_queue_depth,
        blocking_threads,
        idle_blocking_threads,
        active_workers,
        parked_workers,
        total_steal_count,
        total_steal_operations,
        total_busy_duration_ms,
        spawned_tasks_count,
        workers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_hardware_parallelism_detection() {
        let cores = detect_hardware_parallelism();
        assert!(cores >= 1, "Hardware parallelism should be at least 1");
    }

    #[test]
    fn test_init_tokio_pool_default() {
        let rt = init_tokio_pool(None).expect("Failed to create default tokio pool");
        let workers = rt.metrics().num_workers();
        assert!(workers >= 1, "Should have at least 1 worker thread");
        assert_eq!(workers, detect_hardware_parallelism());
    }

    #[test]
    fn test_init_tokio_pool_custom_workers() {
        let rt = init_tokio_pool(Some(3)).expect("Failed to create custom tokio pool");
        assert_eq!(rt.metrics().num_workers(), 3);
    }

    #[test]
    fn test_worker_thread_naming() {
        let rt = init_tokio_pool(Some(2)).expect("Failed to create tokio pool");
        let handle = rt.spawn(async {
            std::thread::current().name().unwrap_or("").to_string()
        });
        let name = rt.block_on(handle).expect("Failed to join task");
        assert_eq!(name, "aura-core-worker");
    }

    #[test]
    fn test_worker_thread_distribution() {
        let rt = init_tokio_pool(Some(4)).expect("Failed to create tokio pool");
        let thread_ids = Arc::new(Mutex::new(HashSet::new()));

        rt.block_on(async {
            let mut handles = Vec::new();
            for _ in 0..20 {
                let ids = thread_ids.clone();
                handles.push(tokio::spawn(async move {
                    let id = std::thread::current().id();
                    ids.lock().unwrap().insert(id);
                    let start = std::time::Instant::now();
                    while start.elapsed() < Duration::from_millis(15) {
                        std::hint::spin_loop();
                    }
                }));
            }
            for handle in handles {
                handle.await.unwrap();
            }
        });

        let count = thread_ids.lock().unwrap().len();
        assert!(
            count > 1,
            "Expected tasks to execute across multiple worker threads, got {}",
            count
        );
    }

    #[test]
    fn test_runtime_health_snapshot() {
        let rt = init_tokio_pool(Some(2)).expect("Failed to create tokio pool");
        rt.block_on(async {
            let snapshot = health_snapshot().expect("Expected health snapshot");
            assert_eq!(snapshot.worker_threads, 2);
            assert!(snapshot.is_healthy());
            assert_eq!(snapshot.workers.len(), 2);
            assert_eq!(snapshot.workers[0].worker_index, 0);
            assert_eq!(snapshot.workers[1].worker_index, 1);
        });
    }

    #[test]
    fn test_blocking_thread_isolation() {
        let rt = init_tokio_pool(Some(2)).expect("Failed to create tokio pool");
        rt.block_on(async {
            let blocking_handle = spawn_blocking(|| {
                std::thread::sleep(Duration::from_millis(50));
                42
            });

            let start = tokio::time::Instant::now();
            tokio::time::sleep(Duration::from_millis(10)).await;
            let elapsed = start.elapsed();
            assert!(elapsed >= Duration::from_millis(10));
            assert!(
                elapsed < Duration::from_millis(45),
                "Async timer sleep was delayed by blocking task: {:?}",
                elapsed
            );

            let result = blocking_handle.await.expect("Blocking task panicked");
            assert_eq!(result, 42);
        });
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_timer_preservation_under_heavy_blocking_load() {
        let tick_count = Arc::new(AtomicUsize::new(0));
        let tick_count_clone = Arc::clone(&tick_count);

        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_signal_clone = Arc::clone(&stop_signal);

        let timer_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(15));
            while !stop_signal_clone.load(Ordering::Relaxed) {
                interval.tick().await;
                tick_count_clone.fetch_add(1, Ordering::Relaxed);
            }
        });

        let mut compute_handles = Vec::new();
        for _ in 0..8 {
            let handle = spawn_blocking_compute(|| {
                let start = std::time::Instant::now();
                let mut hasher = blake3::Hasher::new();
                while start.elapsed() < Duration::from_millis(150) {
                    hasher.update(b"aura-microkernel-heavy-compute-test-data-block");
                }
                hasher.finalize()
            });
            compute_handles.push(handle);
        }

        for handle in compute_handles {
            let res = handle.await.expect("blocking compute task must succeed");
            assert_eq!(res.as_bytes().len(), 32);
        }

        stop_signal.store(true, Ordering::Relaxed);
        let _ = timer_handle.await;

        let final_ticks = tick_count.load(Ordering::Relaxed);
        assert!(
            final_ticks >= 5,
            "Timer was starved by compute load! Expected >= 5 ticks, but only got {}",
            final_ticks
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_channel_dispatch_latency_under_compute_load() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<u32>(100);

        let mut compute_handles = Vec::new();
        for i in 0..6 {
            compute_handles.push(spawn_blocking_compute(move || {
                let start = std::time::Instant::now();
                let mut acc = 0u64;
                while start.elapsed() < Duration::from_millis(100) {
                    acc = acc.wrapping_add(i as u64);
                }
                acc
            }));
        }

        let dispatch_handle = tokio::spawn(async move {
            let mut latencies = Vec::new();
            for msg in 0..10 {
                let t0 = std::time::Instant::now();
                tx.send(msg).await.unwrap();
                let elapsed = t0.elapsed();
                latencies.push(elapsed);
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            latencies
        });

        let receiver_handle = tokio::spawn(async move {
            let mut count = 0;
            while let Some(_msg) = rx.recv().await {
                count += 1;
                if count == 10 {
                    break;
                }
            }
            count
        });

        let latencies = dispatch_handle.await.unwrap();
        let count = receiver_handle.await.unwrap();
        assert_eq!(count, 10);

        for h in compute_handles {
            let _ = h.await.unwrap();
        }

        for (idx, lat) in latencies.iter().enumerate() {
            assert!(
                *lat < Duration::from_millis(25),
                "Message {} dispatch was stalled: took {:?}",
                idx,
                lat
            );
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_wasmtime_compilation_offload_preserves_timer() {
        let tick_count = Arc::new(AtomicUsize::new(0));
        let tick_count_clone = Arc::clone(&tick_count);

        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop);

        let timer = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(10));
            while !stop_clone.load(Ordering::Relaxed) {
                interval.tick().await;
                tick_count_clone.fetch_add(1, Ordering::Relaxed);
            }
        });

        let compile_handle = spawn_blocking_compute(|| {
            let engine = wasmtime::Engine::default();
            let wat_src = r#"
                (module
                    (func $add (param $a i32) (param $b i32) (result i32)
                        local.get $a
                        local.get $b
                        i32.add
                    )
                    (export "add" (func $add))
                )
            "#;
            for _ in 0..50 {
                let _ = wasmtime::Module::new(&engine, wat_src).unwrap();
            }
            true
        });

        let res = compile_handle.await.unwrap();
        assert!(res);

        stop.store(true, Ordering::Relaxed);
        let _ = timer.await;

        let ticks = tick_count.load(Ordering::Relaxed);
        assert!(
            ticks >= 2,
            "Timer ticks ({}) were starved during Wasmtime JIT compilation",
            ticks
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_fair_work_distribution_across_workers() {
        let thread_ids = Arc::new(Mutex::new(HashSet::new()));
        let mut handles = Vec::new();

        for _ in 0..20 {
            let thread_ids_clone = Arc::clone(&thread_ids);
            handles.push(tokio::spawn(async move {
                let current_id = std::thread::current().id();
                thread_ids_clone.lock().unwrap().insert(current_id);
                let start = std::time::Instant::now();
                while start.elapsed() < Duration::from_millis(15) {
                    std::hint::spin_loop();
                }
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        let distinct_threads = thread_ids.lock().unwrap().len();
        assert!(
            distinct_threads >= 2,
            "Work was not distributed across worker threads: only {} distinct threads observed",
            distinct_threads
        );
    }

    #[tokio::test]
    async fn test_run_blocking_compute_result_propagation() {
        let ok_res = run_blocking_compute(|| Ok::<u32, std::io::Error>(42)).await;
        assert_eq!(ok_res.unwrap(), 42);

        let err_res = run_blocking_compute(|| {
            Err::<u32, std::io::Error>(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "file not found",
            ))
        })
        .await;
        assert!(err_res.is_err());
    }
}
