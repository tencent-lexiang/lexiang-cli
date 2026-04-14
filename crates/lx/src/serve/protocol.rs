//! JSON-RPC 2.0 protocol types for `lx serve`
//!
//! Extends the existing `json_rpc.rs` (which is client-side for MCP calls)
//! with server-side types needed for `lx serve`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 request (received from client via stdin)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<i64>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// JSON-RPC 2.0 response (sent to client via stdout)
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// JSON-RPC 2.0 notification (server-initiated, sent to client via stdout)
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: &'static str,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// Convenience type for method handler results
pub type JsonRpcResult = Result<Value, JsonRpcError>;

impl JsonRpcResponse {
    /// Create a success response
    pub fn ok(id: Option<i64>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response
    pub fn err(id: Option<i64>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(error),
        }
    }
}

impl JsonRpcError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(code: i32, message: impl Into<String>, data: Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }

    /// Standard JSON-RPC errors
    pub fn method_not_found(method: &str) -> Self {
        Self::new(-32601, format!("Method not found: {method}"))
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(-32602, message)
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(-32603, message)
    }
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JsonRpcError({}, {})", self.code, self.message)
    }
}

impl std::error::Error for JsonRpcError {}

impl JsonRpcNotification {
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            method: method.into(),
            params,
        }
    }
}

impl JsonRpcRequest {
    /// Returns true if this is a notification (no id field, or id is null)
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }
}
