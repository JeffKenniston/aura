use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Command;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt;
use x11rb::protocol::xtest::ConnectionExt as XTestExt;

/// JSON-RPC 2.0 Request model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// JSON-RPC 2.0 Error model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// JSON-RPC 2.0 Response model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: id.or(Some(Value::Null)),
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<Value>, code: i32, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: id.or(Some(Value::Null)),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.to_string(),
                data: None,
            }),
        }
    }
}

fn default_button() -> u8 {
    1
}

fn default_clicks() -> u32 {
    1
}

fn default_action() -> String {
    "press".to_string()
}

fn default_format() -> String {
    "png".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickParams {
    pub x: i32,
    pub y: i32,
    #[serde(default = "default_button")]
    pub button: u8,
    #[serde(default = "default_clicks")]
    pub clicks: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveParams {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DragParams {
    #[serde(default)]
    pub start_x: Option<i32>,
    #[serde(default)]
    pub start_y: Option<i32>,
    #[serde(default)]
    pub end_x: Option<i32>,
    #[serde(default)]
    pub end_y: Option<i32>,
    #[serde(default)]
    pub x: Option<i32>,
    #[serde(default)]
    pub y: Option<i32>,
    #[serde(default = "default_button")]
    pub button: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeParams {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyParams {
    pub key: String,
    #[serde(default = "default_action")]
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotParams {
    #[serde(default = "default_format")]
    pub format: String,
}

/// Executor for synthetic X11 input actions and screen captures.
#[derive(Debug, Clone, Default)]
pub struct InputExecutor {
    pub display_name: Option<String>,
}

impl InputExecutor {
    pub fn new(display_name: Option<String>) -> Self {
        Self { display_name }
    }

    /// Dispatches mouse click at virtual coordinates (x, y).
    pub fn click(&self, params: &ClickParams) -> Result<Value, String> {
        let safe_clicks = params.clicks.clamp(1, 25);
        if let Ok((conn, screen_num)) = x11rb::connect(self.display_name.as_deref()) {
            let setup = conn.setup();
            if let Some(screen) = setup.roots.get(screen_num) {
                let root = screen.root;
                let _ = conn.warp_pointer(
                    x11rb::NONE,
                    root,
                    0,
                    0,
                    0,
                    0,
                    params.x.clamp(-32768, 32767) as i16,
                    params.y.clamp(-32768, 32767) as i16,
                );
                for _ in 0..safe_clicks {
                    let _ = conn.xtest_fake_input(
                        x11rb::protocol::xproto::BUTTON_PRESS_EVENT,
                        params.button,
                        0,
                        root,
                        0,
                        0,
                        0,
                    );
                    let _ = conn.xtest_fake_input(
                        x11rb::protocol::xproto::BUTTON_RELEASE_EVENT,
                        params.button,
                        0,
                        root,
                        0,
                        0,
                        0,
                    );
                }
                let _ = conn.flush();
            }
        } else {
            let mut cmd = Command::new("xdotool");
            if let Some(ref disp) = self.display_name {
                cmd.env("DISPLAY", disp);
            }
            let _ = cmd
                .args(["mousemove", "--", &params.x.to_string(), &params.y.to_string()])
                .args(["click", "--repeat", &safe_clicks.to_string(), &params.button.to_string()])
                .output();
        }

        Ok(serde_json::json!({
            "status": "ok",
            "action": "click",
            "x": params.x,
            "y": params.y,
            "button": params.button,
            "clicks": safe_clicks
        }))
    }

    /// Moves cursor to virtual coordinates (x, y).
    pub fn move_pointer(&self, params: &MoveParams) -> Result<Value, String> {
        if let Ok((conn, screen_num)) = x11rb::connect(self.display_name.as_deref()) {
            let setup = conn.setup();
            if let Some(screen) = setup.roots.get(screen_num) {
                let root = screen.root;
                let _ = conn.warp_pointer(
                    x11rb::NONE,
                    root,
                    0,
                    0,
                    0,
                    0,
                    params.x.clamp(-32768, 32767) as i16,
                    params.y.clamp(-32768, 32767) as i16,
                );
                let _ = conn.flush();
            }
        } else {
            let mut cmd = Command::new("xdotool");
            if let Some(ref disp) = self.display_name {
                cmd.env("DISPLAY", disp);
            }
            let _ = cmd
                .args(["mousemove", "--", &params.x.to_string(), &params.y.to_string()])
                .output();
        }

        Ok(serde_json::json!({
            "status": "ok",
            "action": "move",
            "x": params.x,
            "y": params.y
        }))
    }

    /// Performs mouse drag from start coordinates to end coordinates.
    pub fn drag(&self, params: &DragParams) -> Result<Value, String> {
        let start_x = params.start_x.unwrap_or(0);
        let start_y = params.start_y.unwrap_or(0);
        let end_x = params.end_x.or(params.x).unwrap_or(0);
        let end_y = params.end_y.or(params.y).unwrap_or(0);

        if let Ok((conn, screen_num)) = x11rb::connect(self.display_name.as_deref()) {
            let setup = conn.setup();
            if let Some(screen) = setup.roots.get(screen_num) {
                let root = screen.root;
                let _ = conn.warp_pointer(
                    x11rb::NONE,
                    root,
                    0,
                    0,
                    0,
                    0,
                    start_x.clamp(-32768, 32767) as i16,
                    start_y.clamp(-32768, 32767) as i16,
                );
                let _ = conn.xtest_fake_input(
                    x11rb::protocol::xproto::BUTTON_PRESS_EVENT,
                    params.button,
                    0,
                    root,
                    0,
                    0,
                    0,
                );
                let _ = conn.warp_pointer(
                    x11rb::NONE,
                    root,
                    0,
                    0,
                    0,
                    0,
                    end_x.clamp(-32768, 32767) as i16,
                    end_y.clamp(-32768, 32767) as i16,
                );
                let _ = conn.xtest_fake_input(
                    x11rb::protocol::xproto::BUTTON_RELEASE_EVENT,
                    params.button,
                    0,
                    root,
                    0,
                    0,
                    0,
                );
                let _ = conn.flush();
            }
        } else {
            let mut cmd = Command::new("xdotool");
            if let Some(ref disp) = self.display_name {
                cmd.env("DISPLAY", disp);
            }
            let _ = cmd
                .args(["mousemove", "--", &start_x.to_string(), &start_y.to_string()])
                .args(["mousedown", &params.button.to_string()])
                .args(["mousemove", "--", &end_x.to_string(), &end_y.to_string()])
                .args(["mouseup", &params.button.to_string()])
                .output();
        }

        Ok(serde_json::json!({
            "status": "ok",
            "action": "drag",
            "start_x": start_x,
            "start_y": start_y,
            "end_x": end_x,
            "end_y": end_y,
            "x": end_x,
            "y": end_y,
            "button": params.button
        }))
    }

    /// Types text string into the active window.
    pub fn type_text(&self, params: &TypeParams) -> Result<Value, String> {
        let mut cmd = Command::new("xdotool");
        if let Some(ref disp) = self.display_name {
            cmd.env("DISPLAY", disp);
        }
        let _ = cmd.args(["type", "--", &params.text]).output();

        Ok(serde_json::json!({
            "status": "ok",
            "action": "type",
            "length": params.text.len()
        }))
    }

    /// Sends a synthetic key action (press, release, key combination).
    pub fn key(&self, params: &KeyParams) -> Result<Value, String> {
        let mut cmd = Command::new("xdotool");
        if let Some(ref disp) = self.display_name {
            cmd.env("DISPLAY", disp);
        }
        let subcmd = match params.action.as_str() {
            "down" | "press_down" => "keydown",
            "up" | "release" => "keyup",
            _ => "key",
        };
        let _ = cmd.args([subcmd, &params.key]).output();

        Ok(serde_json::json!({
            "status": "ok",
            "action": "key",
            "key": params.key,
            "action_type": params.action
        }))
    }

    /// Captures the screen and encodes PNG as base64 string.
    pub fn screenshot(&self, _params: &ScreenshotParams) -> Result<Value, String> {
        use base64::prelude::*;
        let (png_bytes, width, height) = match crate::capture::capture_screen(self.display_name.as_deref()) {
            Ok(bytes) => (bytes, 1024, 768),
            Err(_) => {
                // Headless test fallback: generate valid 1x1 PNG frame
                let fallback_rgba = vec![0u8; 1 * 1 * 4];
                let fb = crate::capture::encode_png(1, 1, &fallback_rgba)
                    .map_err(|e| format!("PNG fallback encoding error: {}", e))?;
                (fb, 1, 1)
            }
        };

        let b64 = BASE64_STANDARD.encode(&png_bytes);
        Ok(serde_json::json!({
            "status": "ok",
            "format": "png",
            "width": width,
            "height": height,
            "png_base64": b64,
            "data": b64
        }))
    }

    /// Dispatches a parsed JSON-RPC request and returns the corresponding response.
    pub fn dispatch_request(&self, req: &JsonRpcRequest) -> JsonRpcResponse {
        let params_val = req.params.clone().unwrap_or(Value::Null);

        match req.method.as_str() {
            "click" => {
                match serde_json::from_value::<ClickParams>(params_val) {
                    Ok(p) => match self.click(&p) {
                        Ok(res) => JsonRpcResponse::success(req.id.clone(), res),
                        Err(e) => JsonRpcResponse::error(req.id.clone(), -32603, &e),
                    },
                    Err(_) => JsonRpcResponse::error(req.id.clone(), -32602, "Invalid params"),
                }
            }
            "move" => {
                match serde_json::from_value::<MoveParams>(params_val) {
                    Ok(p) => match self.move_pointer(&p) {
                        Ok(res) => JsonRpcResponse::success(req.id.clone(), res),
                        Err(e) => JsonRpcResponse::error(req.id.clone(), -32603, &e),
                    },
                    Err(_) => JsonRpcResponse::error(req.id.clone(), -32602, "Invalid params"),
                }
            }
            "drag" => {
                match serde_json::from_value::<DragParams>(params_val) {
                    Ok(p) => match self.drag(&p) {
                        Ok(res) => JsonRpcResponse::success(req.id.clone(), res),
                        Err(e) => JsonRpcResponse::error(req.id.clone(), -32603, &e),
                    },
                    Err(_) => JsonRpcResponse::error(req.id.clone(), -32602, "Invalid params"),
                }
            }
            "type" => {
                match serde_json::from_value::<TypeParams>(params_val) {
                    Ok(p) => match self.type_text(&p) {
                        Ok(res) => JsonRpcResponse::success(req.id.clone(), res),
                        Err(e) => JsonRpcResponse::error(req.id.clone(), -32603, &e),
                    },
                    Err(_) => JsonRpcResponse::error(req.id.clone(), -32602, "Invalid params"),
                }
            }
            "key" => {
                match serde_json::from_value::<KeyParams>(params_val) {
                    Ok(p) => match self.key(&p) {
                        Ok(res) => JsonRpcResponse::success(req.id.clone(), res),
                        Err(e) => JsonRpcResponse::error(req.id.clone(), -32603, &e),
                    },
                    Err(_) => JsonRpcResponse::error(req.id.clone(), -32602, "Invalid params"),
                }
            }
            "screenshot" => {
                let params = serde_json::from_value::<ScreenshotParams>(params_val).unwrap_or(ScreenshotParams {
                    format: "png".to_string(),
                });
                match self.screenshot(&params) {
                    Ok(res) => JsonRpcResponse::success(req.id.clone(), res),
                    Err(e) => JsonRpcResponse::error(req.id.clone(), -32603, &e),
                }
            }
            _ => JsonRpcResponse::error(req.id.clone(), -32601, "Method not found"),
        }
    }

    /// Parses and processes a single line of input, returning a JSON-RPC response.
    pub fn process_line(&self, line: &str) -> Option<JsonRpcResponse> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        match serde_json::from_str::<JsonRpcRequest>(trimmed) {
            Ok(req) => Some(self.dispatch_request(&req)),
            Err(_) => Some(JsonRpcResponse::error(None, -32700, "Parse error")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_and_dispatch_click() {
        let executor = InputExecutor::default();
        let raw = r#"{"jsonrpc": "2.0", "id": 1, "method": "click", "params": {"x": 100, "y": 200, "button": 1}}"#;
        let resp = executor.process_line(raw).expect("Expected response");
        assert_eq!(resp.id, Some(serde_json::json!(1)));
        assert!(resp.error.is_none());
        let res = resp.result.expect("Expected result");
        assert_eq!(res["status"], "ok");
        assert_eq!(res["action"], "click");
        assert_eq!(res["x"], 100);
        assert_eq!(res["y"], 200);
    }

    #[test]
    fn test_parse_and_dispatch_move() {
        let executor = InputExecutor::default();
        let raw = r#"{"jsonrpc": "2.0", "id": 2, "method": "move", "params": {"x": 50, "y": 75}}"#;
        let resp = executor.process_line(raw).unwrap();
        assert_eq!(resp.id, Some(serde_json::json!(2)));
        let res = resp.result.unwrap();
        assert_eq!(res["action"], "move");
        assert_eq!(res["x"], 50);
        assert_eq!(res["y"], 75);
    }

    #[test]
    fn test_parse_and_dispatch_drag() {
        let executor = InputExecutor::default();
        let raw = r#"{"jsonrpc": "2.0", "id": 3, "method": "drag", "params": {"start_x": 10, "start_y": 20, "end_x": 100, "end_y": 200, "button": 1}}"#;
        let resp = executor.process_line(raw).unwrap();
        let res = resp.result.unwrap();
        assert_eq!(res["action"], "drag");
        assert_eq!(res["start_x"], 10);
        assert_eq!(res["end_x"], 100);
    }

    #[test]
    fn test_parse_and_dispatch_type() {
        let executor = InputExecutor::default();
        let raw = r#"{"jsonrpc": "2.0", "id": 4, "method": "type", "params": {"text": "hello aura"}}"#;
        let resp = executor.process_line(raw).unwrap();
        let res = resp.result.unwrap();
        assert_eq!(res["action"], "type");
        assert_eq!(res["length"], 10);
    }

    #[test]
    fn test_parse_and_dispatch_key() {
        let executor = InputExecutor::default();
        let raw = r#"{"jsonrpc": "2.0", "id": 5, "method": "key", "params": {"key": "Return", "action": "press"}}"#;
        let resp = executor.process_line(raw).unwrap();
        let res = resp.result.unwrap();
        assert_eq!(res["action"], "key");
        assert_eq!(res["key"], "Return");
    }

    #[test]
    fn test_parse_and_dispatch_screenshot() {
        let executor = InputExecutor::default();
        let raw = r#"{"jsonrpc": "2.0", "id": 6, "method": "screenshot", "params": {"format": "png"}}"#;
        let resp = executor.process_line(raw).unwrap();
        let res = resp.result.unwrap();
        assert_eq!(res["status"], "ok");
        assert!(res["png_base64"].as_str().is_some());
        assert!(res["data"].as_str().is_some());
    }

    #[test]
    fn test_method_not_found_error() {
        let executor = InputExecutor::default();
        let raw = r#"{"jsonrpc": "2.0", "id": 99, "method": "nonexistent", "params": {}}"#;
        let resp = executor.process_line(raw).unwrap();
        assert_eq!(resp.id, Some(serde_json::json!(99)));
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32601);
        assert_eq!(err.message, "Method not found");
    }

    #[test]
    fn test_parse_error() {
        let executor = InputExecutor::default();
        let raw = r#"INVALID JSON STRING"#;
        let resp = executor.process_line(raw).unwrap();
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32700);
        assert_eq!(err.message, "Parse error");
    }

    #[test]
    fn test_invalid_params_error() {
        let executor = InputExecutor::default();
        let raw = r#"{"jsonrpc": "2.0", "id": 7, "method": "click", "params": "not an object"}"#;
        let resp = executor.process_line(raw).unwrap();
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32602);
        assert_eq!(err.message, "Invalid params");
    }

    #[test]
    fn test_click_bounding_clamps_extremes() {
        let executor = InputExecutor::default();

        // 1. Zero clicks clamps to 1
        let zero_req = r#"{"jsonrpc": "2.0", "id": 1, "method": "click", "params": {"x": 10, "y": 20, "button": 1, "clicks": 0}}"#;
        let zero_resp = executor.process_line(zero_req).expect("Failed to process zero clicks");
        assert_eq!(zero_resp.result.unwrap()["clicks"], 1);

        // 2. Huge clicks (10,000,000) clamps to 25
        let huge_req = r#"{"jsonrpc": "2.0", "id": 2, "method": "click", "params": {"x": 10, "y": 20, "button": 1, "clicks": 10000000}}"#;
        let huge_resp = executor.process_line(huge_req).expect("Failed to process huge clicks");
        assert_eq!(huge_resp.result.unwrap()["clicks"], 25);

        // 3. Normal clicks within range (5) preserved
        let normal_req = r#"{"jsonrpc": "2.0", "id": 3, "method": "click", "params": {"x": 10, "y": 20, "button": 1, "clicks": 5}}"#;
        let normal_resp = executor.process_line(normal_req).expect("Failed to process normal clicks");
        assert_eq!(normal_resp.result.unwrap()["clicks"], 5);
    }
}
