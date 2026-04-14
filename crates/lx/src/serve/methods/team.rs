//! Team methods: list, listFrequent

use crate::rpc_method;
use crate::serve::{JsonRpcResult, ServeContext};
use serde_json::Value;

async fn handle_team_list(ctx: &ServeContext, _params: Value) -> JsonRpcResult {
    let result = ctx
        .mcp_call("team_list_teams", serde_json::json!({}))
        .await?;
    Ok(serde_json::json!({ "teams": result }))
}

async fn handle_team_list_frequent(ctx: &ServeContext, _params: Value) -> JsonRpcResult {
    let result = ctx
        .mcp_call("team_list_frequent_teams", serde_json::json!({}))
        .await?;
    Ok(serde_json::json!({ "teams": result }))
}

inventory::submit! { rpc_method!("team/list", handle_team_list) }
inventory::submit! { rpc_method!("team/listFrequent", handle_team_list_frequent) }
