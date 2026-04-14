//! Contact methods: whoami, searchStaff

use crate::rpc_method;
use crate::serve::{JsonRpcResult, ServeContext};
use serde_json::Value;

async fn handle_contact_whoami(ctx: &ServeContext, _params: Value) -> JsonRpcResult {
    let result = ctx
        .mcp_call("contact_whoami", serde_json::json!({}))
        .await?;
    Ok(serde_json::json!({ "user": result }))
}

async fn handle_contact_search_staff(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let keyword = ctx.require_str(&params, "keyword")?;
    let result = ctx
        .mcp_call(
            "contact_search_staff",
            serde_json::json!({ "keyword": keyword }),
        )
        .await?;
    Ok(serde_json::json!({ "staff": result }))
}

inventory::submit! { rpc_method!("contact/whoami", handle_contact_whoami) }
inventory::submit! { rpc_method!("contact/searchStaff", handle_contact_search_staff) }
