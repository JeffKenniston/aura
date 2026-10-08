use crate::capture::capture_screen;
use std::error::Error;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Instant;
use vsock::{VsockAddr, VsockListener, VsockStream, VMADDR_CID_ANY, VMADDR_CID_HOST};

/// Handles incoming connection: captures screen and streams PNG bytes back to caller.
fn handle_client_stream<S: Read + Write>(
    mut stream: S,
    display_name: Option<&str>,
    length_prefix: bool,
    peer_name: &str,
) -> Result<(), Box<dyn Error>> {
    println!("[vsock-screenshot-daemon] Client connected from {}", peer_name);
    let start = Instant::now();

    // Check if client sent an initial command with a non-blocking / peek attempt
    // (If none sent, proceed directly to screen capture)
    let png_bytes = capture_screen(display_name)?;
    let capture_duration = start.elapsed();

    println!(
        "[vsock-screenshot-daemon] Frame captured in {:?}, size: {} bytes",
        capture_duration,
        png_bytes.len()
    );

    if length_prefix {
        let len_be = (png_bytes.len() as u32).to_be_bytes();
        stream.write_all(&len_be)?;
    }

    stream.write_all(&png_bytes)?;
    stream.flush()?;

    println!(
        "[vsock-screenshot-daemon] Transmitted frame to {} in {:?}",
        peer_name,
        start.elapsed()
    );
    Ok(())
}

/// Runs the VSOCK listener daemon accepting connections and sending PNG screenshots.
pub fn run_vsock_listener(
    port: u32,
    display_name: Option<&str>,
    length_prefix: bool,
) -> Result<(), Box<dyn Error>> {
    let addr = VsockAddr::new(VMADDR_CID_ANY, port);
    let listener = VsockListener::bind(&addr)?;
    println!(
        "[vsock-screenshot-daemon] Listening on AF_VSOCK CID ANY, port: {}",
        port
    );

    for stream_res in listener.incoming() {
        match stream_res {
            Ok(stream) => {
                let peer_desc = match stream.peer_addr() {
                    Ok(p) => format!("CID:{}:{}", p.cid(), p.port()),
                    Err(_) => "unknown-vsock-peer".to_string(),
                };
                if let Err(e) = handle_client_stream(stream, display_name, length_prefix, &peer_desc) {
                    eprintln!("[vsock-screenshot-daemon] Error handling stream: {}", e);
                }
            }
            Err(e) => {
                eprintln!("[vsock-screenshot-daemon] Error accepting connection: {}", e);
            }
        }
    }

    Ok(())
}

/// Initiates connection to host over VSOCK and pushes a single screenshot PNG.
pub fn push_vsock_screenshot(
    host_cid: u32,
    port: u32,
    display_name: Option<&str>,
    length_prefix: bool,
) -> Result<(), Box<dyn Error>> {
    let target_cid = if host_cid == 0 { VMADDR_CID_HOST } else { host_cid };
    let addr = VsockAddr::new(target_cid, port);
    println!(
        "[vsock-screenshot-daemon] Connecting to host CID: {}, port: {}",
        target_cid, port
    );
    let stream = VsockStream::connect(&addr)?;
    handle_client_stream(
        stream,
        display_name,
        length_prefix,
        &format!("CID:{}:{}", target_cid, port),
    )?;
    Ok(())
}

/// Runs TCP fallback listener for local testing outside microVM environments.
pub fn run_tcp_listener(
    port: u16,
    display_name: Option<&str>,
    length_prefix: bool,
) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))?;
    println!("[vsock-screenshot-daemon] Listening on TCP 0.0.0.0:{}", port);

    for stream_res in listener.incoming() {
        match stream_res {
            Ok(stream) => {
                let peer_desc = stream
                    .peer_addr()
                    .map(|a| a.to_string())
                    .unwrap_or_else(|_| "unknown-tcp-peer".to_string());
                if let Err(e) = handle_client_stream(stream, display_name, length_prefix, &peer_desc) {
                    eprintln!("[vsock-screenshot-daemon] Error handling stream: {}", e);
                }
            }
            Err(e) => {
                eprintln!("[vsock-screenshot-daemon] Error accepting connection: {}", e);
            }
        }
    }

    Ok(())
}

/// Pushes screenshot over TCP for testing outside microVM environments.
pub fn push_tcp_screenshot(
    addr: &str,
    display_name: Option<&str>,
    length_prefix: bool,
) -> Result<(), Box<dyn Error>> {
    println!("[vsock-screenshot-daemon] Connecting to TCP address: {}", addr);
    let stream = TcpStream::connect(addr)?;
    handle_client_stream(stream, display_name, length_prefix, addr)?;
    Ok(())
}
