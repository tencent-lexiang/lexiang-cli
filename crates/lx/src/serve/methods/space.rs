//! Space methods: list, listRecent, listByTeam, describe, mount, unmount, sync, changes

use crate::rpc_method;
use crate::serve::{JsonRpcResult, ServeContext};
use serde_json::Value;

async fn handle_space_list(ctx: &ServeContext, _params: Value) -> JsonRpcResult {
    let result = ctx
        .mcp_call("space_list_recently_spaces", serde_json::json!({}))
        .await?;
    Ok(serde_json::json!({ "spaces": result }))
}

async fn handle_space_list_recent(ctx: &ServeContext, _params: Value) -> JsonRpcResult {
    let result = ctx
        .mcp_call("space_list_recently_spaces", serde_json::json!({}))
        .await?;
    Ok(serde_json::json!({ "spaces": result }))
}

async fn handle_space_list_by_team(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let team_id = ctx.require_str(&params, "team_id")?;
    let result = ctx
        .mcp_call(
            "space_list_spaces",
            serde_json::json!({ "team_id": team_id }),
        )
        .await?;
    Ok(serde_json::json!({ "spaces": result }))
}

async fn handle_space_describe(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let space_id = ctx.require_str(&params, "space_id")?;
    ctx.mcp_call(
        "space_describe_space",
        serde_json::json!({ "space_id": space_id }),
    )
    .await
}

async fn handle_space_mount(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let space_id = ctx.require_str(&params, "space_id")?;
    // Full implementation will start git clone + background sync
    Ok(serde_json::json!({ "status": "mounted", "space_id": space_id }))
}

async fn handle_space_unmount(_ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let space_id = params
        .get("space_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::serve::JsonRpcError::invalid_params("Missing space_id"))?;
    Ok(serde_json::json!({ "status": "unmounted", "space_id": space_id }))
}

async fn handle_space_sync(_ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let space_id = params
        .get("space_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::serve::JsonRpcError::invalid_params("Missing space_id"))?;

    // Placeholder — will integrate with lx git pull
    // For now, just report sync completed
    Ok(serde_json::json!({
        "synced": true,
        "entryCount": 0,
        "space_id": space_id,
    }))
}

async fn handle_space_changes(_ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let space_id = params
        .get("space_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::serve::JsonRpcError::invalid_params("Missing space_id"))?;

    // Placeholder — check if remote has changes
    // For now, always report no changes
    Ok(serde_json::json!({
        "hasChanges": false,
        "space_id": space_id,
    }))
}

inventory::submit! { rpc_method!("space/list", handle_space_list) }
inventory::submit! { rpc_method!("space/listRecent", handle_space_list_recent) }
inventory::submit! { rpc_method!("space/listByTeam", handle_space_list_by_team) }
inventory::submit! { rpc_method!("space/describe", handle_space_describe) }
inventory::submit! { rpc_method!("space/mount", handle_space_mount) }
inventory::submit! { rpc_method!("space/unmount", handle_space_unmount) }
inventory::submit! { rpc_method!("space/sync", handle_space_sync) }
inventory::submit! { rpc_method!("space/changes", handle_space_changes) }
