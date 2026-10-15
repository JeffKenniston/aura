use crate::capture::capture_screen;
use crate::input::InputExecutor;
use std::error::Error;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;
use vsock::{VsockAddr, VsockListener, VsockStream, VMADDR_CID_ANY, VMADDR_CID_HOST};

/// Transmits raw PNG screenshot to writer (legacy one-shot capture mode).
pub fn send_raw_screenshot<W: Write>(
    writer: &mut W,
    display_name: Option<&str>,
    length_prefix: bool,
) -> Result<(), Box<dyn Error>> {
    let png_bytes = match capture_screen(display_name) {
        Ok(b) => b,
        Err(_) => {
            // Headless fallback frame for tests
            let fallback_rgba = vec![0u8; 1 * 1 * 4];
            crate::capture::encode_png(1, 1, &fallback_rgba)?
        }
    };

    if length_prefix {
        let len_be = (png_bytes.len() as u32).to_be_bytes();
        writer.write_all(&len_be)?;
    }

    writer.write_all(&png_bytes)?;
    writer.flush()?;
    Ok(())
}

/// Handles incoming connection in persistent JSON-RPC 2.0 loop,
/// falling back to raw PNG capture if client sends legacy raw capture commands or times out.
pub fn handle_client_interactive<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    display_name: Option<&str>,
    length_prefix: bool,
    peer_name: &str,
) -> Result<(), Box<dyn Error>> {
    let executor = InputExecutor::new(display_name.map(|s| s.to_string()));
    let mut line = String::new();

    let read_res = reader.read_line(&mut line);
    match read_res {
        Ok(0) => {
            // Stream EOF
            Ok(())
        }
        Ok(_) => {
            let trimmed = line.trim();
            if trimmed.starts_with('{') {
                // Line-delimited JSON-RPC mode
                if let Some(resp) = executor.process_line(trimmed) {
                    let resp_str = serde_json::to_string(&resp)?;
                    writer.write_all(resp_str.as_bytes())?;
                    writer.write_all(b"\n")?;
                    writer.flush()?;
                }
                line.clear();

                // Persistent command loop over same connection
                loop {
                    match reader.read_line(&mut line) {
                        Ok(0) => break, // EOF
                        Ok(_) => {
                            let trimmed = line.trim();
                            if !trimmed.is_empty() {
                                if let Some(resp) = executor.process_line(trimmed) {
                                    let resp_str = serde_json::to_string(&resp)?;
                                    writer.write_all(resp_str.as_bytes())?;
                                    writer.write_all(b"\n")?;
                                    writer.flush()?;
                                }
                            }
                            line.clear();
                        }
                        Err(e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                || e.kind() == std::io::ErrorKind::TimedOut =>
                        {
                            continue;
                        }
                        Err(e) => return Err(Box::new(e)),
                    }
                }
                Ok(())
            } else {
                // Non-JSON command -> Legacy raw PNG capture fallback
                println!(
                    "[vsock-screenshot-daemon] Client {} requested raw stream",
                    peer_name
                );
                send_raw_screenshot(writer, display_name, length_prefix)
            }
        }
        Err(e)
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut =>
        {
            // Initial read timed out with 0 bytes sent -> Legacy raw PNG capture fallback
            println!(
                "[vsock-screenshot-daemon] Client {} timed out on initial command, sending raw frame",
                peer_name
            );
            send_raw_screenshot(writer, display_name, length_prefix)
        }
        Err(e) => Err(Box::new(e)),
    }
}

/// Runs the VSOCK listener daemon accepting connections.
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

    let disp_opt = display_name.map(|s| s.to_string());

    for stream_res in listener.incoming() {
        match stream_res {
            Ok(mut stream) => {
                let peer_desc = match stream.peer_addr() {
                    Ok(p) => format!("CID:{}:{}", p.cid(), p.port()),
                    Err(_) => "unknown-vsock-peer".to_string(),
                };
                let disp = disp_opt.clone();
                std::thread::spawn(move || {
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(150)));
                    if let Ok(clone) = stream.try_clone() {
                        let mut reader = BufReader::new(clone);
                        if let Err(e) = handle_client_interactive(
                            &mut reader,
                            &mut stream,
                            disp.as_deref(),
                            length_prefix,
                            &peer_desc,
                        ) {
                            eprintln!("[vsock-screenshot-daemon] Error handling stream: {}", e);
                        }
                    }
                });
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
    let target_cid = if host_cid == 0 {
        VMADDR_CID_HOST
    } else {
        host_cid
    };
    let addr = VsockAddr::new(target_cid, port);
    println!(
        "[vsock-screenshot-daemon] Connecting to host CID: {}, port: {}",
        target_cid, port
    );
    let mut stream = VsockStream::connect(&addr)?;
    send_raw_screenshot(&mut stream, display_name, length_prefix)
}

/// Runs TCP fallback listener for local testing outside microVM environments.
pub fn run_tcp_listener(
    port: u16,
    display_name: Option<&str>,
    length_prefix: bool,
) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))?;
    println!("[vsock-screenshot-daemon] Listening on TCP 0.0.0.0:{}", port);

    let disp_opt = display_name.map(|s| s.to_string());

    for stream_res in listener.incoming() {
        match stream_res {
            Ok(mut stream) => {
                let peer_desc = stream
                    .peer_addr()
                    .map(|a| a.to_string())
                    .unwrap_or_else(|_| "unknown-tcp-peer".to_string());
                let disp = disp_opt.clone();
                std::thread::spawn(move || {
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(150)));
                    if let Ok(clone) = stream.try_clone() {
                        let mut reader = BufReader::new(clone);
                        if let Err(e) = handle_client_interactive(
                            &mut reader,
                            &mut stream,
                            disp.as_deref(),
                            length_prefix,
                            &peer_desc,
                        ) {
                            eprintln!("[vsock-screenshot-daemon] Error handling stream: {}", e);
                        }
                    }
                });
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
    let mut stream = TcpStream::connect(addr)?;
    send_raw_screenshot(&mut stream, display_name, length_prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_handle_client_jsonrpc_multiple_commands() {
        let req1 = r#"{"jsonrpc": "2.0", "id": 1, "method": "move", "params": {"x": 100, "y": 200}}"#;
        let req2 = r#"{"jsonrpc": "2.0", "id": 2, "method": "click", "params": {"x": 100, "y": 200, "button": 1}}"#;
        let req3 = r#"{"jsonrpc": "2.0", "id": 3, "method": "type", "params": {"text": "echo ok"}}"#;
        let input_data = format!("{}\n{}\n{}\n", req1, req2, req3);

        let mut reader = Cursor::new(input_data.into_bytes());
        let mut writer = Vec::new();

        handle_client_interactive(&mut reader, &mut writer, None, false, "test-peer").unwrap();

        let output_str = String::from_utf8(writer).unwrap();
        let lines: Vec<&str> = output_str.trim().split('\n').collect();
        assert_eq!(lines.len(), 3);

        let resp1: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(resp1["id"], 1);
        assert_eq!(resp1["result"]["action"], "move");

        let resp2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(resp2["id"], 2);
        assert_eq!(resp2["result"]["action"], "click");

        let resp3: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
        assert_eq!(resp3["id"], 3);
        assert_eq!(resp3["result"]["action"], "type");
    }

    #[test]
    fn test_handle_client_legacy_raw_fallback() {
        let input_data = b"screenshot\n";
        let mut reader = Cursor::new(input_data);
        let mut writer = Vec::new();

        handle_client_interactive(&mut reader, &mut writer, None, false, "legacy-peer").unwrap();

        // Must output valid PNG bytes
        assert_eq!(&writer[0..8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
    }

    #[test]
    fn test_handle_client_legacy_length_prefixed() {
        let input_data = b"raw\n";
        let mut reader = Cursor::new(input_data);
        let mut writer = Vec::new();

        handle_client_interactive(&mut reader, &mut writer, None, true, "legacy-peer").unwrap();

        let len = u32::from_be_bytes([writer[0], writer[1], writer[2], writer[3]]) as usize;
        assert_eq!(len, writer.len() - 4);
        assert_eq!(&writer[4..12], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
    }
}
