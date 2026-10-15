use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use vsock_screenshot_daemon::server::handle_client_interactive;

/// Helper to spin up an interactive JSON-RPC daemon listener on an ephemeral TCP port.
fn spawn_test_server() -> (std::net::SocketAddr, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral TCP port");
    let addr = listener.local_addr().unwrap();

    let server_handle = thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            let clone = match stream.try_clone() {
                Ok(c) => c,
                Err(_) => break,
            };
            let mut reader = BufReader::new(clone);
            thread::spawn(move || {
                let _ = handle_client_interactive(&mut reader, &mut stream, None, false, "stress-client");
            });
        }
    });

    (addr, server_handle)
}

#[test]
fn test_rapid_sequential_burst_pipelined_75_calls() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral TCP port");
    let addr = listener.local_addr().unwrap();

    let server_handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let clone = stream.try_clone().unwrap();
            let mut reader = BufReader::new(clone);
            let _ = handle_client_interactive(&mut reader, &mut stream, None, false, "burst-75-client");
        }
    });

    let mut client = TcpStream::connect(addr).expect("Failed to connect client");

    // Pipeline 75 mixed requests in a single buffered burst
    let mut payload = String::new();
    for i in 1..=75 {
        let req = match i % 6 {
            0 => format!(r#"{{"jsonrpc": "2.0", "id": {}, "method": "move", "params": {{"x": {}, "y": {}}}}}{}"#, i, i * 10, i * 10, "\n"),
            1 => format!(r#"{{"jsonrpc": "2.0", "id": {}, "method": "click", "params": {{"x": {}, "y": {}, "button": 1, "clicks": 1}}}}{}"#, i, i * 5, i * 5, "\n"),
            2 => format!(r#"{{"jsonrpc": "2.0", "id": {}, "method": "drag", "params": {{"start_x": 0, "start_y": 0, "end_x": {}, "end_y": {}}}}}{}"#, i, i, i, "\n"),
            3 => format!(r#"{{"jsonrpc": "2.0", "id": {}, "method": "type", "params": {{"text": "cmd_{}"}}}}{}"#, i, i, "\n"),
            4 => format!(r#"{{"jsonrpc": "2.0", "id": {}, "method": "key", "params": {{"key": "Return", "action": "press"}}}}{}"#, i, "\n"),
            _ => format!(r#"{{"jsonrpc": "2.0", "id": {}, "method": "screenshot", "params": {{"format": "png"}}}}{}"#, i, "\n"),
        };
        payload.push_str(&req);
    }

    client.write_all(payload.as_bytes()).expect("Failed to write burst payload");
    client.flush().expect("Failed to flush client stream");

    let mut client_reader = BufReader::new(client.try_clone().unwrap());
    for i in 1..=75 {
        let mut line = String::new();
        let bytes_read = client_reader.read_line(&mut line).expect("Failed to read response line");
        assert!(bytes_read > 0, "Unexpected early EOF at request {}", i);

        let resp: serde_json::Value = serde_json::from_str(line.trim())
            .unwrap_or_else(|e| panic!("Failed to parse response {}: {} on line: {}", i, e, line));

        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], i);
        assert!(resp["error"].is_null(), "Unexpected error in response {}: {:?}", i, resp["error"]);
        assert_eq!(resp["result"]["status"], "ok");
    }

    drop(client_reader);
    drop(client);
    let _ = server_handle.join();
}

#[test]
fn test_malformed_json_rpc_messages_and_error_recovery() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral TCP port");
    let addr = listener.local_addr().unwrap();

    let server_handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let clone = stream.try_clone().unwrap();
            let mut reader = BufReader::new(clone);
            let _ = handle_client_interactive(&mut reader, &mut stream, None, false, "malformed-client");
        }
    });

    let mut client = TcpStream::connect(addr).expect("Failed to connect client");
    let mut reader = BufReader::new(client.try_clone().unwrap());

    // 1. Initial message starts with '{' but is malformed JSON syntax
    client.write_all(b"{\"invalid_json\":\n").unwrap();
    client.flush().unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(resp["error"]["code"], -32700);
    assert_eq!(resp["error"]["message"], "Parse error");

    // 2. Missing method name
    client.write_all(b"{\"jsonrpc\": \"2.0\", \"id\": 2}\n").unwrap();
    client.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(resp["error"]["code"], -32700);

    // 3. Unknown method
    client.write_all(b"{\"jsonrpc\": \"2.0\", \"id\": 3, \"method\": \"non_existent_method\", \"params\": {}}\n").unwrap();
    client.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(resp["id"], 3);
    assert_eq!(resp["error"]["code"], -32601);
    assert_eq!(resp["error"]["message"], "Method not found");

    // 4. Invalid params for 'click' (params is string instead of object)
    client.write_all(b"{\"jsonrpc\": \"2.0\", \"id\": 4, \"method\": \"click\", \"params\": \"not an object\"}\n").unwrap();
    client.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(resp["id"], 4);
    assert_eq!(resp["error"]["code"], -32602);
    assert_eq!(resp["error"]["message"], "Invalid params");

    // 5. Invalid params type for coordinates
    client.write_all(b"{\"jsonrpc\": \"2.0\", \"id\": 5, \"method\": \"move\", \"params\": {\"x\": \"wrong_string\", \"y\": 10}}\n").unwrap();
    client.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(resp["id"], 5);
    assert_eq!(resp["error"]["code"], -32602);

    // 6. Missing required text field for 'type'
    client.write_all(b"{\"jsonrpc\": \"2.0\", \"id\": 6, \"method\": \"type\", \"params\": {}}\n").unwrap();
    client.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(resp["id"], 6);
    assert_eq!(resp["error"]["code"], -32602);

    // 7. Non-JSON string sent within existing JSON-RPC session
    client.write_all(b"COMPLETELY_RANDOM_GARBAGE_STRING\n").unwrap();
    client.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(resp["error"]["code"], -32700);

    // 8. CRITICAL RECOVERY CHECK: Verify connection is STILL ALIVE and serves valid requests!
    client.write_all(b"{\"jsonrpc\": \"2.0\", \"id\": 100, \"method\": \"move\", \"params\": {\"x\": 640, \"y\": 480}}\n").unwrap();
    client.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(resp["id"], 100);
    assert_eq!(resp["result"]["status"], "ok");
    assert_eq!(resp["result"]["x"], 640);
    assert_eq!(resp["result"]["y"], 480);

    drop(reader);
    drop(client);
    let _ = server_handle.join();
}

#[test]
fn test_boundary_coordinates_zero_negative_intmax() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral TCP port");
    let addr = listener.local_addr().unwrap();

    let server_handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let clone = stream.try_clone().unwrap();
            let mut reader = BufReader::new(clone);
            let _ = handle_client_interactive(&mut reader, &mut stream, None, false, "boundary-client");
        }
    });

    let mut client = TcpStream::connect(addr).expect("Failed to connect client");
    let mut reader = BufReader::new(client.try_clone().unwrap());

    let boundary_cases = vec![
        // (id, method, params_json)
        (1, "move", r#"{"x": 0, "y": 0}"#),
        (2, "move", r#"{"x": -1, "y": -1}"#),
        (3, "move", r#"{"x": -32768, "y": -32768}"#),
        (4, "move", r#"{"x": 32767, "y": 32767}"#),
        (5, "move", r#"{"x": -2147483648, "y": 2147483647}"#),
        (6, "click", r#"{"x": 2147483647, "y": -2147483648, "button": 1, "clicks": 1}"#),
        (7, "click", r#"{"x": 0, "y": 0, "button": 255, "clicks": 100}"#),
        (8, "drag", r#"{"start_x": -2147483648, "start_y": -2147483648, "end_x": 2147483647, "end_y": 2147483647, "button": 1}"#),
        (9, "drag", r#"{"x": 0, "y": 0}"#),
    ];

    for (id, method, params) in boundary_cases {
        let req = format!(r#"{{"jsonrpc": "2.0", "id": {}, "method": "{}", "params": {}}}{}"#, id, method, params, "\n");
        client.write_all(req.as_bytes()).unwrap();
        client.flush().unwrap();

        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let resp: serde_json::Value = serde_json::from_str(line.trim())
            .unwrap_or_else(|e| panic!("Failed to parse boundary response {}: {}", id, e));

        assert_eq!(resp["id"], id);
        assert!(resp["error"].is_null(), "Boundary test {} errored: {:?}", id, resp["error"]);
        assert_eq!(resp["result"]["status"], "ok");
    }

    // Now test extreme coordinate overflow (> i32::MAX) -> should return Invalid params without panic
    let overflow_req = r#"{"jsonrpc": "2.0", "id": 999, "method": "move", "params": {"x": 9999999999999999999999, "y": 0}}"#;
    client.write_all(format!("{}\n", overflow_req).as_bytes()).unwrap();
    client.flush().unwrap();

    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(resp["id"], 999);
    assert_eq!(resp["error"]["code"], -32602);

    drop(reader);
    drop(client);
    let _ = server_handle.join();
}

#[test]
fn test_large_typing_string_and_unicode_emojis() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral TCP port");
    let addr = listener.local_addr().unwrap();

    let server_handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let clone = stream.try_clone().unwrap();
            let mut reader = BufReader::new(clone);
            let _ = handle_client_interactive(&mut reader, &mut stream, None, false, "unicode-client");
        }
    });

    let mut client = TcpStream::connect(addr).expect("Failed to connect client");
    let mut reader = BufReader::new(client.try_clone().unwrap());

    // 1. Large 50KB typing string
    let large_str = "A".repeat(50_000);
    let req1 = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "type",
        "params": { "text": large_str }
    });
    client.write_all(format!("{}\n", req1).as_bytes()).unwrap();
    client.flush().unwrap();

    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let resp1: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(resp1["id"], 1);
    assert_eq!(resp1["result"]["status"], "ok");
    assert_eq!(resp1["result"]["length"], 50_000);

    // 2. Unicode and emojis
    let unicode_str = "🚀✨ Crab 🦀 Aura Sandboxed GUI Automation 🎯";
    let req2 = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "type",
        "params": { "text": unicode_str }
    });
    client.write_all(format!("{}\n", req2).as_bytes()).unwrap();
    client.flush().unwrap();

    line.clear();
    reader.read_line(&mut line).unwrap();
    let resp2: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(resp2["id"], 2);
    assert_eq!(resp2["result"]["status"], "ok");
    assert_eq!(resp2["result"]["length"], unicode_str.len());

    // 3. Empty string
    let req3 = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "type",
        "params": { "text": "" }
    });
    client.write_all(format!("{}\n", req3).as_bytes()).unwrap();
    client.flush().unwrap();

    line.clear();
    reader.read_line(&mut line).unwrap();
    let resp3: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(resp3["id"], 3);
    assert_eq!(resp3["result"]["status"], "ok");
    assert_eq!(resp3["result"]["length"], 0);

    drop(reader);
    drop(client);
    let _ = server_handle.join();
}

#[test]
fn test_concurrent_tcp_clients_stress() {
    let (addr, _server_handle) = spawn_test_server();

    let mut handles = Vec::new();

    // 5 concurrent threads, each sending 20 commands
    for client_id in 0..5 {
        let handle = thread::spawn(move || {
            let mut client = TcpStream::connect(addr).expect("Client failed to connect");
            let mut reader = BufReader::new(client.try_clone().unwrap());

            for seq in 1..=20 {
                let req_id = client_id * 1000 + seq;
                let req = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "method": "move",
                    "params": { "x": req_id, "y": req_id }
                });
                client.write_all(format!("{}\n", req).as_bytes()).unwrap();
                client.flush().unwrap();

                let mut line = String::new();
                let bytes = reader.read_line(&mut line).unwrap();
                assert!(bytes > 0, "Client {} seq {} connection closed early", client_id, seq);

                let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
                assert_eq!(resp["id"], req_id);
                assert_eq!(resp["result"]["status"], "ok");
                assert_eq!(resp["result"]["x"], req_id);
            }
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn test_varied_delays_and_extreme_clicks_over_persistent_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral TCP port");
    let addr = listener.local_addr().unwrap();

    let server_handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            // Explicitly set 150ms read timeout matching daemon listener configuration
            stream
                .set_read_timeout(Some(std::time::Duration::from_millis(150)))
                .unwrap();
            let clone = stream.try_clone().unwrap();
            let mut reader = BufReader::new(clone);
            let _ = handle_client_interactive(
                &mut reader,
                &mut stream,
                None,
                false,
                "delays-and-extreme-clicks-client",
            );
        }
    });

    let mut client = TcpStream::connect(addr).expect("Failed to connect client");
    client
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    let mut reader = BufReader::new(client.try_clone().unwrap());

    // 1. Varied inter-command sleep delays: 0ms, 50ms, 150ms, 200ms, 400ms, 600ms across 10+ sequential commands
    let delays = [0, 50, 150, 200, 400, 600, 0, 50, 150, 200, 400, 600];
    assert!(delays.len() >= 10, "Must test across 10+ sequential commands");

    for (idx, &delay_ms) in delays.iter().enumerate() {
        if delay_ms > 0 {
            thread::sleep(std::time::Duration::from_millis(delay_ms));
        }

        let cmd_id = (idx + 1) as u64;
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": cmd_id,
            "method": "move",
            "params": { "x": (idx as i32) * 10, "y": (idx as i32) * 20 }
        });

        let mut payload = serde_json::to_string(&req).unwrap();
        payload.push('\n');
        client
            .write_all(payload.as_bytes())
            .unwrap_or_else(|e| panic!("Failed writing command {} after {}ms delay: {}", cmd_id, delay_ms, e));
        client.flush().unwrap();

        let mut resp_line = String::new();
        let bytes_read = reader
            .read_line(&mut resp_line)
            .unwrap_or_else(|e| panic!("Read failed on command {} after {}ms delay (ConnectionReset/Timeout?): {}", cmd_id, delay_ms, e));

        assert!(
            bytes_read > 0,
            "Session terminated unexpectedly (EOF) on command {} after {}ms delay",
            cmd_id,
            delay_ms
        );

        let resp: serde_json::Value = serde_json::from_str(resp_line.trim())
            .unwrap_or_else(|e| panic!("Invalid JSON response on command {}: {} (line: {})", cmd_id, e, resp_line));

        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], cmd_id);
        assert!(resp["error"].is_null(), "Unexpected error on command {}: {:?}", cmd_id, resp["error"]);
        assert_eq!(resp["result"]["status"], "ok");
        assert_eq!(resp["result"]["action"], "move");
    }

    // 2. Extreme clicks: 0, -1, 10_000_000 asserting bounded execution and valid JSON-RPC responses

    // Case A: clicks = 0 -> Clamps to 1 bounded clicks
    {
        let req_zero = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 101,
            "method": "click",
            "params": { "x": 100, "y": 200, "button": 1, "clicks": 0 }
        });
        client.write_all(format!("{}\n", req_zero).as_bytes()).unwrap();
        client.flush().unwrap();

        let mut line = String::new();
        let bytes = reader.read_line(&mut line).unwrap();
        assert!(bytes > 0, "Session terminated on clicks: 0");

        let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 101);
        assert!(resp["error"].is_null());
        assert_eq!(resp["result"]["status"], "ok");
        assert_eq!(resp["result"]["action"], "click");
        assert_eq!(resp["result"]["clicks"], 1, "Clicks: 0 must clamp to 1");
    }

    // Case B: clicks = -1 -> Invalid params error (u32 parse failure), session MUST NOT terminate
    {
        let req_neg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 102,
            "method": "click",
            "params": { "x": 100, "y": 200, "button": 1, "clicks": -1 }
        });
        client.write_all(format!("{}\n", req_neg).as_bytes()).unwrap();
        client.flush().unwrap();

        let mut line = String::new();
        let bytes = reader.read_line(&mut line).unwrap();
        assert!(bytes > 0, "Session terminated on clicks: -1");

        let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 102);
        assert!(resp["result"].is_null());
        let err = &resp["error"];
        assert_eq!(err["code"], -32602, "Clicks: -1 must return -32602 Invalid params error");
    }

    // Case C: clicks = 10_000_000 -> Clamps to 25 bounded clicks
    {
        let req_huge = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 103,
            "method": "click",
            "params": { "x": 100, "y": 200, "button": 1, "clicks": 10_000_000 }
        });
        client.write_all(format!("{}\n", req_huge).as_bytes()).unwrap();
        client.flush().unwrap();

        let mut line = String::new();
        let bytes = reader.read_line(&mut line).unwrap();
        assert!(bytes > 0, "Session terminated on clicks: 10_000_000");

        let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 103);
        assert!(resp["error"].is_null());
        assert_eq!(resp["result"]["status"], "ok");
        assert_eq!(resp["result"]["action"], "click");
        assert_eq!(resp["result"]["clicks"], 25, "Clicks: 10_000_000 must clamp to 25");
    }

    // 3. Follow-up command over the same connection after delay to ensure session continuity
    {
        thread::sleep(std::time::Duration::from_millis(200));
        let req_final = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 104,
            "method": "screenshot",
            "params": { "format": "png" }
        });
        client.write_all(format!("{}\n", req_final).as_bytes()).unwrap();
        client.flush().unwrap();

        let mut line = String::new();
        let bytes = reader.read_line(&mut line).unwrap();
        assert!(bytes > 0, "Session terminated on follow-up command");

        let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(resp["id"], 104);
        assert_eq!(resp["result"]["status"], "ok");
        assert!(resp["result"]["png_base64"].is_string());
    }

    drop(reader);
    drop(client);
    let _ = server_handle.join();
}
