//! Search methods: kb, search (alias)

use crate::rpc_method;
use crate::serve::{JsonRpcResult, ServeContext};
use serde_json::Value;

async fn handle_search_kb(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let keyword = ctx.require_str(&params, "keyword")?;
    let result = ctx
        .mcp_call(
            "search_kb_search",
            serde_json::json!({ "keyword": keyword, "type": "`kb_doc`" }),
        )
        .await?;
    Ok(serde_json::json!({ "results": result }))
}

/// General search — supports type parameter (space, `kb_doc`, doc, team, all)
async fn handle_search(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let keyword = ctx.require_str(&params, "keyword")?;
    let search_type = params
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("`kb_doc`");
    let limit = params.get("limit").and_then(Value::as_u64).unwrap_or(30);

    ctx.mcp_call(
        "search_kb_search",
        serde_json::json!({ "keyword": keyword, "type": search_type, "limit": limit }),
    )
    .await
}

inventory::submit! { rpc_method!("search/kb", handle_search_kb) }
inventory::submit! { rpc_method!("search", handle_search) }
