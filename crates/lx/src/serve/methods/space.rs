//! Space methods: list, listRecent, listByTeam, describe, mine, mount, unmount, sync, changes

use crate::rpc_method;
use crate::serve::{JsonRpcResult, ServeContext};
use serde_json::Value;

async fn handle_space_list(ctx: &ServeContext, _params: Value) -> JsonRpcResult {
    let result = ctx
        .mcp_call("space_list_recently_spaces", serde_json::json!({}))
        .await?;
    // mcp_call_with 已经处理了 { code, message, data } 格式，这里 result 就是 data
    // data 是 { spaces, visits }，需要提取 spaces
    let spaces = result
        .get("spaces")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    Ok(serde_json::json!({ "spaces": spaces }))
}

async fn handle_space_list_recent(ctx: &ServeContext, _params: Value) -> JsonRpcResult {
    let result = ctx
        .mcp_call("space_list_recently_spaces", serde_json::json!({}))
        .await?;
    let spaces = result
        .get("spaces")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    Ok(serde_json::json!({ "spaces": spaces }))
}

async fn handle_space_list_by_team(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let team_id = ctx.require_str(&params, "team_id")?;
    let result = ctx
        .mcp_call(
            "space_list_spaces",
            serde_json::json!({ "team_id": team_id }),
        )
        .await?;
    let spaces = result
        .get("spaces")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    Ok(serde_json::json!({ "spaces": spaces }))
}

async fn handle_space_describe(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let space_id = ctx.require_str(&params, "space_id")?;
    let result = ctx
        .mcp_call(
            "space_describe_space",
            serde_json::json!({ "space_id": space_id }),
        )
        .await?;
    // mcp_call_with 已经处理了 { code, message, data } 格式，这里 result 就是 data
    // data 是 { space: {...}, config: {...}, is_personal: ... }
    // 提取 space 返回给前端
    let space = result.get("space").cloned().unwrap_or(result);
    Ok(space)
}

async fn handle_space_mine(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let is_async = params
        .get("is_async")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let result = ctx
        .mcp_call(
            "space_describe_personal_space",
            serde_json::json!({ "is_async": is_async }),
        )
        .await?;

    // 正常返回: { space: { id, name, root_entry_id, ... } } → 提取 space
    // 异步创建: { task_id: "...", is_creating: true } → 直接透传
    if let Some(space) = result.get("space") {
        Ok(space.clone())
    } else {
        Ok(result)
    }
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
inventory::submit! { rpc_method!("space/mine", handle_space_mine) }
inventory::submit! { rpc_method!("space/mount", handle_space_mount) }
inventory::submit! { rpc_method!("space/unmount", handle_space_unmount) }
inventory::submit! { rpc_method!("space/sync", handle_space_sync) }
inventory::submit! { rpc_method!("space/changes", handle_space_changes) }
