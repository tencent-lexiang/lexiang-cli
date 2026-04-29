//! Auth methods: auth/status, auth/login, auth/startOAuth, auth/completeOAuth,
//!              auth/startClientLogin, auth/completeClientLogin, auth/logout

use crate::rpc_method;
use crate::serve::{error_codes, JsonRpcError, JsonRpcResult, ServeContext};
use serde_json::Value;

async fn handle_auth_status(_ctx: &ServeContext, _params: Value) -> JsonRpcResult {
    // 检查本地 token 文件是否存在且未过期
    let has_valid_token = match crate::auth::load_token() {
        Ok(Some(token)) => !crate::auth::is_expired_public(&token),
        _ => false,
    };

    // 判断认证类型
    let auth_type = if crate::auth::load_client_session().ok().flatten().is_some() {
        "client"
    } else {
        "oauth"
    };

    Ok(serde_json::json!({
        "authenticated": has_valid_token,
        "authType": auth_type,
    }))
}

async fn handle_auth_login(ctx: &ServeContext, _params: Value) -> JsonRpcResult {
    // RPC 模式下不需要 companyFrom，直接返回成功
    let client = ctx.mcp_client().await.map_err(|e| {
        JsonRpcError::new(
            error_codes::AUTH_REQUIRED,
            format!("Authentication failed: {}", e),
        )
    })?;

    // 尝试调用 whoami 来验证认证是否成功
    match ctx
        .mcp_call_with(&client, "contact_whoami", serde_json::json!({}))
        .await
    {
        Ok(user_data) => Ok(serde_json::json!({
            "success": true,
            "user": user_data,
        })),
        Err(e) => Err(JsonRpcError::new(
            error_codes::AUTH_REQUIRED,
            format!("Authentication failed: {}", e),
        )),
    }
}

/// 两阶段 OAuth 第一阶段：启动回调服务器，返回授权 URL
///
/// VS Code 扩展调用此方法获取 `authUrl`，然后用
/// `vscode.env.openExternal(authUrl)` 打开浏览器（兼容 Remote SSH），
/// 最后调用 `auth/completeOAuth` 等待完成。
async fn handle_auth_start_oauth(ctx: &ServeContext, _params: Value) -> JsonRpcResult {
    let (auth_url, pending) = crate::auth::login_start()
        .await
        .map_err(|e| JsonRpcError::new(error_codes::INTERNAL_ERROR, e.to_string()))?;

    // 存入共享状态
    {
        let mut state = ctx.state.write().await;
        state.pending_oauth = Some(pending);
    }

    Ok(serde_json::json!({
        "authUrl": auth_url,
    }))
}

/// 两阶段 OAuth 第二阶段：等待回调完成
///
/// VS Code 扩展在打开浏览器后调用此方法轮询结果。
/// 超时时间建议设为 120 秒。
async fn handle_auth_complete_oauth(ctx: &ServeContext, _params: Value) -> JsonRpcResult {
    let pending = {
        let state = ctx.state.read().await;
        state.pending_oauth.clone()
    };

    let Some(pending) = pending else {
        return Err(JsonRpcError::new(
            error_codes::AUTH_REQUIRED,
            "No pending OAuth flow. Call auth/startOAuth first.",
        ));
    };

    // 等待回调完成（最多 120 秒）
    let token_result =
        tokio::time::timeout(std::time::Duration::from_secs(120), pending.wait()).await;

    // 无论成功/失败/超时，都关闭回调服务器
    pending.abort_server();

    // 清除 pending 状态
    {
        let mut state = ctx.state.write().await;
        state.pending_oauth = None;
    }

    let token = token_result
        .map_err(|_| {
            JsonRpcError::new(
                error_codes::AUTH_REQUIRED,
                "OAuth flow timed out (120s). Please try again.",
            )
        })?
        .map_err(|e| {
            JsonRpcError::new(error_codes::AUTH_REQUIRED, format!("OAuth failed: {}", e))
        })?;

    // token 已在 handle_callback 中保存到文件，这里只需更新内存缓存
    {
        let mut state = ctx.state.write().await;
        state.access_token = Some(token.access_token.clone());
        state.cached_mcp_client = None; // token 变化，强制下次重建
        tracing::info!(
            token_len = token.access_token.len(),
            "auth/completeOAuth: mcp client cache cleared"
        );
    }

    Ok(serde_json::json!({
        "success": true,
    }))
}

/// 客户端登录：返回登录 URL
///
/// 可选参数 `redirectUrl`：登录完成后浏览器跳转的目标 URL。
/// VS Code 扩展可传入 `vscode://lexiang.lefs-vscode/auth-callback`
/// 以便浏览器回调自动被 VS Code URI handler 捕获。
async fn handle_auth_start_client_login(_ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let redirect_url = params.get("redirectUrl").and_then(|v| v.as_str());
    Ok(serde_json::json!({
        "authUrl": crate::auth::client_login_url(redirect_url),
    }))
}

/// 客户端登录：用回调 URL 完成登录
async fn handle_auth_complete_client_login(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let callback = params
        .get("callbackUrl")
        .or_else(|| params.get("code"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing callbackUrl or code"))?;

    let token = crate::auth::login_with_client_callback(callback)
        .await
        .map_err(|e| {
            JsonRpcError::new(
                error_codes::AUTH_REQUIRED,
                format!("Client login failed: {e}"),
            )
        })?;

    {
        let mut state = ctx.state.write().await;
        state.access_token = Some(token.access_token.clone());
        state.cached_mcp_client = None;
    }

    Ok(serde_json::json!({ "success": true }))
}

/// 登出：删除本地 token 和 session
async fn handle_auth_logout(ctx: &ServeContext, _params: Value) -> JsonRpcResult {
    crate::auth::logout()
        .map_err(|e| JsonRpcError::new(error_codes::INTERNAL_ERROR, e.to_string()))?;

    // 清除内存中的 token 和缓存的 MCP client
    {
        let mut state = ctx.state.write().await;
        state.access_token = None;
        state.pending_oauth = None;
        state.cached_mcp_client = None;
    }

    Ok(serde_json::json!({
        "success": true,
    }))
}

inventory::submit! { rpc_method!("auth/status", handle_auth_status) }
inventory::submit! { rpc_method!("auth/login", handle_auth_login) }
inventory::submit! { rpc_method!("auth/startOAuth", handle_auth_start_oauth) }
inventory::submit! { rpc_method!("auth/completeOAuth", handle_auth_complete_oauth) }
inventory::submit! { rpc_method!("auth/startClientLogin", handle_auth_start_client_login) }
inventory::submit! { rpc_method!("auth/completeClientLogin", handle_auth_complete_client_login) }
inventory::submit! { rpc_method!("auth/logout", handle_auth_logout) }
