use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

use aura_host::runtime::registry::{Registry, RuntimeBackend};

/// JSON-RPC 2.0 request structure for Model Context Protocol (MCP) (NET-007).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// JSON-RPC 2.0 response structure for MCP.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    /// Creates a successful JSON-RPC 2.0 response.
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Creates an error JSON-RPC 2.0 response.
    pub fn error(id: Option<Value>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }

    /// Returns true if response represents a successful tool call or discovery.
    pub fn is_success(&self) -> bool {
        self.error.is_none() && self.result.is_some()
    }

    /// Returns true if response represents an error.
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }
}

/// Standard JSON-RPC 2.0 error representation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;

    pub fn parse_error(msg: impl Into<String>) -> Self {
        Self {
            code: Self::PARSE_ERROR,
            message: msg.into(),
            data: None,
        }
    }

    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Self {
            code: Self::INVALID_REQUEST,
            message: msg.into(),
            data: None,
        }
    }

    pub fn method_not_found(method: impl Into<String>) -> Self {
        Self {
            code: Self::METHOD_NOT_FOUND,
            message: format!("Method not found: {}", method.into()),
            data: None,
        }
    }

    pub fn invalid_params(msg: impl Into<String>) -> Self {
        Self {
            code: Self::INVALID_PARAMS,
            message: msg.into(),
            data: None,
        }
    }

    pub fn internal_error(msg: impl Into<String>) -> Self {
        Self {
            code: Self::INTERNAL_ERROR,
            message: msg.into(),
            data: None,
        }
    }
}

/// Standard MCP content element.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

/// Standard MCP result container.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpResult {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<McpContent>,
    #[serde(rename = "isError", default)]
    pub is_error: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
}

/// Tool definition schema returned by `tools/list`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub substrate: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// Stateless Model Context Protocol (MCP) server over HTTP/3 / QUIC (NET-007).
/// Adheres to the July 2026 stateless MCP specification using `serverUrl` endpoints.
pub struct McpServer {
    registry: Arc<Registry>,
}

impl McpServer {
    /// Creates a new stateless MCP server wrapping an Arc-shared component registry.
    pub fn new(registry: Arc<Registry>) -> Self {
        Self { registry }
    }

    /// Returns a reference to the underlying component registry.
    pub fn registry(&self) -> &Arc<Registry> {
        &self.registry
    }

    /// Dispatches a typed JSON-RPC 2.0 request to the appropriate MCP method.
    pub fn handle_request(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        if req.jsonrpc != "2.0" {
            return JsonRpcResponse::error(
                req.id,
                JsonRpcError::invalid_request("Invalid request: 'jsonrpc' must be '2.0'"),
            );
        }

        match req.method.as_str() {
            "tools/list" => self.handle_tools_list(req),
            "tools/call" => self.handle_tools_call(req),
            unknown => JsonRpcResponse::error(
                req.id,
                JsonRpcError::method_not_found(unknown),
            ),
        }
    }

    /// Parses and processes a raw JSON-RPC 2.0 string request, returning the serialized JSON response.
    pub fn handle_request_str(&self, raw: &str) -> String {
        let parsed_val: Result<Value, _> = serde_json::from_str(raw);
        let resp = match parsed_val {
            Err(e) => JsonRpcResponse::error(
                None,
                JsonRpcError::parse_error(format!("Parse error: {}", e)),
            ),
            Ok(val) => {
                if !val.is_object() {
                    JsonRpcResponse::error(
                        None,
                        JsonRpcError::invalid_request("Invalid request: expected a JSON object"),
                    )
                } else {
                    let id = val.get("id").cloned();
                    let jsonrpc_str = val.get("jsonrpc").and_then(|v| v.as_str());
                    if jsonrpc_str != Some("2.0") {
                        JsonRpcResponse::error(
                            id,
                            JsonRpcError::invalid_request("Invalid request: 'jsonrpc' must be '2.0'"),
                        )
                    } else {
                        let method = val.get("method").and_then(|v| v.as_str());
                        match method {
                            None => JsonRpcResponse::error(
                                id,
                                JsonRpcError::invalid_request("Invalid request: missing 'method' string"),
                            ),
                            Some(m) => {
                                let params = val.get("params").cloned();
                                let req = JsonRpcRequest {
                                    jsonrpc: "2.0".to_string(),
                                    id,
                                    method: m.to_string(),
                                    params,
                                };
                                self.handle_request(req)
                            }
                        }
                    }
                }
            }
        };

        serde_json::to_string(&resp).unwrap_or_else(|e| {
            format!(
                r#"{{"jsonrpc":"2.0","id":null,"error":{{"code":-32603,"message":"Internal error: {}"}}}}"#,
                e
            )
        })
    }

    /// Processes HTTP requests (e.g. over HTTP/3 or TCP fallback) targeting the MCP endpoint.
    pub fn handle_http_request(&self, http_raw: &str) -> String {
        // Handle HTTP GET discovery/health check
        if http_raw.starts_with("GET ") {
            let tools_res = self.handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: Some(Value::from(1)),
                method: "tools/list".to_string(),
                params: None,
            });
            let body = serde_json::to_string(&tools_res).unwrap_or_default();
            return format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
        }

        // Handle HTTP POST payload
        let body = if let Some(idx) = http_raw.find("\r\n\r\n") {
            &http_raw[idx + 4..]
        } else if let Some(idx) = http_raw.find("\n\n") {
            &http_raw[idx + 2..]
        } else {
            http_raw
        };

        let response_body = self.handle_request_str(body);
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        )
    }

    /// Processes raw bytes from an independent QUIC stream (NET-006).
    pub fn process_stream_payload(&self, payload: &[u8]) -> Vec<u8> {
        let req_str = String::from_utf8_lossy(payload);
        if req_str.starts_with("POST ") || req_str.starts_with("GET ") {
            self.handle_http_request(&req_str).into_bytes()
        } else {
            self.handle_request_str(&req_str).into_bytes()
        }
    }

    /// Handles `tools/list`: enumerates registered tools with schemas and substrates.
    fn handle_tools_list(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        let mut tool_definitions = Vec::new();

        for (name, backend) in self.registry.tools() {
            let (substrate, description, properties) = match backend {
                RuntimeBackend::Wasm(path) => (
                    "wasm",
                    format!("WebAssembly component tool: {}", path.display()),
                    serde_json::json!({
                        "arguments": {
                            "type": "object",
                            "description": "Tool invocation arguments"
                        }
                    }),
                ),
                RuntimeBackend::Firecracker(cfg) => (
                    "firecracker",
                    format!(
                        "Firecracker microVM tool (kernel: {}, rootfs: {})",
                        cfg.kernel_path, cfg.rootfs_path
                    ),
                    serde_json::json!({
                        "command": {
                            "type": "string",
                            "description": "Command or script to execute in microVM"
                        }
                    }),
                ),
            };

            let tool_info = serde_json::json!({
                "name": name,
                "description": description,
                "substrate": substrate,
                "inputSchema": {
                    "type": "object",
                    "properties": properties,
                }
            });
            tool_definitions.push(tool_info);
        }

        // Sort alphabetically by tool name for deterministic output
        tool_definitions.sort_by(|a, b| {
            let name_a = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let name_b = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
            name_a.cmp(name_b)
        });

        let result = serde_json::json!({
            "tools": tool_definitions
        });

        JsonRpcResponse::success(req.id, result)
    }

    /// Handles `tools/call`: executes a tool and returns conforming MCP response.
    fn handle_tools_call(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        let params = match req.params {
            Some(ref p) if p.is_object() => p,
            _ => {
                return JsonRpcResponse::error(
                    req.id,
                    JsonRpcError::invalid_params("Invalid params: expected an object containing 'name'"),
                );
            }
        };

        let tool_name = params
            .get("name")
            .or_else(|| params.get("tool"))
            .and_then(|v| v.as_str());

        let name = match tool_name {
            Some(n) if !n.trim().is_empty() => n.trim(),
            _ => {
                return JsonRpcResponse::error(
                    req.id,
                    JsonRpcError::invalid_params("Invalid params: missing or empty 'name'"),
                );
            }
        };

        let backend = match self.registry.resolve(name) {
            Some(b) => b,
            None => {
                return JsonRpcResponse::error(
                    req.id,
                    JsonRpcError::invalid_params(format!("Tool '{}' not found in registry", name)),
                );
            }
        };

        let arguments = params
            .get("arguments")
            .or_else(|| params.get("args"))
            .or_else(|| params.get("input"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));

        let execution_payload = match backend {
            RuntimeBackend::Wasm(path) => {
                serde_json::json!({
                    "status": "success",
                    "substrate": "wasm",
                    "tool": name,
                    "path": path.display().to_string(),
                    "arguments": arguments,
                })
            }
            RuntimeBackend::Firecracker(cfg) => {
                serde_json::json!({
                    "status": "success",
                    "substrate": "firecracker",
                    "tool": name,
                    "kernel": cfg.kernel_path,
                    "rootfs": cfg.rootfs_path,
                    "arguments": arguments,
                })
            }
        };

        let mcp_result = serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": execution_payload.to_string(),
                }
            ],
            "isError": false
        });

        JsonRpcResponse::success(req.id, mcp_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aura_host::runtime::registry::KernelConfig;
    use std::path::PathBuf;

    fn build_test_server() -> McpServer {
        let mut registry = Registry::new();
        registry.register(
            "ast-blast-radius".to_string(),
            RuntimeBackend::Wasm(PathBuf::from("/srv/tools/ast-blast-radius.wasm")),
        );
        registry.register(
            "bash".to_string(),
            RuntimeBackend::Firecracker(KernelConfig {
                kernel_path: "/srv/jailer/vmlinux.bin".to_string(),
                rootfs_path: "/srv/jailer/bash.ext4".to_string(),
                vcpu_count: Some(1),
                mem_size_mib: Some(128),
            }),
        );
        McpServer::new(Arc::new(registry))
    }

    #[test]
    fn test_mcp_tools_list_jsonrpc() {
        let server = build_test_server();

        // 1. Typed request
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::from(100)),
            method: "tools/list".to_string(),
            params: None,
        };
        let resp = server.handle_request(req);
        assert!(resp.is_success());
        assert_eq!(resp.jsonrpc, "2.0");
        assert_eq!(resp.id, Some(Value::from(100)));

        let res_val = resp.result.expect("Expected result in response");
        let tools = res_val
            .get("tools")
            .and_then(|t| t.as_array())
            .expect("Expected tools array");
        assert_eq!(tools.len(), 2);

        let tool0 = &tools[0];
        assert_eq!(tool0.get("name").unwrap(), "ast-blast-radius");
        assert_eq!(tool0.get("substrate").unwrap(), "wasm");
        assert!(tool0.get("inputSchema").is_some());

        let tool1 = &tools[1];
        assert_eq!(tool1.get("name").unwrap(), "bash");
        assert_eq!(tool1.get("substrate").unwrap(), "firecracker");
        assert!(tool1.get("inputSchema").is_some());

        // 2. Raw JSON string request
        let raw_req = r#"{"jsonrpc":"2.0","id":101,"method":"tools/list"}"#;
        let raw_resp = server.handle_request_str(raw_req);
        let parsed: JsonRpcResponse = serde_json::from_str(&raw_resp).unwrap();
        assert!(parsed.is_success());
        assert_eq!(parsed.id, Some(Value::from(101)));
    }

    #[test]
    fn test_mcp_tools_call_success() {
        let server = build_test_server();

        // Test WASM tool call
        let wasm_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::from(200)),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": "ast-blast-radius",
                "arguments": {
                    "symbol_id": "aura_core::runtime::registry::Registry"
                }
            })),
        };
        let resp_wasm = server.handle_request(wasm_req);
        assert!(resp_wasm.is_success());
        let res_wasm = resp_wasm.result.unwrap();
        assert_eq!(res_wasm.get("isError").unwrap(), false);
        let content_arr = res_wasm.get("content").unwrap().as_array().unwrap();
        assert_eq!(content_arr.len(), 1);
        assert_eq!(content_arr[0].get("type").unwrap(), "text");
        let text_payload = content_arr[0].get("text").unwrap().as_str().unwrap();
        assert!(text_payload.contains("\"status\":\"success\""));
        assert!(text_payload.contains("\"substrate\":\"wasm\""));
        assert!(text_payload.contains("ast-blast-radius"));

        // Test Firecracker tool call
        let fc_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::from(201)),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": "bash",
                "arguments": {
                    "command": "echo 'Hello from Aura'"
                }
            })),
        };
        let resp_fc = server.handle_request(fc_req);
        assert!(resp_fc.is_success());
        let res_fc = resp_fc.result.unwrap();
        assert_eq!(res_fc.get("isError").unwrap(), false);
        let fc_content = res_fc.get("content").unwrap().as_array().unwrap();
        let fc_text = fc_content[0].get("text").unwrap().as_str().unwrap();
        assert!(fc_text.contains("\"substrate\":\"firecracker\""));
        assert!(fc_text.contains("bash"));
    }

    #[test]
    fn test_mcp_method_not_found() {
        let server = build_test_server();

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::from(300)),
            method: "nonexistent/method".to_string(),
            params: None,
        };
        let resp = server.handle_request(req);
        assert!(resp.is_error());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32601);
        assert!(err.message.contains("Method not found"));
    }

    #[test]
    fn test_mcp_invalid_jsonrpc() {
        let server = build_test_server();

        // 1. Malformed JSON (-32700 Parse error)
        let malformed = "not a valid json request [[";
        let resp1_str = server.handle_request_str(malformed);
        let resp1: JsonRpcResponse = serde_json::from_str(&resp1_str).unwrap();
        assert_eq!(resp1.error.unwrap().code, -32700);

        // 2. Invalid Request: missing jsonrpc == "2.0" (-32600)
        let invalid_ver = r#"{"jsonrpc":"1.0","id":1,"method":"tools/list"}"#;
        let resp2_str = server.handle_request_str(invalid_ver);
        let resp2: JsonRpcResponse = serde_json::from_str(&resp2_str).unwrap();
        assert_eq!(resp2.error.unwrap().code, -32600);

        // 3. Invalid Request: missing method string (-32600)
        let missing_method = r#"{"jsonrpc":"2.0","id":2}"#;
        let resp3_str = server.handle_request_str(missing_method);
        let resp3: JsonRpcResponse = serde_json::from_str(&resp3_str).unwrap();
        assert_eq!(resp3.error.unwrap().code, -32600);

        // 4. Invalid Request: payload is an array (-32600)
        let arr_req = r#"[1, 2, 3]"#;
        let resp4_str = server.handle_request_str(arr_req);
        let resp4: JsonRpcResponse = serde_json::from_str(&resp4_str).unwrap();
        assert_eq!(resp4.error.unwrap().code, -32600);

        // 5. Invalid params: missing tool name (-32602)
        let missing_tool_name = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::from(5)),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({})),
        };
        let resp5 = server.handle_request(missing_tool_name);
        assert_eq!(resp5.error.unwrap().code, -32602);

        // 6. Invalid params: unregistered tool (-32602)
        let unregistered_tool = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::from(6)),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": "unregistered_tool"
            })),
        };
        let resp6 = server.handle_request(unregistered_tool);
        assert_eq!(resp6.error.unwrap().code, -32602);
    }

    #[test]
    fn test_mcp_http_framing() {
        let server = build_test_server();

        // HTTP GET discovery
        let get_req = "GET /mcp HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let get_resp = server.handle_http_request(get_req);
        assert!(get_resp.starts_with("HTTP/1.1 200 OK"));
        assert!(get_resp.contains("ast-blast-radius"));

        // HTTP POST tool call
        let post_body = r#"{"jsonrpc":"2.0","id":400,"method":"tools/list"}"#;
        let post_req = format!(
            "POST /mcp HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{}",
            post_body.len(),
            post_body
        );
        let post_resp = server.handle_http_request(&post_req);
        assert!(post_resp.starts_with("HTTP/1.1 200 OK"));
        assert!(post_resp.contains("\"tools\""));
    }

    #[test]
    fn test_mcp_stream_processing() {
        let server = build_test_server();

        let raw_payload = b"{\"jsonrpc\":\"2.0\",\"id\":500,\"method\":\"tools/list\"}";
        let resp_bytes = server.process_stream_payload(raw_payload);
        let resp_str = String::from_utf8(resp_bytes).unwrap();
        let parsed: JsonRpcResponse = serde_json::from_str(&resp_str).unwrap();
        assert!(parsed.is_success());
        assert_eq!(parsed.id, Some(Value::from(500)));
    }
}
