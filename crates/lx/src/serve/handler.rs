//! JSON-RPC method dispatch
//!
//! Routes incoming requests using the inventory-registered method table.
//! Falls through to MCP tool calls for unregistered methods (dynamic proxy).

use super::protocol::{JsonRpcError, JsonRpcRequest, JsonRpcResponse, JsonRpcResult};
use super::{RpcMethod, ServeContext, ServeState};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Dispatch a JSON-RPC request
pub async fn dispatch(
    request: JsonRpcRequest,
    state: &Arc<RwLock<ServeState>>,
) -> Option<JsonRpcResponse> {
    // Notifications (no id) don't get a response
    if request.is_notification() {
        return None;
    }

    let id = request.id;
    let ctx = ServeContext::new(state.clone());
    let result = dispatch_method(&request.method, request.params, &ctx).await;

    Some(match result {
        Ok(value) => JsonRpcResponse::ok(id, value),
        Err(error) => JsonRpcResponse::err(id, error),
    })
}

/// Route method: inventory table → MCP dynamic fallback
async fn dispatch_method(
    method: &str,
    params: serde_json::Value,
    ctx: &ServeContext,
) -> JsonRpcResult {
    // 1. Try registered handlers (compile-time, inventory-collected)
    for registered in inventory::iter::<RpcMethod> {
        if registered.name == method {
            return (registered.handler)(ctx, params).await;
        }
    }

    // 2. Dynamic MCP fallback: "domain/action" → "domain_action" tool call
    //    e.g., "block/listChildren" → MCP tool "block_list_block_children"
    //    This ensures new MCP tools are automatically available without code changes.
    if let Some(tool_name) = rpc_method_to_mcp_tool(method) {
        return ctx.mcp_call(&tool_name, params).await;
    }

    Err(JsonRpcError::method_not_found(method))
}

/// Convert a JSON-RPC method name to an MCP tool name
///
/// Strategy: "domain/action" → try multiple MCP tool name patterns
/// - "space/list" → try `space_list`, `space_list_spaces`, `space_list_recently_spaces`
/// - "entry/content" → try `entry_content`, `entry_describe_ai_parse_content`
///
/// For the simple case, we use `domain_action` as the base and let MCP resolve it.
/// A more sophisticated version could consult the MCP schema for exact names.
fn rpc_method_to_mcp_tool(method: &str) -> Option<String> {
    // Only convert methods with "/" — they look like our domain/method pattern
    if !method.contains('/') {
        return None;
    }

    // "domain/action" → "domain_action"
    let tool_name = method.replace('/', "_");

    // Skip lifecycle methods
    if matches!(method, "initialize" | "exit") {
        return None;
    }

    Some(tool_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_method_to_mcp_tool() {
        assert_eq!(
            rpc_method_to_mcp_tool("space/list"),
            Some("space_list".to_string())
        );
        assert_eq!(
            rpc_method_to_mcp_tool("entry/content"),
            Some("entry_content".to_string())
        );
        assert_eq!(
            rpc_method_to_mcp_tool("contact/whoami"),
            Some("contact_whoami".to_string())
        );
        // No "/" — not a domain method
        assert_eq!(rpc_method_to_mcp_tool("initialize"), None);
    }
}
