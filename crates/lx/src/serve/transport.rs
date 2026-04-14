//! stdio transport for JSON-RPC server
//!
//! Reads JSON-RPC requests from stdin, dispatches them, and writes responses to stdout.
//! Uses newline-delimited JSON (one JSON-RPC message per line).

use super::handler;
use super::protocol::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use super::ServeState;
use anyhow::Result;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::RwLock;

/// stdio transport for the JSON-RPC server
pub struct ServeTransport {
    state: Arc<RwLock<ServeState>>,
    verbose: bool,
}

impl ServeTransport {
    pub fn new(state: Arc<RwLock<ServeState>>, verbose: bool) -> Self {
        Self { state, verbose }
    }

    /// Run the main loop: read stdin → dispatch → write stdout
    pub async fn run(self) -> Result<()> {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut stdout = tokio::io::stdout();

        // Ensure access token is resolved at startup
        {
            let mut state = self.state.write().await;
            match crate::auth::get_access_token(&state.config).await {
                Ok(token) => {
                    state.access_token = Some(token);
                    if self.verbose {
                        eprintln!("[lx serve] auth resolved successfully");
                    }
                }
                Err(e) => {
                    eprintln!("[lx serve] warning: auth not available: {e}");
                }
            }
        }

        if self.verbose {
            eprintln!("[lx serve] ready — reading JSON-RPC from stdin");
        }

        let mut line = String::new();
        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line).await?;

            if bytes_read == 0 {
                // stdin closed — parent process exited
                if self.verbose {
                    eprintln!("[lx serve] stdin closed, shutting down");
                }
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Parse JSON-RPC request
            let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
                Ok(req) => req,
                Err(e) => {
                    eprintln!("[lx serve] parse error: {e}");
                    let error_response = JsonRpcError::new(-32700, format!("Parse error: {e}"));
                    let resp = JsonRpcResponse::err(None, error_response);
                    let output = serde_json::to_string(&resp)?;
                    stdout.write_all(output.as_bytes()).await?;
                    stdout.write_all(b"\n").await?;
                    stdout.flush().await?;
                    continue;
                }
            };

            if self.verbose {
                eprintln!("[lx serve] <- {}", request.method);
            }

            // Check for exit notification
            if request.method == "exit" {
                if self.verbose {
                    eprintln!("[lx serve] received exit notification, shutting down");
                }
                break;
            }

            // Dispatch and get optional response
            let response = handler::dispatch(request, &self.state).await;

            if let Some(resp) = response {
                let output = serde_json::to_string(&resp)?;
                stdout.write_all(output.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
        }

        Ok(())
    }
}
