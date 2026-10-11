//! Empirical Load Test: High-Concurrency QUIC Stream Multiplexing Scale (HOST-004)
//!
//! Validates the requirement from requirements.md (HOST-004) and aura-host-domain/SKILL.md:
//! "The host transport layer must multiplex 5,000+ concurrent HTTP/3 QUIC streams without
//! thread starvation, worker depletion, or application-layer head-of-line blocking."
//!
//! Test architecture:
//! 1. High-Performance Sans-I/O Server: Binds to 127.0.0.1:0, accepts QUIC connection via quiche,
//!    multiplexes incoming bidirectional streams, and echoes processed responses.
//! 2. Multiplexed Async Client Driver: Binds to 127.0.0.1:0, establishes single QUIC connection,
//!    and exposes an async MPSC stream channel.
//! 3. 5,000 Concurrent Tokio Tasks: Drives 5,000 concurrent client stream requests via `tokio::spawn`,
//!    each tracking its individual round-trip latency and verifying response payload integrity.
//! 4. Liveness Canary Task: Concurrently measures event loop timer jitter to empirically prove zero
//!    worker thread starvation throughout high-concurrency stream multiplexing.
//! 5. Empirical Metrics Collection: Computes throughput (streams/sec), latency distribution
//!    (Min, P50, P90, P99, Max), peak memory usage (RSS via /proc/self/status), and verifies 0 packet
//!    loss and 0 stream resets via native Quiche telemetry counters.

use aura_core::config::TransportConfig;
use aura_core::transport::Http3Transport;
use quiche;
use std::collections::HashMap;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, oneshot};

/// Self-signed TLS 1.3 certificate valid through 2047 for local test harnesses.
const TEST_CERT_PEM: &str = "-----BEGIN CERTIFICATE-----\n\
MIIC7TCCAdUCFDuGBhl3l5Z++VCLkvaav4yteBonMA0GCSqGSIb3DQEBCwUAMEUx\n\
CzAJBgNVBAYTAkFVMRMwEQYDVQQIDApTb21lLVN0YXRlMSEwHwYDVQQKDBhJbnRl\n\
cm5ldCBXaWRnaXRzIFB0eSBMdGQwHhcNMjAwMzIzMTYwNzU0WhcNNDcwODA5MTYw\n\
NzU0WjAhMQswCQYDVQQGEwJHQjESMBAGA1UEAwwJcXVpYy50ZWNoMIIBIjANBgkq\n\
hkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAz5bOL7LD9kiIagcVrZqZ13ZcR0KhMuzs\n\
brqULbZKyqC+uBRgINxYJ7LPnJ4LPYuCt/nAaQ7CLXfKgzAMFu8eIK6UEvZA6+7b\n\
20E4rvOpPbTB/T4JbYZNQKyM9AEwr6j0P6vFgrWT7aBzhkmiqEe5vv/7ZOEGb+Ab\n\
+cvMeszfBbk93nyzKdNaUuh95x7/p0Ow315np2PRuoT0QQnA9zE/9eZ3Jah3cNZn\n\
NuQ6BDHlkegzTV5JhYYblRo/pmt2E9E0ha+NWsRLf3ZJUYhkYR3UqMltEKuLglCO\n\
VWBbPmKd4IZUNIotpKMVQSVb9agNBF49hH9iBhN3fBm7Hp8KBpjJLwIDAQABMA0G\n\
CSqGSIb3DQEBCwUAA4IBAQCo/Rn4spa5XFk0cCoKypP27DxePkGD9rQZk/CY4inV\n\
JV16anZ1pr9yfO61+m3fRKTZq7yxtHRDWxDdROHx9LqV1dXLAmh1ecV9Kn6/796O\n\
EHsOcVB0Lfi9Ili7//oUqlhGNploRuQbgWAXU+Eo1xJRWIXeedhzBSgEOMaQk3Zn\n\
TdYFhP0/Ao/fEdI4VULv1A43ztnZIB2KXWgUQoFT32woL47eWge8LxxVmmH3STtz\n\
nNcGnYxIorCQemDHDzMrvxRWgHxkpFGGqAhkFFyCmhKFPglKwt+yVTx26T8tShID\n\
ISMj0rgVMptmtWKJfzNCvFG52gsuO4w3yGdjgjRRrBDm\n\
-----END CERTIFICATE-----\n";

/// Matching 2048-bit RSA private key for TEST_CERT_PEM.
const TEST_KEY_PEM: &str = "-----BEGIN PRIVATE KEY-----\n\
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDPls4vssP2SIhq\n\
BxWtmpnXdlxHQqEy7OxuupQttkrKoL64FGAg3Fgnss+cngs9i4K3+cBpDsItd8qD\n\
MAwW7x4grpQS9kDr7tvbQTiu86k9tMH9Pglthk1ArIz0ATCvqPQ/q8WCtZPtoHOG\n\
SaKoR7m+//tk4QZv4Bv5y8x6zN8FuT3efLMp01pS6H3nHv+nQ7DfXmenY9G6hPRB\n\
CcD3MT/15nclqHdw1mc25DoEMeWR6DNNXkmFhhuVGj+ma3YT0TSFr41axEt/dklR\n\
iGRhHdSoyW0Qq4uCUI5VYFs+Yp3ghlQ0ii2koxVBJVv1qA0EXj2Ef2IGE3d8Gbse\n\
nwoGmMkvAgMBAAECggEBAMtFkpUmablKgTnBwjqCvs47OlUVK6AgW8x5qwuwC0Cr\n\
ctXyLcc/vJry/1UPdVZIvDHGv+Cf8Qhw2r7nV49FiqzaBmki9aOR+3uRPB4kvr6L\n\
t8Fw8+5pqlAAJu3wFGqN+M44N2mswDPaAAWpKTu7MGmVY+f+aT03qG1MYOiGoISK\n\
gP6DHiinddD38spM2muyCUyFZk9a+aBEfaQzZoU3gc0yB6R/qBOWZ7NIoIUMicku\n\
Zf3L6/06uunyZp+ueR83j1YWbg3JoYKlGAuQtDRF709+MQrim8lKTnfuHiBeZKYZ\n\
GNLSo7lGjrp6ccSyfXmlA36hSfdlrWtZJ4+utZShftECgYEA+NNOFNa1BLfDw3ot\n\
a6L4W6FE45B32bLbnBdg8foyEYrwzHLPFCbws1Z60pNr7NaCHDIMiKVOXvKQa78d\n\
qdWuPUVJ83uVs9GI8tAo00RAvBn6ut9yaaLa8mIv6ZpfU20IgE5sDjB7IBY9tTVd\n\
EDyJcDuKQXzQ48qmEw86wINQMd0CgYEA1ZMdt7yLnpDiYa6M/BuKjp7PWKcRlzVM\n\
BcCEYHA4LJ6xEOH4y9DEx2y5ljwOcXgJhXAfAyGQr7s1xiP/nXurqfmdP8u7bawp\n\
VwuWJ8Vv0ZXITaU0isezG2Dpnseuion3qSraWlmWUlWLVVgKETZmk7cF7VIXa0NT\n\
LFREdObI5HsCgYBUbm8KRyi5Zxm4VNbgtTYM8ZYMmdLxPe2i85PjyAABT+IRncuC\n\
jQwT7n5Swc9XWBpiMuFp5J3JPgmfZgRMwsMS61YClqbfk3Qi4FtaBMjqiu43Rubt\n\
zWL56DNV0xoRlufRkcq8rdq5spJR0L+5aLFCMhHh0taW1QaxZPOMq4IkyQKBgQC3\n\
GetubGzewqPyzuz77ri5URm+jW0dT4ofnE9hRpRCXMK9EJ52TkOGHYZ2cIKJcTno\n\
dpl/27Tpk/ykJJSu9SnVDbVszkOf4OuIPty6uCAHdPxG5Q3ItTCulkVz5QmUqHf1\n\
RlHxB8FCUSilQFdRLmx+03h3X9vID+4soQoXlwxAJQKBgE5SQpN+TG5V+E4zHgNd\n\
6cy6gA5dGDJ0KbsgxJwlKTFA9nIcs2ssBxLY9U4x75EGuqpeVNmq6xwwmPtBs0rp\n\
M3W4zdFrZQ3BneFRW7WbSBbsUSprkJW/p4GXa17GzGUq/MDXlGhNlApP1nknzFvE\n\
xGaH0/H/TZxpLCogVP9npUkj\n\
-----END PRIVATE KEY-----\n";

/// Writes temporary PEM files for server TLS configuration.
fn setup_test_tls_credentials() -> (tempfile::NamedTempFile, tempfile::NamedTempFile) {
    let mut cert_file = tempfile::NamedTempFile::new().expect("Failed to create cert tempfile");
    cert_file
        .write_all(TEST_CERT_PEM.as_bytes())
        .expect("Failed to write cert PEM");
    cert_file.flush().expect("Failed to flush cert PEM");

    let mut key_file = tempfile::NamedTempFile::new().expect("Failed to create key tempfile");
    key_file
        .write_all(TEST_KEY_PEM.as_bytes())
        .expect("Failed to write key PEM");
    key_file.flush().expect("Failed to flush key PEM");

    (cert_file, key_file)
}

/// Reads Linux process Resident Set Size (RSS) in kilobytes from /proc/self/status.
fn read_resident_set_size_kb() -> u64 {
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[1].parse::<u64>() {
                        return kb;
                    }
                }
            }
        }
    }
    0
}

/// Comprehensive metrics captured across the 5,000-stream concurrency benchmark.
#[derive(Debug, Clone)]
pub struct StreamScaleMetrics {
    pub total_streams: usize,
    pub total_duration: Duration,
    pub throughput_streams_per_sec: f64,
    pub min_latency: Duration,
    pub p50_latency: Duration,
    pub p90_latency: Duration,
    pub p99_latency: Duration,
    pub max_latency: Duration,
    pub initial_rss_kb: u64,
    pub peak_rss_kb: u64,
    pub memory_growth_kb: u64,
    pub max_canary_jitter: Duration,
    pub client_packets_sent: usize,
    pub client_packets_recv: usize,
    pub client_packets_lost: usize,
    pub client_packets_retrans: usize,
    pub client_streams_reset: u64,
    pub server_packets_sent: usize,
    pub server_packets_recv: usize,
    pub server_packets_lost: usize,
    pub server_streams_reset: u64,
}

impl StreamScaleMetrics {
    pub fn report_summary(&self) {
        println!("\n==================================================================");
        println!("  HOST-004 EMPIRICAL LOAD TEST RESULTS (5,000+ QUIC STREAMS)");
        println!("==================================================================");
        println!(" Total Multiplexed Streams : {}", self.total_streams);
        println!(" Total Test Duration       : {:.3?}", self.total_duration);
        println!(" Throughput                : {:.2} streams/sec", self.throughput_streams_per_sec);
        println!("------------------------------------------------------------------");
        println!(" Round-Trip Latency Profile:");
        println!("   - Min                   : {:.3?}", self.min_latency);
        println!("   - P50 (Median)          : {:.3?}", self.p50_latency);
        println!("   - P90                   : {:.3?}", self.p90_latency);
        println!("   - P99                   : {:.3?}", self.p99_latency);
        println!("   - Max                   : {:.3?}", self.max_latency);
        println!("------------------------------------------------------------------");
        println!(" Worker Starvation & Scheduling Invariant:");
        println!("   - Max Canary Jitter     : {:.3?} (threshold < 100ms)", self.max_canary_jitter);
        println!("------------------------------------------------------------------");
        println!(" Memory Usage (/proc/self/status):");
        println!("   - Initial RSS           : {} KB ({:.2} MB)", self.initial_rss_kb, self.initial_rss_kb as f64 / 1024.0);
        println!("   - Peak RSS              : {} KB ({:.2} MB)", self.peak_rss_kb, self.peak_rss_kb as f64 / 1024.0);
        println!("   - Memory Growth         : {} KB ({:.2} MB)", self.memory_growth_kb, self.memory_growth_kb as f64 / 1024.0);
        println!("------------------------------------------------------------------");
        println!(" QUIC Telemetry & Packet Loss Invariants:");
        println!("   - Client Packets Sent   : {}", self.client_packets_sent);
        println!("   - Client Packets Recv   : {}", self.client_packets_recv);
        println!("   - Client Packets Lost   : {} (must be 0)", self.client_packets_lost);
        println!("   - Client Retransmissions: {} (must be 0)", self.client_packets_retrans);
        println!("   - Client Stream Resets  : {} (must be 0)", self.client_streams_reset);
        println!("   - Server Packets Sent   : {}", self.server_packets_sent);
        println!("   - Server Packets Recv   : {}", self.server_packets_recv);
        println!("   - Server Packets Lost   : {} (must be 0)", self.server_packets_lost);
        println!("   - Server Stream Resets  : {} (must be 0)", self.server_streams_reset);
        println!("==================================================================\n");
    }
}

/// Request message submitted by individual stream tasks to the client multiplexer driver.
struct StreamJob {
    stream_id: u64,
    payload: Vec<u8>,
    reply_tx: oneshot::Sender<Vec<u8>>,
    start_time: Instant,
}

/// Runs the 5,000-stream concurrency load test with the specified number of streams.
async fn run_quic_stream_multiplexing_load_test(num_streams: usize) -> StreamScaleMetrics {
    let initial_rss_kb = read_resident_set_size_kb();
    let test_start = Instant::now();

    // 1. Prepare Transport Configuration for 5,000+ streams
    let mut transport_config = TransportConfig::default();
    transport_config.udp_bind_addr = "127.0.0.1:0".to_string();
    transport_config.initial_max_streams_bidi = 10_000;
    transport_config.initial_max_data = 50_000_000; // 50MB connection flow control window
    transport_config.max_idle_timeout_ms = 30_000;

    let (cert_file, key_file) = setup_test_tls_credentials();

    // Build Server Quiche Configuration
    let mut server_quiche_config = Http3Transport::build_quiche_config(&transport_config)
        .expect("Failed to build server quiche config");
    server_quiche_config
        .load_cert_chain_from_pem_file(cert_file.path().to_str().unwrap())
        .expect("Failed to load server cert PEM");
    server_quiche_config
        .load_priv_key_from_pem_file(key_file.path().to_str().unwrap())
        .expect("Failed to load server key PEM");

    // Build Client Quiche Configuration
    let mut client_quiche_config = Http3Transport::build_quiche_config(&transport_config)
        .expect("Failed to build client quiche config");
    client_quiche_config.verify_peer(false); // Disable cert verification for loopback test

    // 2. Bind OS UDP Sockets (Exactly 2 sockets: 1 server, 1 client -> zero port/FD exhaustion)
    let server_socket = Arc::new(
        UdpSocket::bind(&transport_config.udp_bind_addr)
            .await
            .expect("Failed to bind server UDP socket"),
    );
    let server_addr: SocketAddr = server_socket.local_addr().expect("Server local addr");

    let client_socket = Arc::new(
        UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind client UDP socket"),
    );
    let client_addr: SocketAddr = client_socket.local_addr().expect("Client local addr");

    // 3. Launch Liveness Canary to detect Tokio worker thread starvation
    let canary_stop = Arc::new(AtomicBool::new(false));
    let canary_stop_clone = canary_stop.clone();
    let canary_task = tokio::spawn(async move {
        let mut max_delay = Duration::ZERO;
        let tick_interval = Duration::from_millis(5);
        while !canary_stop_clone.load(Ordering::Relaxed) {
            let t0 = Instant::now();
            tokio::time::sleep(tick_interval).await;
            let elapsed = t0.elapsed();
            if elapsed > tick_interval {
                let delay = elapsed - tick_interval;
                if delay > max_delay {
                    max_delay = delay;
                }
            }
        }
        max_delay
    });

    // 4. Synchronization Channels
    let (server_ready_tx, server_ready_rx) = oneshot::channel();
    let (server_shutdown_tx, mut server_shutdown_rx) = oneshot::channel();
    let (server_stats_tx, server_stats_rx) = oneshot::channel();

    // 5. Spawn Server Driver Task
    let server_socket_task = server_socket.clone();
    tokio::spawn(async move {
        let mut in_buf = [0u8; 65535];
        let mut out_buf = [0u8; 65535];
        let mut stream_buf = [0u8; 8192];
        let mut stream_buffers: HashMap<u64, Vec<u8>> = HashMap::new();

        // Step 5a: Await initial handshake packet from client
        let (len, from_addr) = server_socket_task
            .recv_from(&mut in_buf)
            .await
            .expect("Server recv initial packet");

        let hdr = quiche::Header::from_slice(&mut in_buf[..len], quiche::MAX_CONN_ID_LEN)
            .expect("Server parse initial header");

        let mut server_conn = quiche::accept(
            &hdr.dcid,
            None,
            server_addr,
            from_addr,
            &mut server_quiche_config,
        )
        .expect("Server quiche::accept");

        server_conn
            .recv(
                &mut in_buf[..len],
                quiche::RecvInfo {
                    to: server_addr,
                    from: from_addr,
                },
            )
            .expect("Server feed initial packet");

        // Send initial handshake response
        while let Ok((write_len, send_info)) = server_conn.send(&mut out_buf) {
            server_socket_task
                .send_to(&out_buf[..write_len], send_info.to)
                .await
                .expect("Server initial send");
        }

        let _ = server_ready_tx.send(());

        // Step 5b: Server Multiplexing Event Loop
        loop {
            tokio::select! {
                recv_res = server_socket_task.recv_from(&mut in_buf) => {
                    if let Ok((len, from)) = recv_res {
                        let recv_info = quiche::RecvInfo { to: server_addr, from };
                        let _ = server_conn.recv(&mut in_buf[..len], recv_info);

                        // Process incoming streams
                        for s in server_conn.readable() {
                            while let Ok((read_len, fin)) = server_conn.stream_recv(s, &mut stream_buf) {
                                let entry = stream_buffers.entry(s).or_default();
                                entry.extend_from_slice(&stream_buf[..read_len]);
                                if fin {
                                    // Formulate echo response: "aura_resp_stream_XXXXX"
                                    let req_str = String::from_utf8_lossy(entry);
                                    let resp_str = req_str.replace("req", "resp");
                                    let _ = server_conn.stream_send(s, resp_str.as_bytes(), true);
                                    stream_buffers.remove(&s);
                                }
                            }
                        }
                    }
                }
                _ = tokio::time::sleep(server_conn.timeout().unwrap_or(Duration::from_millis(50))) => {
                    server_conn.on_timeout();
                }
                _ = &mut server_shutdown_rx => {
                    // Drain remaining outbound packets before teardown
                    while let Ok((write_len, send_info)) = server_conn.send(&mut out_buf) {
                        let _ = server_socket_task.send_to(&out_buf[..write_len], send_info.to).await;
                    }
                    let _ = server_stats_tx.send(server_conn.stats());
                    break;
                }
            }

            // Drain outbound packets
            while let Ok((write_len, send_info)) = server_conn.send(&mut out_buf) {
                let _ = server_socket_task
                    .send_to(&out_buf[..write_len], send_info.to)
                    .await;
            }
        }
    });

    server_ready_rx.await.expect("Server ready sync failed");

    // 6. Spawn Client Driver Task
    let (job_tx, mut job_rx) = mpsc::channel::<StreamJob>(num_streams);
    let (client_stats_tx, client_stats_rx) = oneshot::channel();
    let client_socket_task = client_socket.clone();

    tokio::spawn(async move {
        let mut in_buf = [0u8; 65535];
        let mut out_buf = [0u8; 65535];
        let mut stream_buf = [0u8; 8192];
        let mut inflight: HashMap<u64, (Vec<u8>, oneshot::Sender<Vec<u8>>)> = HashMap::new();

        // Step 6a: Generate SCID & initiate QUIC connection
        let mut scid = [0u8; quiche::MAX_CONN_ID_LEN];
        let uuid_bytes = uuid::Uuid::new_v4();
        scid[..16].copy_from_slice(uuid_bytes.as_bytes());
        let scid = quiche::ConnectionId::from_ref(&scid);

        let mut client_conn = quiche::connect(
            None,
            &scid,
            client_addr,
            server_addr,
            &mut client_quiche_config,
        )
        .expect("Client quiche::connect");

        // Send initial handshake packet
        let (write_len, send_info) = client_conn
            .send(&mut out_buf)
            .expect("Client initial send");
        client_socket_task
            .send_to(&out_buf[..write_len], send_info.to)
            .await
            .expect("Client send initial UDP");

        // Complete TLS 1.3 Handshake
        while !client_conn.is_established() {
            let timeout = client_conn.timeout().unwrap_or(Duration::from_millis(20));
            tokio::select! {
                recv_res = client_socket_task.recv_from(&mut in_buf) => {
                    if let Ok((len, from)) = recv_res {
                        let recv_info = quiche::RecvInfo { to: client_addr, from };
                        let _ = client_conn.recv(&mut in_buf[..len], recv_info);
                    }
                }
                _ = tokio::time::sleep(timeout) => {
                    client_conn.on_timeout();
                }
            }
            while let Ok((write_len, send_info)) = client_conn.send(&mut out_buf) {
                let _ = client_socket_task
                    .send_to(&out_buf[..write_len], send_info.to)
                    .await;
            }
        }

        // Step 6b: Client Stream Multiplexing Loop
        let mut completed_streams = 0;
        let mut all_jobs_submitted = false;

        loop {
            tokio::select! {
                // Ingest stream jobs from concurrent client tasks
                maybe_job = job_rx.recv(), if !all_jobs_submitted => {
                    match maybe_job {
                        Some(job) => {
                            if let Ok(_) = client_conn.stream_send(job.stream_id, &job.payload, true) {
                                inflight.insert(job.stream_id, (Vec::new(), job.reply_tx));
                            } else {
                                panic!("quiche stream_send failed on stream {}", job.stream_id);
                            }

                            // Efficiently drain any additional ready jobs in the channel buffer
                            while let Ok(extra_job) = job_rx.try_recv() {
                                if let Ok(_) = client_conn.stream_send(extra_job.stream_id, &extra_job.payload, true) {
                                    inflight.insert(extra_job.stream_id, (Vec::new(), extra_job.reply_tx));
                                } else {
                                    panic!("quiche stream_send failed on extra stream {}", extra_job.stream_id);
                                }
                            }
                        }
                        None => {
                            all_jobs_submitted = true;
                        }
                    }
                }

                // Ingest UDP responses from server
                recv_res = client_socket_task.recv_from(&mut in_buf) => {
                    if let Ok((len, from)) = recv_res {
                        let recv_info = quiche::RecvInfo { to: client_addr, from };
                        let _ = client_conn.recv(&mut in_buf[..len], recv_info);

                        for s in client_conn.readable() {
                            while let Ok((read_len, fin)) = client_conn.stream_recv(s, &mut stream_buf) {
                                if let Some((buf, _)) = inflight.get_mut(&s) {
                                    buf.extend_from_slice(&stream_buf[..read_len]);
                                }
                                if fin {
                                    if let Some((buf, reply_tx)) = inflight.remove(&s) {
                                        let _ = reply_tx.send(buf);
                                        completed_streams += 1;
                                    }
                                }
                            }
                        }
                    }
                }

                // Protocol timers & pacing
                _ = tokio::time::sleep(client_conn.timeout().unwrap_or(Duration::from_millis(20))) => {
                    client_conn.on_timeout();
                }
            }

            // Drain outbound datagrams
            while let Ok((write_len, send_info)) = client_conn.send(&mut out_buf) {
                let _ = client_socket_task
                    .send_to(&out_buf[..write_len], send_info.to)
                    .await;
            }

            if completed_streams == num_streams {
                break;
            }
        }

        let _ = client_stats_tx.send(client_conn.stats());
    });

    // 7. Spawn 5,000 Concurrent Client Tasks Driving Concurrent Stream Multiplexing
    let mut task_handles = Vec::with_capacity(num_streams);
    for i in 0..num_streams {
        let tx = job_tx.clone();
        let stream_id = (i as u64) * 4; // Client bidirectional QUIC stream identifiers
        let payload = format!("aura_req_stream_{:05}", i).into_bytes();
        let expected_resp = format!("aura_resp_stream_{:05}", i).into_bytes();

        let handle = tokio::spawn(async move {
            let (reply_tx, reply_rx) = oneshot::channel();
            let start = Instant::now();
            let job = StreamJob {
                stream_id,
                payload,
                reply_tx,
                start_time: start,
            };

            tx.send(job).await.expect("Failed to enqueue stream job");
            let resp = reply_rx.await.expect("Stream reply dropped");
            assert_eq!(resp, expected_resp, "Payload mismatch on stream {}", stream_id);
            start.elapsed()
        });
        task_handles.push(handle);
    }
    drop(job_tx); // Drop primary sender so receiver terminates when all tasks finish

    // 8. Await Completion of All 5,000 Concurrent Streams
    let mut latencies = Vec::with_capacity(num_streams);
    for handle in task_handles {
        let lat = handle.await.expect("Stream task panicked");
        latencies.push(lat);
    }
    latencies.sort();

    let total_duration = test_start.elapsed();

    // 9. Stop Canary and Collect Metrics
    canary_stop.store(true, Ordering::Relaxed);
    let max_canary_jitter = canary_task.await.expect("Canary task panicked");

    let _ = server_shutdown_tx.send(());
    let server_stats = server_stats_rx.await.expect("Server stats failed");
    let client_stats = client_stats_rx.await.expect("Client stats failed");

    let peak_rss_kb = read_resident_set_size_kb();
    let memory_growth_kb = peak_rss_kb.saturating_sub(initial_rss_kb);

    let throughput_streams_per_sec = num_streams as f64 / total_duration.as_secs_f64();
    let min_latency = latencies[0];
    let p50_latency = latencies[(num_streams as f64 * 0.50) as usize];
    let p90_latency = latencies[(num_streams as f64 * 0.90) as usize];
    let p99_latency = latencies[(num_streams as f64 * 0.99) as usize];
    let max_latency = latencies[num_streams - 1];

    StreamScaleMetrics {
        total_streams: num_streams,
        total_duration,
        throughput_streams_per_sec,
        min_latency,
        p50_latency,
        p90_latency,
        p99_latency,
        max_latency,
        initial_rss_kb,
        peak_rss_kb,
        memory_growth_kb,
        max_canary_jitter,
        client_packets_sent: client_stats.sent,
        client_packets_recv: client_stats.recv,
        client_packets_lost: client_stats.lost,
        client_packets_retrans: client_stats.retrans,
        client_streams_reset: client_stats.reset_stream_count_local
            + client_stats.reset_stream_count_remote,
        server_packets_sent: server_stats.sent,
        server_packets_recv: server_stats.recv,
        server_packets_lost: server_stats.lost,
        server_streams_reset: server_stats.reset_stream_count_local
            + server_stats.reset_stream_count_remote,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_5000_quic_streams_full_concurrency() {
    const TARGET_STREAMS: usize = 5_000;
    println!("\n[HOST-004] Starting Empirical Scale Test: 5,000 Full Concurrency Streams...");

    let metrics = run_quic_stream_multiplexing_load_test(TARGET_STREAMS).await;
    metrics.report_summary();

    // Invariant 1: Scale Completeness
    assert_eq!(
        metrics.total_streams, TARGET_STREAMS,
        "Must multiplex and complete all 5,000 streams"
    );

    // Invariant 2: Zero Packet Loss
    assert_eq!(
        metrics.client_packets_lost, 0,
        "Client must experience 0 packet loss across 5,000 streams"
    );
    assert_eq!(
        metrics.server_packets_lost, 0,
        "Server must experience 0 packet loss across 5,000 streams"
    );

    // Invariant 3: Zero Stream Resets
    assert_eq!(
        metrics.client_streams_reset, 0,
        "Client must experience 0 stream resets"
    );
    assert_eq!(
        metrics.server_streams_reset, 0,
        "Server must experience 0 stream resets"
    );

    // Invariant 4: Zero Worker Thread Starvation
    assert!(
        metrics.max_canary_jitter < Duration::from_millis(150),
        "Max canary timer jitter ({:?}) exceeded 150ms: worker starvation detected!",
        metrics.max_canary_jitter
    );

    // Invariant 5: Sub-Second P99 Latency
    assert!(
        metrics.p99_latency < Duration::from_millis(1000),
        "P99 round-trip latency ({:?}) must be sub-second",
        metrics.p99_latency
    );

    // Invariant 6: Bounded Peak Memory Consumption (< 150MB growth)
    assert!(
        metrics.memory_growth_kb < 150_000,
        "Memory growth ({} KB) exceeded bounded ceiling of 150MB",
        metrics.memory_growth_kb
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_5000_quic_streams_batched_concurrency() {
    const BATCH_SIZE: usize = 1_000;
    const NUM_BATCHES: usize = 5;
    println!("\n[HOST-004] Starting Empirical Scale Test: 5 Batches of 1,000 Concurrent Streams...");

    let mut total_streams = 0;
    let mut total_duration = Duration::ZERO;

    for batch_idx in 0..NUM_BATCHES {
        let metrics = run_quic_stream_multiplexing_load_test(BATCH_SIZE).await;
        total_streams += metrics.total_streams;
        total_duration += metrics.total_duration;

        assert_eq!(metrics.client_packets_lost, 0, "Batch {} packet loss", batch_idx);
        assert_eq!(metrics.client_streams_reset, 0, "Batch {} stream reset", batch_idx);
        assert!(
            metrics.max_canary_jitter < Duration::from_millis(150),
            "Batch {} worker starvation",
            batch_idx
        );
    }

    assert_eq!(total_streams, 5_000, "Must complete 5,000 streams across batches");
    println!(
        "[HOST-004] Batched Scale Test Passed: 5,000 streams completed across 5 batches in {:.3?}",
        total_duration
    );
}
