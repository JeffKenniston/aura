use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use vsock_screenshot_daemon::server::handle_client_interactive;

#[test]
fn test_tcp_jsonrpc_e2e_all_methods() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral port");
    let addr = listener.local_addr().unwrap();

    let server_handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let clone = stream.try_clone().unwrap();
            let mut reader = BufReader::new(clone);
            handle_client_interactive(&mut reader, &mut stream, None, false, "e2e-client").unwrap();
        }
    });

    let mut client = TcpStream::connect(addr).expect("Failed to connect to daemon TCP");

    let commands = vec![
        r#"{"jsonrpc": "2.0", "id": 1, "method": "move", "params": {"x": 300, "y": 400}}"#,
        r#"{"jsonrpc": "2.0", "id": 2, "method": "click", "params": {"x": 300, "y": 400, "button": 1, "clicks": 2}}"#,
        r#"{"jsonrpc": "2.0", "id": 3, "method": "drag", "params": {"start_x": 100, "start_y": 100, "end_x": 500, "end_y": 300, "button": 1}}"#,
        r#"{"jsonrpc": "2.0", "id": 4, "method": "type", "params": {"text": "cargo test --all"}}"#,
        r#"{"jsonrpc": "2.0", "id": 5, "method": "key", "params": {"key": "Return", "action": "press"}}"#,
        r#"{"jsonrpc": "2.0", "id": 6, "method": "screenshot", "params": {"format": "png"}}"#,
    ];

    let mut client_reader = BufReader::new(client.try_clone().unwrap());

    for (idx, cmd) in commands.iter().enumerate() {
        client.write_all(cmd.as_bytes()).unwrap();
        client.write_all(b"\n").unwrap();
        client.flush().unwrap();

        let mut resp_line = String::new();
        client_reader.read_line(&mut resp_line).unwrap();

        let resp: serde_json::Value = serde_json::from_str(resp_line.trim()).unwrap();
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], (idx + 1) as i64);

        let result = &resp["result"];
        assert_eq!(result["status"], "ok");

        match idx {
            0 => {
                assert_eq!(result["action"], "move");
                assert_eq!(result["x"], 300);
            }
            1 => {
                assert_eq!(result["action"], "click");
                assert_eq!(result["clicks"], 2);
            }
            2 => {
                assert_eq!(result["action"], "drag");
                assert_eq!(result["start_x"], 100);
                assert_eq!(result["end_x"], 500);
            }
            3 => {
                assert_eq!(result["action"], "type");
                assert_eq!(result["length"], 16);
            }
            4 => {
                assert_eq!(result["action"], "key");
                assert_eq!(result["key"], "Return");
            }
            5 => {
                assert_eq!(result["format"], "png");
                assert!(result["png_base64"].as_str().is_some());
            }
            _ => {}
        }
    }

    drop(client_reader);
    drop(client);
    server_handle.join().unwrap();
}

#[test]
fn test_tcp_pipelined_burst_commands() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral port");
    let addr = listener.local_addr().unwrap();

    let server_handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let clone = stream.try_clone().unwrap();
            let mut reader = BufReader::new(clone);
            handle_client_interactive(&mut reader, &mut stream, None, false, "burst-client").unwrap();
        }
    });

    let mut client = TcpStream::connect(addr).expect("Failed to connect");

    // Send 50 pipelined commands in one burst
    let mut payload = String::new();
    for i in 1..=50 {
        payload.push_str(&format!(
            r#"{{"jsonrpc": "2.0", "id": {}, "method": "move", "params": {{"x": {}, "y": {}}}}}{}"#,
            i, i, i, "\n"
        ));
    }
    client.write_all(payload.as_bytes()).unwrap();
    client.flush().unwrap();

    let mut client_reader = BufReader::new(client.try_clone().unwrap());
    for i in 1..=50 {
        let mut line = String::new();
        client_reader.read_line(&mut line).unwrap();
        let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(resp["id"], i);
        assert_eq!(resp["result"]["x"], i);
    }

    drop(client_reader);
    drop(client);
    server_handle.join().unwrap();
}

#[test]
fn test_tcp_spaced_commands_succeed_over_same_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral port");
    let addr = listener.local_addr().unwrap();

    let server_handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            // Explicitly set 150ms read timeout matching daemon listener configuration
            stream
                .set_read_timeout(Some(std::time::Duration::from_millis(150)))
                .unwrap();
            let clone = stream.try_clone().unwrap();
            let mut reader = BufReader::new(clone);
            handle_client_interactive(&mut reader, &mut stream, None, false, "paced-client").unwrap();
        }
    });

    let mut client = TcpStream::connect(addr).expect("Failed to connect to daemon TCP");
    let mut client_reader = BufReader::new(client.try_clone().unwrap());

    // 1. Initial command
    let cmd1 = r#"{"jsonrpc": "2.0", "id": 1, "method": "move", "params": {"x": 120, "y": 240}}"#;
    client.write_all(cmd1.as_bytes()).unwrap();
    client.write_all(b"\n").unwrap();
    client.flush().unwrap();

    let mut line1 = String::new();
    client_reader.read_line(&mut line1).unwrap();
    let resp1: serde_json::Value = serde_json::from_str(line1.trim()).unwrap();
    assert_eq!(resp1["id"], 1);
    assert_eq!(resp1["result"]["status"], "ok");
    assert_eq!(resp1["result"]["action"], "move");
    assert_eq!(resp1["result"]["x"], 120);

    // 2. Inter-command latency: sleep 300ms (> 150ms socket read timeout)
    std::thread::sleep(std::time::Duration::from_millis(300));

    // 3. Second command over the exact same connection
    let cmd2 = r#"{"jsonrpc": "2.0", "id": 2, "method": "click", "params": {"x": 120, "y": 240, "button": 1, "clicks": 2}}"#;
    client.write_all(cmd2.as_bytes()).unwrap();
    client.write_all(b"\n").unwrap();
    client.flush().unwrap();

    let mut line2 = String::new();
    let bytes_read = client_reader.read_line(&mut line2).unwrap();
    assert!(bytes_read > 0, "Connection dropped prematurely after 300ms idle delay");

    let resp2: serde_json::Value = serde_json::from_str(line2.trim()).unwrap();
    assert_eq!(resp2["id"], 2);
    assert_eq!(resp2["result"]["status"], "ok");
    assert_eq!(resp2["result"]["action"], "click");
    assert_eq!(resp2["result"]["clicks"], 2);

    // 4. Second inter-command latency: sleep another 300ms
    std::thread::sleep(std::time::Duration::from_millis(300));

    // 5. Third command over the same connection
    let cmd3 = r#"{"jsonrpc": "2.0", "id": 3, "method": "screenshot", "params": {"format": "png"}}"#;
    client.write_all(cmd3.as_bytes()).unwrap();
    client.write_all(b"\n").unwrap();
    client.flush().unwrap();

    let mut line3 = String::new();
    client_reader.read_line(&mut line3).unwrap();
    let resp3: serde_json::Value = serde_json::from_str(line3.trim()).unwrap();
    assert_eq!(resp3["id"], 3);
    assert_eq!(resp3["result"]["status"], "ok");
    assert!(resp3["result"]["png_base64"].is_string());

    drop(client_reader);
    drop(client);
    server_handle.join().unwrap();
}

