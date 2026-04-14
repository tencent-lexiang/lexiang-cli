//! `lx serve` — stdio JSON-RPC 2.0 server
//!
//! Provides a programmatic API for editors (VS Code, Neovim, etc.) to access
//! Lexiang knowledge base data via JSON-RPC 2.0 over stdin/stdout.
//!
//! # Architecture
//!
//! Method handlers use `inventory::submit!` for compile-time auto-registration.
//! No central match statement — each handler is an independent unit.
//!
//! Unknown methods automatically fall through to MCP tool calls (dynamic proxy),
//! so new MCP tools are available without code changes.
//!
//! # Adding a new handler
//!
//! ```ignore
//! // In any file under src/serve/methods/
//! use crate::serve::{JsonRpcResult, ServeContext, rpc_method};
//!
//! async fn handle_my_method(ctx: &ServeContext, params: Value) -> JsonRpcResult {
//!     let client = ctx.mcp_client().await?;
//!     Ok(serde_json::json!({ "result": "..." }))
//! }
//!
//! inventory::submit! {
//!     rpc_method!("my/domain/method", handle_my_method)
//! }
//! ```
//!
//! # Protocol
//! - Reads JSON-RPC requests from stdin (one per line, `\n`-delimited)
//! - Writes JSON-RPC responses/notifications to stdout
//! - Logs to stderr only (never stdout — would corrupt JSON-RPC stream)

mod handler;
mod methods;
mod protocol;
mod transport;

pub use protocol::{
    JsonRpcError, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, JsonRpcResult,
};
pub use transport::ServeTransport;

use crate::config::Config;
use anyhow::Result;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

// ═══════════════════════════════════════════════════════════
//  Shared State & Context
// ═══════════════════════════════════════════════════════════

/// Shared server state (mutable, guarded by `RwLock`)
pub struct ServeState {
    pub config: Config,
    pub access_token: Option<String>,
}

impl ServeState {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            access_token: None,
        }
    }
}

/// Request-scoped context passed to every handler
///
/// Provides convenience methods for common operations (MCP client, auth, etc.)
pub struct ServeContext {
    state: Arc<RwLock<ServeState>>,
}

impl ServeContext {
    pub fn new(state: Arc<RwLock<ServeState>>) -> Self {
        Self { state }
    }

    /// Get a ready-to-use MCP client (resolves auth automatically)
    pub async fn mcp_client(&self) -> Result<crate::mcp::McpClient, JsonRpcError> {
        let state = self.state.read().await;
        let token = crate::auth::get_access_token(&state.config)
            .await
            .map_err(|e| JsonRpcError::new(error_codes::AUTH_EXPIRED, e.to_string()))?;
        crate::mcp::McpClient::new(&state.config.mcp.url, Some(token))
            .map_err(|e| JsonRpcError::new(error_codes::INTERNAL_ERROR, e.to_string()))
    }

    /// Get an MCP client for a specific `company_from` (tenant)
    pub async fn mcp_client_for(
        &self,
        company_from: &str,
    ) -> Result<crate::mcp::McpClient, anyhow::Error> {
        let mcp_url = format!("https://{}.lexiangla.com/api/mcp", company_from);
        let token = crate::auth::get_access_token(&self.state.read().await.config).await?;
        crate::mcp::McpClient::new(&mcp_url, Some(token))
    }

    /// Call an MCP tool directly
    pub async fn mcp_call(&self, tool_name: &str, args: Value) -> JsonRpcResult {
        let client = self.mcp_client().await?;
        self.mcp_call_with(&client, tool_name, args).await
    }

    /// Call an MCP tool with a pre-created client (avoids redundant auth resolution)
    pub async fn mcp_call_with(
        &self,
        client: &crate::mcp::McpClient,
        tool_name: &str,
        args: Value,
    ) -> JsonRpcResult {
        client
            .call_tool(tool_name, args)
            .await
            .map_err(|e| JsonRpcError::new(error_codes::NETWORK_ERROR, e.to_string()))
    }

    /// Extract a required string param
    pub fn require_str<'a>(&self, params: &'a Value, key: &str) -> Result<&'a str, JsonRpcError> {
        params
            .get(key)
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params(format!("Missing {key}")))
    }
}

// ═══════════════════════════════════════════════════════════
//  Method Registry (inventory-based)
// ═══════════════════════════════════════════════════════════

/// Type-erased async handler function
type HandlerFn =
    fn(&ServeContext, Value) -> Pin<Box<dyn Future<Output = JsonRpcResult> + Send + '_>>;

/// A registered JSON-RPC method descriptor
pub struct RpcMethod {
    /// Method name (e.g., "space/list", "entry/tree")
    pub name: &'static str,
    /// Handler function
    pub handler: HandlerFn,
}

inventory::collect!(RpcMethod);

/// Macro to submit a method to the registry at compile time
///
/// # Usage
/// ```ignore
/// inventory::submit! { rpc_method!("space/list", handle_space_list) }
/// ```
#[macro_export]
macro_rules! rpc_method {
    ($name:literal, $handler:expr) => {
        $crate::serve::RpcMethod {
            name: $name,
            handler: |ctx, params| Box::pin($handler(ctx, params)),
        }
    };
}

// ═══════════════════════════════════════════════════════════
//  Error Codes
// ═══════════════════════════════════════════════════════════

pub mod error_codes {
    pub const AUTH_EXPIRED: i32 = -32001;
    pub const AUTH_REQUIRED: i32 = -32001;
    pub const NOT_FOUND: i32 = -32002;
    pub const QUOTA_EXCEEDED: i32 = -32003;
    pub const NETWORK_ERROR: i32 = -32004;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INTERNAL_ERROR: i32 = -32603;
}

// ═══════════════════════════════════════════════════════════
//  Entry Point
// ═══════════════════════════════════════════════════════════

/// Run the JSON-RPC server on stdio
pub async fn run_serve(config: Config, verbose: bool) -> Result<()> {
    if verbose {
        let methods: Vec<&str> = inventory::iter::<RpcMethod>
            .into_iter()
            .map(|m| m.name)
            .collect();
        eprintln!("[lx serve] registered methods: {:?}", methods);
        eprintln!("[lx serve] starting JSON-RPC server on stdio (verbose mode)");
    }

    let state = Arc::new(RwLock::new(ServeState::new(config)));
    let transport = ServeTransport::new(state, verbose);

    transport.run().await
}
