//! 认证模块 —— OAuth 登录、Token 存储与刷新
//!
//! 对外暴露：
//!   - `login()`           — OAuth 2.0 登录（CLI 一次性流程）
//!   - `login_start()`     — OAuth 两阶段：启动回调服务器，返回授权 URL
//!   - `login_wait()`      — OAuth 两阶段：等待回调完成，返回 token
//!   - `logout()`          — 登出（删除本地 token）
//!   - `get_access_token`  — 获取有效 token（自动刷新）

use crate::config::Config;
use crate::datadir;
use anyhow::Result;
use axum::extract::Query;
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use oauth2::{CsrfToken, PkceCodeChallenge, PkceCodeVerifier};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

// ═══════════════════════════════════════════════════════════
//  常量
// ═══════════════════════════════════════════════════════════

const WELL_KNOWN_URL: &str = "https://mcp.lexiang-app.com/.well-known/oauth-authorization-server";
const CALLBACK_START_PORT: u16 = 18765;
const CALLBACK_MAX_PORT_ATTEMPTS: u16 = 50;
const TOKEN_FILE: &str = "token.json";

// ── 客户端登录常量 ──────────────────────────────────────────────────────
const CLIENT_AUTH_LOGIN_URL: &str = "https://lexiangla.com/tapi/account/v1/auth_login";
const CLIENT_MCP_TOKENS_URL: &str = "https://lexiangla.com/sapi/account/mcp/v1/tokens";
const SESSION_FILE: &str = "session.json";
const PENDING_LOGIN_FILE: &str = "pending-login.json";
const CALLBACK_URL_FILE: &str = "client-callback-url.txt";

// ═══════════════════════════════════════════════════════════
//  公开数据结构
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenData {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    /// 动态注册获得的 `client_id`，refresh 时需要
    pub client_id: Option<String>,
}

/// 客户端登录获得的 Cookie session 数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientSessionData {
    pub cookie: String,
    pub created_at: i64,
}

// ═══════════════════════════════════════════════════════════
//  内部数据结构（OAuth 协议）
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
struct OAuthServerConfig {
    authorization_endpoint: String,
    token_endpoint: String,
    registration_endpoint: String,
    scopes_supported: Vec<String>,
}

/// 简化版 —— 仅在 refresh 时使用，只需 `token_endpoint`
#[derive(Debug, Deserialize)]
struct OAuthServerConfigMinimal {
    token_endpoint: String,
}

#[derive(Debug, Deserialize)]
struct ClientRegistration {
    client_id: String,
    client_secret: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: String,
    state: String,
}

/// 进行中的 OAuth 状态（两阶段登录）
///
/// `auth/startOAuth` 创建，回调服务器写入完成信号，
/// `auth/completeOAuth` 等待并读取结果。
pub struct PendingOAuth {
    /// 期望的 CSRF state（用于验证回调）
    pub expected_state: String,
    /// 回调服务器端口
    pub callback_port: u16,
    /// OAuth 配置（token exchange 需要）
    oauth_config: OAuthServerConfig,
    /// 动态注册的客户端信息
    client_registration: ClientRegistration,
    /// PKCE verifier（exchange code 需要）
    pkce_verifier: Arc<Mutex<Option<PkceCodeVerifier>>>,
    /// `redirect_uri（exchange` code 需要）
    redirect_uri: String,
    /// 回调完成通知
    completed: Notify,
    /// 回调结果
    result: Arc<Mutex<Option<Result<TokenData>>>>,
    /// 回调服务器任务句柄（OAuth 完成后关闭）
    server_handle: Option<tokio::task::AbortHandle>,
}

impl PendingOAuth {
    /// 处理 OAuth 回调：验证 state，用 code 换 token，然后关闭回调服务器
    pub async fn handle_callback(&self, code: String, state: String) {
        if state != self.expected_state {
            let mut result = self.result.lock().await;
            *result = Some(Err(anyhow::anyhow!("Invalid state parameter")));
            self.abort_server();
            self.completed.notify_one();
            return;
        }

        let pkce_verifier = {
            let mut guard = self.pkce_verifier.lock().await;
            guard.take()
        };
        let Some(verifier) = pkce_verifier else {
            let mut result = self.result.lock().await;
            *result = Some(Err(anyhow::anyhow!("PKCE verifier already used")));
            self.abort_server();
            self.completed.notify_one();
            return;
        };

        let http = Client::new();
        let token_result = exchange_code(
            &http,
            &self.oauth_config,
            &self.client_registration,
            code,
            &self.redirect_uri,
            verifier,
        )
        .await;

        if let Ok(ref token) = token_result {
            tracing::info!(
                token_len = token.access_token.len(),
                "OAuth callback: saving token"
            );
            if let Err(e) = save_token(token) {
                tracing::warn!(error = %e, "OAuth callback: save_token failed");
            } else {
                tracing::info!("OAuth callback: token saved successfully");
            }
        }

        // 关闭回调服务器（无论成功失败）
        self.abort_server();

        let mut result = self.result.lock().await;
        *result = Some(token_result);
        self.completed.notify_one();
    }

    /// 关闭回调 HTTP 服务器
    pub fn abort_server(&self) {
        if let Some(ref handle) = self.server_handle {
            handle.abort();
            tracing::debug!("OAuth callback server aborted");
        }
    }

    /// 等待回调完成并返回结果
    pub async fn wait(&self) -> Result<TokenData> {
        self.completed.notified().await;
        let mut guard = self.result.lock().await;
        guard
            .take()
            .ok_or_else(|| anyhow::anyhow!("OAuth callback not completed"))?
    }
}

// ═══════════════════════════════════════════════════════════
//  公开 API
// ═══════════════════════════════════════════════════════════

/// 统一的 `access_token` 获取入口。
///
/// 1. config 中显式配置的 token（环境变量 / 配置文件 / --token 注入）→ 直接使用
/// 2. 本地 token 未过期 → 直接使用
/// 3. 本地 token 已过期 + 有 `refresh_token` → 自动刷新
/// 4. 刷新失败或无 token → 返回错误提示 `lx login`
pub async fn get_access_token(config: &Config) -> Result<String> {
    // 优先使用 config 中显式配置的 token（--token / LX_ACCESS_TOKEN / 配置文件）
    if let Some(token) = config.mcp.access_token.clone() {
        return Ok(token);
    }

    // 3. 本地 token 文件（自动刷新）
    match get_valid_token().await? {
        Some(td) => Ok(td.access_token),
        None => {
            if load_token()?.is_some() {
                anyhow::bail!("Access token 已过期且刷新失败。请重新运行 'lx login' 登录。")
            } else {
                anyhow::bail!("未找到有效的 access token。请先运行 'lx login' 登录。")
            }
        }
    }
}

/// OAuth 2.0 登录（PKCE + 动态客户端注册）
///
/// CLI 一次性流程：启动回调服务器 → 打印授权链接 → 阻塞等待回调 → 保存 token
pub async fn login() -> Result<TokenData> {
    println!("正在获取 OAuth 配置...");
    let (auth_url, pending) = start_oauth_server().await?;
    println!("\n请在浏览器中完成登录。授权链接:\n{auth_url}\n");
    let token = pending.wait().await?;
    Ok(token)
}

/// 登出 —— 删除本地 token 和客户端 session
pub fn logout() -> Result<()> {
    delete_token()?;
    delete_client_session()?;
    Ok(())
}

/// 两阶段 OAuth 登录 — 第一阶段：启动回调服务器，返回授权 URL
///
/// 适用于 VS Code 等编辑器扩展场景：编辑器用 `vscode.env.openExternal()` 打开浏览器，
/// 然后调用 `pending.wait()` 等待完成。Remote SSH 下也能正常工作。
pub async fn login_start() -> Result<(String, Arc<PendingOAuth>)> {
    start_oauth_server().await
}

/// 底层 OAuth 启动逻辑：获取配置 → 启动回调服务器 → 注册客户端 → 生成授权 URL → 构造 `PendingOAuth`
///
/// `login()` 和 `login_start()` 都调用此函数，统一存储和回调处理逻辑。
async fn start_oauth_server() -> Result<(String, Arc<PendingOAuth>)> {
    let http = Client::new();

    // 1. 获取 OAuth 服务端配置
    let oauth_cfg = fetch_oauth_config(&http).await?;

    // 2. 启动本地回调服务器
    let (tx, rx) = tokio::sync::oneshot::channel();
    let tx = Arc::new(Mutex::new(Some(tx)));

    let app = Router::new().route(
        "/",
        get(move |query: Query<CallbackQuery>| {
            let tx = tx.clone();
            async move {
                if let Some(tx) = tx.lock().await.take() {
                    let _ = tx.send(query.0);
                }
                Html("<html><body><h1>登录成功，请关闭此页面</h1></body></html>")
            }
        }),
    );

    let mut actual_port = CALLBACK_START_PORT;
    let listener = loop {
        let addr = SocketAddr::from(([127, 0, 0, 1], actual_port));
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                tracing::debug!("OAuth 回调端口: {}", actual_port);
                break listener;
            }
            Err(_) => {
                actual_port += 1;
                if actual_port >= CALLBACK_START_PORT + CALLBACK_MAX_PORT_ATTEMPTS {
                    anyhow::bail!(
                        "无法在 {}-{} 范围内分配 OAuth 回调端口",
                        CALLBACK_START_PORT,
                        CALLBACK_START_PORT + CALLBACK_MAX_PORT_ATTEMPTS - 1
                    );
                }
            }
        }
    };

    let redirect_uri = format!("http://127.0.0.1:{}", actual_port);
    let server_handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    })
    .abort_handle();

    // 3. 动态注册客户端
    let reg = register_client(&http, &oauth_cfg, &redirect_uri).await?;

    // 4. PKCE + CSRF
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let state = CsrfToken::new_random();
    let state_secret = state.secret().to_string();

    // 5. 授权 URL
    let auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        oauth_cfg.authorization_endpoint,
        reg.client_id,
        urlencoding::encode(&redirect_uri),
        oauth_cfg.scopes_supported.join("+"),
        state_secret,
        pkce_challenge.as_str(),
    );

    // 6. 构造 PendingOAuth
    let pending = Arc::new(PendingOAuth {
        expected_state: state_secret,
        callback_port: actual_port,
        oauth_config: oauth_cfg,
        client_registration: reg,
        pkce_verifier: Arc::new(Mutex::new(Some(pkce_verifier))),
        redirect_uri,
        completed: Notify::new(),
        result: Arc::new(Mutex::new(None)),
        server_handle: Some(server_handle),
    });

    // 7. 后台等待回调 → handle_callback 里会 save_token + abort_server
    let pending_clone = pending.clone();
    tokio::spawn(async move {
        match rx.await {
            Ok(callback) => {
                pending_clone
                    .handle_callback(callback.code, callback.state)
                    .await;
            }
            Err(_) => {
                let mut result = pending_clone.result.lock().await;
                *result = Some(Err(anyhow::anyhow!("OAuth callback channel closed")));
                pending_clone.completed.notify_one();
            }
        }
    });

    Ok((auth_url, pending))
}

/// 直接保存 access token（跳过 OAuth 流程）
///
/// 适用于从其他渠道获取 token 后直接配置的场景。
pub fn save_token_direct(access_token: &str) -> Result<()> {
    let token = TokenData {
        access_token: access_token.to_string(),
        refresh_token: None,
        expires_at: None,
        client_id: None,
    };
    save_token(&token)
}

// ═══════════════════════════════════════════════════════════
//  Token 持久化（~/.lexiang/auth/token.json）
// ═══════════════════════════════════════════════════════════

fn token_path() -> Result<PathBuf> {
    Ok(datadir::auth_dir().join(TOKEN_FILE))
}

pub fn save_token(token: &TokenData) -> Result<()> {
    let path = token_path()?;
    let json = serde_json::to_string_pretty(token)?;
    fs::write(&path, json)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&path, perms)?;
    }

    Ok(())
}

pub fn load_token() -> Result<Option<TokenData>> {
    let path = token_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let json = fs::read_to_string(&path)?;
    let token: TokenData = serde_json::from_str(&json)?;
    Ok(Some(token))
}

fn delete_token() -> Result<()> {
    let path = token_path()?;
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════
//  Client Session 持久化（~/.lexiang/auth/session.json）
// ═══════════════════════════════════════════════════════════

fn session_path() -> Result<PathBuf> {
    Ok(datadir::auth_dir().join(SESSION_FILE))
}

pub fn save_client_session(session: &ClientSessionData) -> Result<()> {
    let path = session_path()?;
    let json = serde_json::to_string_pretty(session)?;
    fs::write(&path, json)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&path, perms)?;
    }

    Ok(())
}

pub fn load_client_session() -> Result<Option<ClientSessionData>> {
    let path = session_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let json = fs::read_to_string(&path)?;
    let session: ClientSessionData = serde_json::from_str(&json)?;
    Ok(Some(session))
}

fn delete_client_session() -> Result<()> {
    let path = session_path()?;
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════
//  客户端登录 — 文件 IPC（跨进程传递回调 URL）
// ═══════════════════════════════════════════════════════════

/// 写入 pending-login 标记，表示有登录流程等待回调
pub fn write_pending_login() -> Result<()> {
    let path = datadir::auth_dir().join(PENDING_LOGIN_FILE);
    let json = serde_json::json!({ "started_at": chrono::Utc::now().timestamp() });
    fs::write(&path, json.to_string())?;
    Ok(())
}

/// 清除 pending-login 标记
pub fn clear_pending_login() -> Result<()> {
    let path = datadir::auth_dir().join(PENDING_LOGIN_FILE);
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

/// 检查是否有 pending login（`handle-url` 调用）
pub fn has_pending_login() -> bool {
    let path = datadir::auth_dir().join(PENDING_LOGIN_FILE);
    if !path.exists() {
        return false;
    }
    // 检查是否过期（> 5 分钟视为过期）
    let json = fs::read_to_string(&path).unwrap_or_default();
    let started_at: i64 = serde_json::from_str::<serde_json::Value>(&json)
        .ok()
        .and_then(|v| v.get("started_at").and_then(serde_json::Value::as_i64))
        .unwrap_or(0);
    let elapsed = chrono::Utc::now().timestamp() - started_at;
    (0..300).contains(&elapsed)
}

/// 将回调 URL 写入文件（`handle-url` 调用）
pub fn write_callback_url(url: &str) -> Result<()> {
    let path = datadir::auth_dir().join(CALLBACK_URL_FILE);
    fs::write(&path, url)?;
    Ok(())
}

/// 清除回调 URL 文件（`login --client` 开始时清理残留）
pub fn clear_callback_url() -> Result<()> {
    let path = datadir::auth_dir().join(CALLBACK_URL_FILE);
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

/// 等待回调 URL 文件出现（轮询，最多 120 秒）
pub async fn wait_for_callback_url() -> Result<String> {
    let path = datadir::auth_dir().join(CALLBACK_URL_FILE);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);

    while std::time::Instant::now() < deadline {
        if let Ok(content) = fs::read_to_string(&path) {
            if !content.trim().is_empty() {
                // 读取后删除
                let _ = fs::remove_file(&path);
                return Ok(content.trim().to_string());
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    Err(anyhow::anyhow!("等待浏览器回调超时（120秒），请重试"))
}

// ═══════════════════════════════════════════════════════════
//  客户端登录 — URL Scheme 注册（跨平台）
// ═══════════════════════════════════════════════════════════

/// 检查 lexiang:// URL scheme 是否已注册
pub fn is_url_scheme_registered() -> bool {
    is_url_scheme_registered_impl()
}

/// 注册 lexiang:// URL scheme 到系统
pub fn register_url_scheme() -> Result<()> {
    register_url_scheme_impl()
}

// ── macOS: .app bundle + lsregister + LSSetDefaultHandlerForURLScheme ──────

#[cfg(target_os = "macos")]
const APP_BUNDLE_ID: &str = "com.lexiang.cli-url-handler";

#[cfg(target_os = "macos")]
fn url_handler_app_path() -> PathBuf {
    datadir::datadir()
        .join("helpers")
        .join("LexiangURLOpener.app")
}

#[cfg(target_os = "macos")]
fn is_url_scheme_registered_impl() -> bool {
    let app_path = url_handler_app_path();
    let exe_path = app_path.join("Contents/MacOS/LexiangURLOpener");
    if !app_path.join("Contents/Info.plist").exists() || !exe_path.exists() {
        return false;
    }
    // 检查可执行文件是否为旧版 bash 脚本（需重建为 Swift 二进制）
    if let Ok(content) = fs::read_to_string(&exe_path) {
        if content.starts_with("#!/bin/bash") {
            return false;
        }
    }
    // 检查可执行文件中是否包含当前 lx 路径
    let current_exe = std::env::current_exe().unwrap_or_default();
    if !current_exe.as_os_str().is_empty() {
        if let Ok(content) = fs::read_to_string(&exe_path) {
            if !content.contains(&current_exe.display().to_string()) {
                return false;
            }
        }
    }
    true
}

#[cfg(target_os = "macos")]
fn register_url_scheme_impl() -> Result<()> {
    let app_path = url_handler_app_path();

    // 清理旧的 .app bundle（可能包含过期的 bash 脚本或旧路径）
    if app_path.exists() {
        let _ = fs::remove_dir_all(&app_path);
    }

    let contents = app_path.join("Contents");
    let macos = contents.join("MacOS");
    fs::create_dir_all(&macos)?;

    // 1. Info.plist
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>LexiangURLOpener</string>
    <key>CFBundleIdentifier</key>
    <string>{APP_BUNDLE_ID}</string>
    <key>CFBundleName</key>
    <string>Lexiang URL Opener</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSUIElement</key>
    <true/>
    <key>CFBundleURLTypes</key>
    <array>
        <dict>
            <key>CFBundleURLName</key>
            <string>Lexiang CLI URL Handler</string>
            <key>CFBundleURLSchemes</key>
            <array>
                <string>lexiang</string>
            </array>
        </dict>
    </array>
</dict>
</plist>"#
    );
    fs::write(contents.join("Info.plist"), plist)?;

    // 2. 编译 Swift 可执行文件 — 正确处理 Apple Event 中的 URL
    //
    // macOS URL scheme 回调通过 Apple Event (kAEGetURL) 传递 URL，
    // 必须用 NSAppleEventManager 注册 kAEGetURL 处理器才能接收。
    // application(_:open:) 是处理文件打开的，不是 URL scheme！
    // LSUIElement=true 避免在 Dock 中显示图标。
    let lx_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("./lx"));
    let exe_path = macos.join("LexiangURLOpener");
    let swift_source = format!(
        r#"
import Cocoa

let lxPath = "{lx_path}"

func forwardURL(_ url: String) {{
    let process = Process()
    process.executableURL = URL(fileURLWithPath: lxPath)
    process.arguments = ["handle-url", url]
    try? process.run()
    process.waitUntilExit()
    exit(0)
}}

// 优先从命令行参数获取 URL（macOS 首次启动时会把 URL 传给 argv）
if CommandLine.arguments.count > 1 {{
    forwardURL(CommandLine.arguments[1])
}}

// 注册 Apple Event handler 接收 kAEGetURL
class URLHandler: NSObject {{
    @objc func handleGetURL(_ event: NSAppleEventDescriptor, withReplyEvent: NSAppleEventDescriptor) {{
        guard let url = event.paramDescriptor(forKeyword: AEKeyword(keyDirectObject))?.stringValue else {{ return }}
        forwardURL(url)
    }}
}}

let handler = URLHandler()
NSAppleEventManager.shared().setEventHandler(
    handler,
    andSelector: #selector(URLHandler.handleGetURL(_:withReplyEvent:)),
    forEventClass: AEEventClass(kInternetEventClass),
    andEventID: AEEventID(kAEGetURL)
)

// 启动事件循环，5 秒超时防止挂起
DispatchQueue.main.asyncAfter(deadline: .now() + 5) {{
    exit(1)
}}

NSApplication.shared.run()
"#,
        lx_path = lx_path.display()
    );

    // 写入 Swift 源码并编译
    let swift_file = macos.join("LexiangURLOpener.swift");
    fs::write(&swift_file, &swift_source)?;

    let compile_output = std::process::Command::new("swiftc")
        .args([
            "-o",
            &exe_path.display().to_string(),
            &swift_file.display().to_string(),
        ])
        .output()?;

    if !compile_output.status.success() {
        let stderr = String::from_utf8_lossy(&compile_output.stderr);
        anyhow::bail!("Swift 编译失败: {}", stderr.trim());
    }

    // 编译成功后删除源码
    let _ = fs::remove_file(&swift_file);

    // 3. 注册到 Launch Services
    let lsregister = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
    let _ = std::process::Command::new(lsregister)
        .arg("-f")
        .arg(&app_path)
        .output();

    // 4. 设为默认 handler
    let swift_code = format!(
        r#"import CoreServices
LSSetDefaultHandlerForURLScheme("lexiang" as CFString, "{APP_BUNDLE_ID}" as CFString)"#
    );
    let _ = std::process::Command::new("swift")
        .arg("-e")
        .arg(&swift_code)
        .output();

    tracing::info!("lexiang:// URL scheme registered (macOS)");
    Ok(())
}

// ── Windows: 注册表 HKCU\Software\Classes\lexiang ─────────────────────────

#[cfg(target_os = "windows")]
fn is_url_scheme_registered_impl() -> bool {
    // 检查 HKCU\Software\Classes\lexiang 是否存在
    let output = std::process::Command::new("reg")
        .args(["query", r"HKCU\Software\Classes\lexiang", "/ve"])
        .output();
    match output {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

#[cfg(target_os = "windows")]
fn register_url_scheme_impl() -> Result<()> {
    let lx_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("lx"));
    let lx_str = lx_path.display().to_string();

    // reg add HKCU\Software\Classes\lexiang /ve /d "URL:Lexiang CLI" /f
    // reg add HKCU\Software\Classes\lexiang /v "URL Protocol" /f
    // reg add HKCU\Software\Classes\lexiang\shell\open\command /ve /d "\"lx_path\" handle-url \"%1\"" /f
    let reg_key = r"HKCU\Software\Classes\lexiang";

    let commands = [
        vec!["add", reg_key, "/ve", "/d", "URL:Lexiang CLI", "/f"],
        vec!["add", reg_key, "/v", "URL Protocol", "/f"],
        vec![
            "add",
            &format!(r"{}\shell\open\command", reg_key),
            "/ve",
            "/d",
            &format!("\"{}\" handle-url \"%1\"", lx_str),
            "/f",
        ],
    ];

    for args in &commands {
        let output = std::process::Command::new("reg").args(args).output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("reg add failed: {}", stderr.trim());
        }
    }

    tracing::info!("lexiang:// URL scheme registered (Windows)");
    Ok(())
}

// ── Linux: .desktop 文件 + update-desktop-database ────────────────────────

#[cfg(target_os = "linux")]
fn desktop_file_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/usr/local/share"))
        .join("applications")
        .join("lexiang-url-handler.desktop")
}

#[cfg(target_os = "linux")]
fn is_url_scheme_registered_impl() -> bool {
    desktop_file_path().exists()
}

#[cfg(target_os = "linux")]
fn register_url_scheme_impl() -> Result<()> {
    let lx_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("lx"));
    let desktop_dir = desktop_file_path()
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine applications directory"))?;
    fs::create_dir_all(desktop_dir)?;

    let desktop_entry = format!(
        r#"[Desktop Entry]
Type=Application
Name=Lexiang URL Handler
Exec="{}" handle-url %u
MimeType=x-scheme-handler/lexiang;
NoDisplay=true
"#,
        lx_path.display()
    );
    fs::write(desktop_file_path(), desktop_entry)?;

    // 更新桌面数据库
    let _ = std::process::Command::new("update-desktop-database")
        .arg(desktop_dir)
        .output();

    // 设为默认 handler
    let _ = std::process::Command::new("xdg-mime")
        .args([
            "default",
            "lexiang-url-handler.desktop",
            "x-scheme-handler/lexiang",
        ])
        .output();

    tracing::info!("lexiang:// URL scheme registered (Linux)");
    Ok(())
}

// ═══════════════════════════════════════════════════════════
//  Token 有效性检查 & 自动刷新
// ═══════════════════════════════════════════════════════════

pub fn is_expired_public(token: &TokenData) -> bool {
    match token.expires_at {
        Some(expires_at) => chrono::Utc::now().timestamp() >= expires_at - 300, // 提前 5 分钟
        None => false,
    }
}

fn is_expired(token: &TokenData) -> bool {
    is_expired_public(token)
}

/// 获取有效 token：未过期直接返回，过期则尝试 refresh，失败返回 None
async fn get_valid_token() -> Result<Option<TokenData>> {
    match load_token()? {
        Some(token) if !is_expired(&token) => Ok(Some(token)),
        Some(token) if token.refresh_token.is_some() => {
            tracing::info!("Access token 已过期，正在使用 refresh_token 刷新...");
            match refresh_token(&token).await {
                Ok(new_token) => {
                    tracing::info!("Token 刷新成功");
                    save_token(&new_token)?;
                    Ok(Some(new_token))
                }
                Err(e) => {
                    tracing::warn!("Token 刷新失败: {e}");
                    Ok(None)
                }
            }
        }
        Some(_) => {
            tracing::warn!("Access token 已过期且无 refresh_token");
            Ok(None)
        }
        None => Ok(None),
    }
}

/// 使用 `refresh_token` 换取新 token
async fn refresh_token(token: &TokenData) -> Result<TokenData> {
    let refresh = token
        .refresh_token
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No refresh token available"))?;

    let http = Client::new();

    // 复用 .well-known 获取 token_endpoint
    let cfg: OAuthServerConfigMinimal = http.get(WELL_KNOWN_URL).send().await?.json().await?;

    let mut form = vec![
        ("grant_type".to_string(), "refresh_token".to_string()),
        ("refresh_token".to_string(), refresh.clone()),
    ];
    // 公共客户端 refresh 时需要携带 client_id
    if let Some(cid) = &token.client_id {
        form.push(("client_id".to_string(), cid.clone()));
    }

    // lgtm[go/cleartext-transmission] endpoint 来自 HTTPS .well-known 响应
    let resp = http.post(&cfg.token_endpoint).form(&form).send().await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Token 刷新请求失败 (HTTP {}): {}", status, body);
    }

    let tr: OAuthTokenResponse = resp.json().await?;

    Ok(TokenData {
        access_token: tr.access_token,
        refresh_token: tr.refresh_token.or_else(|| token.refresh_token.clone()),
        expires_at: tr
            .expires_in
            .map(|ei| chrono::Utc::now().timestamp() + ei as i64),
        client_id: token.client_id.clone(),
    })
}

// ═══════════════════════════════════════════════════════════
//  OAuth 协议辅助函数（login 内部使用）
// ═══════════════════════════════════════════════════════════

async fn fetch_oauth_config(http: &Client) -> Result<OAuthServerConfig> {
    let resp = http.get(WELL_KNOWN_URL).send().await?;
    if !resp.status().is_success() {
        let err = resp.text().await?;
        anyhow::bail!("Failed to fetch OAuth config: {}", err);
    }
    Ok(resp.json().await?)
}

async fn register_client(
    http: &Client,
    oauth_cfg: &OAuthServerConfig,
    redirect_uri: &str,
) -> Result<ClientRegistration> {
    let body = serde_json::json!({
        "redirect_uris": [redirect_uri],
        "client_name": "Lexiang CLI",
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "scope": oauth_cfg.scopes_supported.join(" "),
    });

    // lgtm[go/cleartext-transmission] endpoint 来自 HTTPS .well-known 响应
    let resp = http
        .post(&oauth_cfg.registration_endpoint)
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let err = resp.text().await?;
        anyhow::bail!("Failed to register client: {}", err);
    }
    Ok(resp.json().await?)
}

async fn exchange_code(
    http: &Client,
    oauth_cfg: &OAuthServerConfig,
    reg: &ClientRegistration,
    code: String,
    redirect_uri: &str,
    pkce_verifier: PkceCodeVerifier,
) -> Result<TokenData> {
    let mut form = vec![
        ("grant_type", "authorization_code".to_string()),
        ("code", code),
        ("redirect_uri", redirect_uri.to_string()),
        ("client_id", reg.client_id.clone()),
        ("code_verifier", pkce_verifier.secret().clone()),
    ];
    if let Some(secret) = &reg.client_secret {
        form.push(("client_secret", secret.clone()));
    }

    // lgtm[go/cleartext-transmission] endpoint 来自 HTTPS .well-known 响应
    let resp = http
        .post(&oauth_cfg.token_endpoint)
        .form(&form)
        .send()
        .await?;

    if !resp.status().is_success() {
        let err = resp.text().await?;
        anyhow::bail!("Token exchange failed: {}", err);
    }

    let tr: OAuthTokenResponse = resp.json().await?;

    Ok(TokenData {
        access_token: tr.access_token,
        refresh_token: tr.refresh_token,
        expires_at: tr
            .expires_in
            .map(|ei| chrono::Utc::now().timestamp() + ei as i64),
        client_id: Some(reg.client_id.clone()),
    })
}

// open_browser 已移除 — 不允许自动打开浏览器，用户需手动复制链接

// ═══════════════════════════════════════════════════════════
//  客户端登录 — 网络流程
// ═══════════════════════════════════════════════════════════

/// 用 code 换取 cookie（调用内部 `auth_login` 接口）
///
/// `auth_login` 返回 JSON：`{"code": 0, "data": {"token": "jwt", "xsrf_token": "..."}}`
/// 内部接口用 `Cookie: token={jwt}` 认证。
async fn exchange_client_code_for_cookie(
    http: &Client,
    code: &str,
    state: Option<&str>,
) -> Result<String> {
    let mut body = serde_json::json!({ "code": code });
    if let Some(s) = state {
        body["state"] = serde_json::Value::String(s.to_string());
    }

    let resp = http.post(CLIENT_AUTH_LOGIN_URL).json(&body).send().await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("auth_login 请求失败 (HTTP {}): {}", status, body);
    }

    let resp_json: serde_json::Value = resp.json().await?;

    // 检查业务 code
    let biz_code = resp_json.get("code").and_then(serde_json::Value::as_i64);
    if biz_code != Some(0) {
        let msg = resp_json
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("auth_login 业务失败 (code={:?}): {}", biz_code, msg);
    }

    let token = resp_json
        .pointer("/data/token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("auth_login 响应中缺少 data.token"))?;

    // 内部接口用 Cookie: token={token}
    let cookie = format!("token={}", token);
    tracing::info!(
        token_len = token.len(),
        "client login: extracted token from auth_login response"
    );
    Ok(cookie)
}

/// 用 cookie 获取 MCP access token
async fn fetch_mcp_token_with_cookie(http: &Client, cookie: &str) -> Result<TokenData> {
    let resp = http
        .get(CLIENT_MCP_TOKENS_URL)
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("MCP tokens 请求失败 (HTTP {}): {}", status, body);
    }

    let json: serde_json::Value = resp.json().await?;
    let access_token = extract_mcp_access_token(&json)?;

    tracing::info!(
        token_len = access_token.len(),
        "client login: extracted MCP access token"
    );

    Ok(TokenData {
        access_token,
        refresh_token: None,
        expires_at: None,
        client_id: None,
    })
}

/// 客户端登录完整流程：解析 code+state → 换 cookie → 获取 MCP token → 持久化
pub async fn login_with_client_callback(callback_or_code: &str) -> Result<TokenData> {
    let (code, state) = extract_client_callback_params(callback_or_code)?;

    let http = Client::new();

    // 1. 用 code (及 state) 换 cookie
    let cookie = exchange_client_code_for_cookie(&http, &code, state.as_deref()).await?;

    // 2. 用 cookie 获取 MCP token
    let token = fetch_mcp_token_with_cookie(&http, &cookie).await?;

    // 3. 两者都成功后持久化
    let session = ClientSessionData {
        cookie,
        created_at: chrono::Utc::now().timestamp(),
    };
    save_client_session(&session)?;
    save_token(&token)?;

    Ok(token)
}

// ═══════════════════════════════════════════════════════════
//  客户端登录 — 纯函数
// ═══════════════════════════════════════════════════════════

/// 获取客户端登录重定向 URL
///
/// `redirect_url`：登录完成后浏览器跳转的目标 URL。
/// - `None` → 默认 `lexiang://auth-callback`（CLI 模式，需手动粘贴）
/// - `Some(url)` → 自定义（如 VS Code 的 `vscode://lexiang.lefs-vscode/auth-callback`）
pub fn client_login_url(redirect_url: Option<&str>) -> String {
    let target = redirect_url.unwrap_or("lexiang://auth-callback");
    format!(
        "https://lexiangla.com/auth/login-redirect?redirect_url={}",
        urlencoding::encode(target)
    )
}

/// 从回调 URL 或原始 code 中提取授权码和 state
///
/// - 如果 `input` 包含 `://`，解析 URL 并读取 `code` 和 `state` 查询参数
/// - 否则将非空 `input` 作为原始 code（state 为 None）
fn extract_client_callback_params(input: &str) -> Result<(String, Option<String>)> {
    let trimmed = input.trim();
    if trimmed.contains("://") {
        let url =
            url::Url::parse(trimmed).map_err(|e| anyhow::anyhow!("Invalid callback URL: {e}"))?;
        let code = url
            .query_pairs()
            .find(|(k, _)| k == "code")
            .map(|(_, v)| v.to_string())
            .ok_or_else(|| anyhow::anyhow!("Callback URL missing 'code' parameter"))?;
        let state = url
            .query_pairs()
            .find(|(k, _)| k == "state")
            .map(|(_, v)| v.to_string());
        Ok((code, state))
    } else if !trimmed.is_empty() {
        Ok((trimmed.to_string(), None))
    } else {
        Err(anyhow::anyhow!("Empty code input"))
    }
}

/// 从 `Set-Cookie` 响应头值列表构建 `Cookie` 请求头
///
/// 每个 `Set-Cookie` 值只保留第一个 `;` 前的 `name=value` 部分
#[allow(dead_code)]
fn build_cookie_header(set_cookie_values: &[String]) -> String {
    set_cookie_values
        .iter()
        .map(|v| v.split(';').next().unwrap_or("").trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

/// 从 MCP tokens API 响应中提取 `access_token`
///
/// 支持多种常见 JSON 结构：
/// - `/data/tokens/0/access_token`
/// - `/data/0/access_token`
/// - `/tokens/0/access_token`
/// - `/access_token`
/// - 递归搜索 `access_token` 或 `token` 键
fn extract_mcp_access_token(value: &serde_json::Value) -> Result<String> {
    // 尝试常见路径
    let paths: &[&[&str]] = &[
        &["data", "tokens", "0", "access_token"],
        &["data", "0", "access_token"],
        &["tokens", "0", "access_token"],
        &["access_token"],
    ];

    for path in paths {
        let mut current = value;
        for key in *path {
            current = match current.get(key) {
                Some(v) => v,
                None => break,
            };
        }
        if let Some(s) = current.as_str() {
            return Ok(s.to_string());
        }
    }

    // 递归搜索
    fn find_token(val: &serde_json::Value) -> Option<String> {
        match val {
            serde_json::Value::Object(map) => {
                if let Some(v) = map.get("access_token").and_then(|v| v.as_str()) {
                    return Some(v.to_string());
                }
                if let Some(v) = map.get("token").and_then(|v| v.as_str()) {
                    return Some(v.to_string());
                }
                for v in map.values() {
                    if let Some(s) = find_token(v) {
                        return Some(s);
                    }
                }
                None
            }
            serde_json::Value::Array(arr) => {
                for v in arr {
                    if let Some(s) = find_token(v) {
                        return Some(s);
                    }
                }
                None
            }
            _ => None,
        }
    }

    find_token(value).ok_or_else(|| anyhow::anyhow!("No access_token found in MCP tokens response"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_client_callback_extracts_code_and_state() {
        let (code, state) = extract_client_callback_params(
            "lexiang://auth-callback?code=c8c3ffaf-e420-4328-8f70-f394e92b4534&state=01420d9d-c7c3-4e8a-b719-0dfb4b4bef48",
        )
        .unwrap();
        assert_eq!(code, "c8c3ffaf-e420-4328-8f70-f394e92b4534");
        assert_eq!(
            state,
            Some("01420d9d-c7c3-4e8a-b719-0dfb4b4bef48".to_string())
        );
    }

    #[test]
    fn parse_client_callback_code_only() {
        let (code, state) = extract_client_callback_params(
            "lexiang://auth-callback?code=c8c3ffaf-e420-4328-8f70-f394e92b4534",
        )
        .unwrap();
        assert_eq!(code, "c8c3ffaf-e420-4328-8f70-f394e92b4534");
        assert_eq!(state, None);
    }

    #[test]
    fn parse_client_callback_accepts_raw_code() {
        let (code, state) =
            extract_client_callback_params("c8c3ffaf-e420-4328-8f70-f394e92b4534").unwrap();
        assert_eq!(code, "c8c3ffaf-e420-4328-8f70-f394e92b4534");
        assert_eq!(state, None);
    }

    #[test]
    fn cookie_header_keeps_name_value_pairs() {
        let cookies = vec![
            "uid=123; Path=/; HttpOnly".to_string(),
            "session=abc; Path=/; Secure".to_string(),
        ];
        assert_eq!(build_cookie_header(&cookies), "uid=123; session=abc");
    }

    #[test]
    fn extract_mcp_token_from_common_shapes() {
        let response = serde_json::json!({
            "code": 0,
            "data": {
                "tokens": [{ "access_token": "mcp-token-1", "enabled": true }]
            }
        });
        assert_eq!(extract_mcp_access_token(&response).unwrap(), "mcp-token-1");
    }

    #[test]
    fn extract_mcp_token_from_data_0_path() {
        let response = serde_json::json!({
            "data": [{ "access_token": "mcp-token-2" }]
        });
        assert_eq!(extract_mcp_access_token(&response).unwrap(), "mcp-token-2");
    }

    #[test]
    fn extract_mcp_token_from_root_access_token() {
        let response = serde_json::json!({
            "access_token": "mcp-token-3"
        });
        assert_eq!(extract_mcp_access_token(&response).unwrap(), "mcp-token-3");
    }

    #[test]
    fn extract_mcp_token_recursive_fallback() {
        let response = serde_json::json!({
            "result": { "nested": { "token": "deep-token" } }
        });
        assert_eq!(extract_mcp_access_token(&response).unwrap(), "deep-token");
    }
}
