//! Quota methods: describe

use crate::rpc_method;
use crate::serve::{JsonRpcResult, ServeContext};
use serde_json::Value;

async fn handle_quota_describe(_ctx: &ServeContext, _params: Value) -> JsonRpcResult {
    // Placeholder — no direct MCP tool for quota yet
    Ok(serde_json::json!({ "used": 0, "total": 0, "percentage": 0.0 }))
}

inventory::submit! { rpc_method!("quota/describe", handle_quota_describe) }
