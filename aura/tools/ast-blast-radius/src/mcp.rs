use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::graph::KnowledgeGraphEngine;

/// JSON-RPC 2.0 request structure for MCP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// JSON-RPC 2.0 response structure for MCP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<McpResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

impl McpResponse {
    /// Returns true if the response represents a successful tool call or discovery.
    pub fn is_success(&self) -> bool {
        self.error.is_none() && self.result.as_ref().map(|r| !r.is_error).unwrap_or(false)
    }

    /// Returns true if the response contains a JSON-RPC error or MCP result error.
    pub fn is_error(&self) -> bool {
        self.error.is_some() || self.result.as_ref().map(|r| r.is_error).unwrap_or(false)
    }

    /// Convenience helper to parse the `BlastRadiusMcpReport` from the first text content item.
    pub fn parse_report(&self) -> Result<BlastRadiusMcpReport, String> {
        let res = self
            .result
            .as_ref()
            .ok_or_else(|| "Response has no result object".to_string())?;
        let content = res
            .content
            .first()
            .ok_or_else(|| "Result has no content elements".to_string())?;
        serde_json::from_str(&content.text)
            .map_err(|e| format!("Failed to parse report JSON: {}", e))
    }
}

/// Result payload for MCP JSON-RPC response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResult {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<McpContent>,
    #[serde(rename = "isError", default)]
    pub is_error: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
}

/// Content element inside an MCP result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

/// JSON-RPC 2.0 error structure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpError {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Tool definition schema returned by `tools/list`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// Formatted report payload returned inside `McpContent::text`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlastRadiusMcpReport {
    pub target_symbol_id: String,
    pub affected_count: usize,
    pub affected_symbols: Vec<String>,
    pub transitively_affected_symbols: Vec<String>,
    pub risk_score: u32,
    pub test_coverage: f32,
    pub is_safe: bool,
    pub requires_escalation: bool,
    pub cycles_detected: bool,
}

/// MCP request handler wrapping the knowledge graph engine.
pub struct McpHandler<'a> {
    engine: &'a KnowledgeGraphEngine,
}

impl<'a> McpHandler<'a> {
    /// Creates a new `McpHandler` referencing the knowledge graph engine.
    pub fn new(engine: &'a KnowledgeGraphEngine) -> Self {
        Self { engine }
    }

    /// Handles a raw JSON string request and returns the serialized JSON response string.
    pub fn handle_raw(&self, request_json: &str) -> String {
        let parsed: Result<McpRequest, _> = serde_json::from_str(request_json);
        match parsed {
            Ok(req) => {
                let resp = self.dispatch(req);
                serde_json::to_string(&resp).unwrap_or_else(|e| {
                    format!(
                        r#"{{"jsonrpc":"2.0","id":null,"error":{{"code":-32603,"message":"Internal error: {}"}}}}"#,
                        e
                    )
                })
            }
            Err(e) => {
                let err_resp = McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: Some(McpError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                        data: None,
                    }),
                };
                serde_json::to_string(&err_resp).unwrap_or_else(|_| {
                    r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}}"#.to_string()
                })
            }
        }
    }

    /// Dispatches a strongly-typed `McpRequest` to the corresponding method handler.
    pub fn dispatch(&self, req: McpRequest) -> McpResponse {
        match req.method.as_str() {
            "tools/call" => self.handle_call_tool(req),
            "tools/list" => self.handle_list_tools(req),
            _ => McpResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: None,
                error: Some(McpError {
                    code: -32601,
                    message: format!("Method not found: {}", req.method),
                    data: None,
                }),
            },
        }
    }

    fn handle_call_tool(&self, req: McpRequest) -> McpResponse {
        let params = match req.params {
            Some(ref p) => p,
            None => {
                return McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id,
                    result: None,
                    error: Some(McpError {
                        code: -32602,
                        message: "Invalid params: missing parameters object".to_string(),
                        data: None,
                    }),
                };
            }
        };

        let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if tool_name != "ast-blast-radius" {
            return McpResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: None,
                error: Some(McpError {
                    code: -32601,
                    message: format!("Unknown tool: {}", tool_name),
                    data: None,
                }),
            };
        }

        // Extract symbol_id or target_symbol_id from arguments or params
        let symbol_id = params
            .get("arguments")
            .and_then(|v| v.as_object())
            .and_then(|args| {
                args.get("symbol_id")
                    .or_else(|| args.get("target_symbol_id"))
            })
            .or_else(|| params.get("symbol_id"))
            .or_else(|| params.get("target_symbol_id"))
            .and_then(|v| v.as_str());

        let target_symbol = match symbol_id {
            Some(s) if !s.trim().is_empty() => s.trim(),
            _ => {
                return McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id,
                    result: None,
                    error: Some(McpError {
                        code: -32602,
                        message: "Missing required argument 'symbol_id'".to_string(),
                        data: None,
                    }),
                };
            }
        };

        match self.engine.compute_blast_radius(target_symbol) {
            Ok(report) => {
                let transitively_affected: Vec<String> = report
                    .affected_symbols
                    .iter()
                    .filter(|s| s.as_str() != target_symbol)
                    .cloned()
                    .collect();

                let mcp_report = BlastRadiusMcpReport {
                    target_symbol_id: target_symbol.to_string(),
                    affected_count: report.affected_symbols.len(),
                    affected_symbols: report.affected_symbols.clone(),
                    transitively_affected_symbols: transitively_affected,
                    risk_score: report.risk_score,
                    test_coverage: report.test_coverage,
                    is_safe: report.is_safe(),
                    requires_escalation: report.requires_escalation(),
                    cycles_detected: false,
                };

                let report_json = serde_json::to_string_pretty(&mcp_report)
                    .unwrap_or_else(|_| format!("{:?}", report));

                McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id,
                    result: Some(McpResult {
                        content: vec![McpContent {
                            content_type: "text".to_string(),
                            text: report_json,
                        }],
                        is_error: false,
                        tools: None,
                    }),
                    error: None,
                }
            }
            Err(err) => McpResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: None,
                error: Some(McpError {
                    code: -32004,
                    message: err,
                    data: None,
                }),
            },
        }
    }

    fn handle_list_tools(&self, req: McpRequest) -> McpResponse {
        let tool_def = ToolDefinition {
            name: "ast-blast-radius".to_string(),
            description: "Recursive Common Table Expression dependency analysis against SQLite knowledge graph".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "symbol_id": {
                        "type": "string",
                        "description": "Qualified symbol identifier to evaluate blast radius for"
                    },
                    "target_symbol_id": {
                        "type": "string",
                        "description": "Alternative key for target symbol identifier"
                    }
                },
                "required": ["symbol_id"]
            }),
        };

        let tools_container = serde_json::json!({
            "tools": [&tool_def]
        });

        McpResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: Some(McpResult {
                content: vec![McpContent {
                    content_type: "text".to_string(),
                    text: serde_json::to_string_pretty(&tools_container).unwrap_or_default(),
                }],
                is_error: false,
                tools: Some(vec![tool_def]),
            }),
            error: None,
        }
    }
}

/// Convenience function routing raw JSON-RPC string directly against the engine.
pub fn handle_mcp_request(engine: &KnowledgeGraphEngine, request_json: &str) -> String {
    let handler = McpHandler::new(engine);
    handler.handle_raw(request_json)
}
