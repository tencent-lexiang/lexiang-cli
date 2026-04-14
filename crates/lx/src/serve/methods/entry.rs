//! Entry methods: tree, content, describe, create, rename, move, listChildren, syncContent

use crate::rpc_method;
use crate::serve::{error_codes, JsonRpcError, JsonRpcResult, ServeContext};
use serde_json::Value;

async fn handle_entry_describe(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let entry_id = ctx.require_str(&params, "entry_id")?;
    ctx.mcp_call(
        "entry_describe_entry",
        serde_json::json!({ "entry_id": entry_id }),
    )
    .await
}

async fn handle_entry_tree(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let space_id = ctx.require_str(&params, "space_id")?;
    let depth = params.get("depth").and_then(Value::as_u64).unwrap_or(3) as usize;

    let client = ctx.mcp_client().await?;

    let space_result = ctx
        .mcp_call_with(
            &client,
            "space_describe_space",
            serde_json::json!({ "space_id": space_id }),
        )
        .await?;

    let root_entry_id = space_result
        .pointer("/data/space/root_entry_id")
        .or_else(|| space_result.pointer("/data/root_entry_id"))
        .or_else(|| space_result.pointer("/space/root_entry_id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            JsonRpcError::new(error_codes::NOT_FOUND, "Cannot determine root_entry_id")
        })?;

    let tree = build_entry_tree(&client, ctx, root_entry_id, depth).await?;
    Ok(serde_json::json!({ "tree": tree }))
}

async fn build_entry_tree(
    client: &crate::mcp::McpClient,
    ctx: &ServeContext,
    parent_id: &str,
    remaining_depth: usize,
) -> JsonRpcResult {
    let result = ctx
        .mcp_call_with(
            client,
            "entry_list_children",
            serde_json::json!({ "parent_entry_id": parent_id }),
        )
        .await?;

    let entries = result
        .pointer("/data/entries")
        .or_else(|| result.get("entries"))
        .or_else(|| result.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if remaining_depth == 0 {
        let items: Vec<Value> = entries
            .into_iter()
            .map(|e| {
                let has_children = e
                    .get("has_children")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                serde_json::json!({
                    "id": e.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                    "name": e.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                    "entry_type": e.get("entry_type").and_then(|v| v.as_str()).unwrap_or("page"),
                    "has_children": has_children,
                })
            })
            .collect();
        return Ok(serde_json::json!(items));
    }

    let mut items = Vec::new();
    for entry in entries {
        let has_children = entry
            .get("has_children")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let entry_id = entry
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let mut item = serde_json::json!({
            "id": entry_id,
            "name": entry.get("name").and_then(|v| v.as_str()).unwrap_or(""),
            "entry_type": entry.get("entry_type").and_then(|v| v.as_str()).unwrap_or("page"),
            "has_children": has_children,
        });

        if has_children && remaining_depth > 0 {
            let children = Box::pin(build_entry_tree(
                client,
                ctx,
                &entry_id,
                remaining_depth - 1,
            ))
            .await?;
            item["children"] = children;
        }

        items.push(item);
    }

    Ok(serde_json::json!(items))
}

async fn handle_entry_content(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let entry_id = ctx.require_str(&params, "entry_id")?;
    let result = ctx
        .mcp_call(
            "entry_describe_ai_parse_content",
            serde_json::json!({ "entry_id": entry_id }),
        )
        .await?;
    Ok(serde_json::json!({ "entry_id": entry_id, "content": result }))
}

async fn handle_entry_create(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let entry_type = ctx.require_str(&params, "type")?;
    let parent_id = ctx.require_str(&params, "parent_id")?;
    let name = ctx.require_str(&params, "name")?;
    let result = ctx
        .mcp_call(
            "entry_create_entry",
            serde_json::json!({ "parent_entry_id": parent_id, "name": name, "entry_type": entry_type }),
        )
        .await?;
    Ok(serde_json::json!({ "entry": result }))
}

async fn handle_entry_rename(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let entry_id = ctx.require_str(&params, "entry_id")?;
    let name = ctx.require_str(&params, "name")?;
    let result = ctx
        .mcp_call(
            "entry_rename_entry",
            serde_json::json!({ "entry_id": entry_id, "name": name }),
        )
        .await?;
    Ok(serde_json::json!({ "entry": result }))
}

async fn handle_entry_move(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let entry_id = ctx.require_str(&params, "entry_id")?;
    let parent_id = ctx.require_str(&params, "parent_id")?;
    let result = ctx
        .mcp_call(
            "entry_move_entry",
            serde_json::json!({ "entry_id": entry_id, "parent_entry_id": parent_id }),
        )
        .await?;
    Ok(serde_json::json!({ "entry": result }))
}

async fn handle_entry_list_children(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let parent_id = ctx.require_str(&params, "parent_id")?;
    let result = ctx
        .mcp_call(
            "entry_list_children",
            serde_json::json!({ "parent_entry_id": parent_id }),
        )
        .await?;
    Ok(serde_json::json!({ "children": result }))
}

async fn handle_entry_sync_content(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let space_id = ctx.require_str(&params, "space_id")?;
    let entries = params
        .get("entries")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let force = params
        .get("force")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let mut succeeded = 0u32;
    let mut failed = 0u32;
    let mut errors: Vec<Value> = Vec::new();

    for entry in &entries {
        let entry_id = entry
            .get("entryId")
            .or_else(|| entry.get("entry_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let name = entry
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(entry_id);

        if entry_id.is_empty() {
            failed += 1;
            errors.push(serde_json::json!({
                "name": name,
                "error": "Missing entryId",
            }));
            continue;
        }

        // Fetch content via MCP
        let result = ctx
            .mcp_call(
                "entry_describe_ai_parse_content",
                serde_json::json!({ "entry_id": entry_id }),
            )
            .await;

        match result {
            Ok(_content) => {
                succeeded += 1;
            }
            Err(e) => {
                failed += 1;
                errors.push(serde_json::json!({
                    "name": name,
                    "error": e.to_string(),
                }));
            }
        }
    }

    Ok(serde_json::json!({
        "succeeded": succeeded,
        "failed": failed,
        "errors": errors,
        "space_id": space_id,
        "force": force,
    }))
}

inventory::submit! { rpc_method!("entry/tree", handle_entry_tree) }
inventory::submit! { rpc_method!("entry/content", handle_entry_content) }
inventory::submit! { rpc_method!("entry/describe", handle_entry_describe) }
inventory::submit! { rpc_method!("entry/syncContent", handle_entry_sync_content) }
inventory::submit! { rpc_method!("entry/create", handle_entry_create) }
inventory::submit! { rpc_method!("entry/rename", handle_entry_rename) }
inventory::submit! { rpc_method!("entry/move", handle_entry_move) }
inventory::submit! { rpc_method!("entry/listChildren", handle_entry_list_children) }
