//! `McpCaller` trait — MCP 调用接口抽象
//!
//! 从 shell/fs/lexiang.rs 提取到公共位置，
//! 让 service 层和 shell/fs 层共同引用。

use anyhow::Result;
use async_trait::async_trait;

/// MCP 调用接口抽象 — 隔离网络依赖，方便测试
#[async_trait]
pub trait McpCaller: Send + Sync {
    /// 调用 MCP 工具
    async fn call_tool(
        &self,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value>;
}

/// 真实 MCP 客户端适配器
pub struct RealMcpCaller {
    url: String,
    access_token: Option<String>,
}

impl RealMcpCaller {
    pub fn new(url: &str, access_token: Option<String>) -> Self {
        Self {
            url: url.to_string(),
            access_token,
        }
    }
}

#[async_trait]
impl McpCaller for RealMcpCaller {
    async fn call_tool(
        &self,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value> {
        use crate::mcp::McpClient;
        let client = McpClient::new(&self.url, self.access_token.clone())?;
        client.call_tool(tool_name, args).await
    }
}
