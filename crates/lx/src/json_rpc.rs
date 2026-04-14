use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    pub id: i64,
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct JsonRpcResponse<T> {
    pub jsonrpc: String,
    pub id: i64,
    #[serde(default)]
    pub result: Option<T>,
    #[serde(default)]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcRequest {
    pub fn new(method: impl Into<String>, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id: chrono::Utc::now().timestamp_millis(),
            method: method.into(),
            params,
        }
    }
}
