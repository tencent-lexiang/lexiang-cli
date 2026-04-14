//! Auth methods: auth/status, auth/login

use crate::rpc_method;
use crate::serve::{error_codes, JsonRpcError, JsonRpcResult, ServeContext};
use serde_json::Value;

async fn handle_auth_status(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let company_from = params.get("companyFrom").and_then(|v| v.as_str());

    let Ok(client) = ctx.mcp_client().await else {
        return Ok(serde_json::json!({
            "authenticated": false,
            "companyFrom": company_from,
        }));
    };

    match ctx
        .mcp_call_with(&client, "contact_whoami", serde_json::json!({}))
        .await
    {
        Ok(user_data) => {
            // 构建 mcpUrl（如果 company_from 已知）
            let mcp_url = company_from.map(|cf| format!("https://{}.lexiangla.com/api/mcp", cf));

            Ok(serde_json::json!({
                "authenticated": true,
                "user": user_data,
                "companyFrom": company_from,
                "mcpUrl": mcp_url,
            }))
        }
        Err(_) => Ok(serde_json::json!({
            "authenticated": false,
            "companyFrom": company_from,
        })),
    }
}

async fn handle_auth_login(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let company_from = ctx.require_str(&params, "companyFrom")?;

    // 使用 lx login 命令进行认证
    // 通过 MCP 的 ensure_authenticated 机制触发浏览器登录
    let client = match ctx.mcp_client_for(company_from).await {
        Ok(c) => c,
        Err(e) => {
            return Err(JsonRpcError::new(
                error_codes::AUTH_REQUIRED,
                format!("Failed to create MCP client for {}: {}", company_from, e),
            ));
        }
    };

    // 尝试调用 whoami 来验证认证是否成功
    match ctx
        .mcp_call_with(&client, "contact_whoami", serde_json::json!({}))
        .await
    {
        Ok(user_data) => {
            let mcp_url = format!("https://{}.lexiangla.com/api/mcp", company_from);
            Ok(serde_json::json!({
                "success": true,
                "mcpUrl": mcp_url,
                "user": user_data,
            }))
        }
        Err(e) => Err(JsonRpcError::new(
            error_codes::AUTH_REQUIRED,
            format!("Authentication failed: {}", e),
        )),
    }
}

inventory::submit! { rpc_method!("auth/status", handle_auth_status) }
inventory::submit! { rpc_method!("auth/login", handle_auth_login) }
