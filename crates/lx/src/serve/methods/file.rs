//! File methods: download

use crate::rpc_method;
use crate::serve::{JsonRpcResult, ServeContext};
use serde_json::Value;

async fn handle_file_download(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let file_id = ctx.require_str(&params, "file_id")?;
    let result = ctx
        .mcp_call(
            "file_download_file",
            serde_json::json!({ "file_id": file_id }),
        )
        .await?;
    Ok(serde_json::json!({ "url": result }))
}

inventory::submit! { rpc_method!("file/download", handle_file_download) }
