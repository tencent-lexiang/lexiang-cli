//! Lifecycle methods: initialize, exit

use crate::rpc_method;
use crate::serve::{JsonRpcResult, RpcMethod, ServeContext};
use serde_json::Value;

async fn handle_initialize(_ctx: &ServeContext, _params: Value) -> JsonRpcResult {
    let methods: Vec<&str> = inventory::iter::<RpcMethod>
        .into_iter()
        .map(|m| m.name)
        .collect();
    Ok(serde_json::json!({
        "server": "lx",
        "version": env!("CARGO_PKG_VERSION"),
        "capabilities": {
            "methods": methods,
            "notifications": ["space/changed", "space/syncProgress", "auth/expired"],
            "dynamicFallback": true,
        }
    }))
}

inventory::submit! { rpc_method!("initialize", handle_initialize) }
