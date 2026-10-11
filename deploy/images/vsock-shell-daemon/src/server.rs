use crate::session::{BashSessionRequest, SessionManager};
use std::error::Error;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use vsock::{VsockAddr, VsockListener, VMADDR_CID_ANY};

/// Handles incoming client commands line-by-line over a buffered stream.
pub fn handle_client<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    manager: &Arc<Mutex<SessionManager>>,
) -> Result<(), Box<dyn Error>> {
    let mut line = String::new();
    while reader.read_line(&mut line)? > 0 {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            line.clear();
            continue;
        }

        let req: BashSessionRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let err_resp = serde_json::json!({
                    "session_id": "error",
                    "stdout": "",
                    "stderr": format!("Invalid JSON request: {}", e),
                    "exit_code": 1,
                    "cwd": null,
                });
                writer.write_all(err_resp.to_string().as_bytes())?;
                writer.write_all(b"\n")?;
                writer.flush()?;
                line.clear();
                continue;
            }
        };

        let output = {
            let mut mgr = manager.lock().unwrap();
            mgr.execute(&req)?
        };

        let resp_json = serde_json::to_string(&output)?;
        writer.write_all(resp_json.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        line.clear();
    }
    Ok(())
}

/// Runs the AF_VSOCK listener daemon for in-guest microVM execution (TOOL-BASH-002).
pub fn run_vsock_listener(port: u32, manager: Arc<Mutex<SessionManager>>) -> Result<(), Box<dyn Error>> {
    let addr = VsockAddr::new(VMADDR_CID_ANY, port);
    let listener = VsockListener::bind(&addr)?;
    println!("[vsock-shell-daemon] Listening on AF_VSOCK port {}", port);

    for stream_res in listener.incoming() {
        if let Ok(mut stream) = stream_res {
            let mgr = Arc::clone(&manager);
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let _ = handle_client(&mut reader, &mut stream, &mgr);
            });
        }
    }
    Ok(())
}

/// Runs the TCP listener for host-side development and unit testing (TOOL-BASH-002).
pub fn run_tcp_listener(port: u16, manager: Arc<Mutex<SessionManager>>) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))?;
    println!("[vsock-shell-daemon] Listening on TCP port {}", port);

    for stream_res in listener.incoming() {
        if let Ok(mut stream) = stream_res {
            let mgr = Arc::clone(&manager);
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let _ = handle_client(&mut reader, &mut stream, &mgr);
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::BashCommandOutput;
    use std::io::Cursor;
    use std::net::TcpStream;

    #[test]
    fn test_handle_client_json_roundtrip() {
        let manager = Arc::new(Mutex::new(SessionManager::new()));
        let req = BashSessionRequest {
            session_id: "test-client-1".to_string(),
            command: "echo 'IPC Roundtrip'".to_string(),
            action: "execute".to_string(),
        };
        let input_data = format!("{}\n", serde_json::to_string(&req).unwrap());
        let mut reader = Cursor::new(input_data.into_bytes());
        let mut writer = Vec::new();

        handle_client(&mut reader, &mut writer, &manager).expect("handle_client failed");

        let resp_str = String::from_utf8(writer).expect("Invalid UTF-8");
        let output: BashCommandOutput = serde_json::from_str(resp_str.trim()).expect("Parse error");
        assert_eq!(output.session_id, "test-client-1");
        assert_eq!(output.stdout, "IPC Roundtrip");
        assert_eq!(output.exit_code, 0);
    }

    #[test]
    fn test_tcp_listener_ipc_e2e() {
        let manager = Arc::new(Mutex::new(SessionManager::new()));
        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral TCP port");
        let local_addr = listener.local_addr().expect("Failed to get local addr");

        let mgr_clone = Arc::clone(&manager);
        let server_thread = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let _ = handle_client(&mut reader, &mut stream, &mgr_clone);
            }
        });

        let mut client_stream = TcpStream::connect(local_addr).expect("Failed to connect to server");
        let req = BashSessionRequest {
            session_id: "e2e-sid".to_string(),
            command: "echo 'Hello over TCP'".to_string(),
            action: "execute".to_string(),
        };
        let line = format!("{}\n", serde_json::to_string(&req).unwrap());
        client_stream.write_all(line.as_bytes()).unwrap();
        client_stream.flush().unwrap();

        let mut reader = BufReader::new(client_stream);
        let mut resp_line = String::new();
        reader.read_line(&mut resp_line).unwrap();

        let output: BashCommandOutput = serde_json::from_str(resp_line.trim()).unwrap();
        assert_eq!(output.stdout, "Hello over TCP");
        assert_eq!(output.exit_code, 0);

        drop(reader);
        server_thread.join().unwrap();
    }
}
