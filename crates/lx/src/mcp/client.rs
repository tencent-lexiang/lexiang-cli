use crate::mcp::{HttpTransport, SchemaManager, ToolCallResult, ToolSchema, ToolsListResult};
use anyhow::Result;
use serde::de::DeserializeOwned;
use std::sync::{Arc, Mutex};

pub struct McpClient {
    transport: HttpTransport,
    schema_manager: Arc<Mutex<SchemaManager>>,
}

impl McpClient {
    pub fn new(url: impl Into<String>, access_token: Option<String>) -> Result<Self> {
        Ok(Self {
            transport: HttpTransport::new(url, access_token)?,
            schema_manager: Arc::new(Mutex::new(SchemaManager::new())),
        })
    }

    pub async fn list_tools(&self) -> Result<Vec<ToolSchema>> {
        let result: ToolsListResult = self
            .transport
            .call("tools/list", serde_json::json!({}))
            .await?;

        // Update schema manager with fetched tools
        if let Ok(mut manager) = self.schema_manager.lock() {
            manager.update_dynamic(result.tools.clone());
        }

        Ok(result.tools)
    }

    pub async fn call_tool(
        &self,
        name: &str,
        mut args: serde_json::Value,
    ) -> Result<serde_json::Value> {
        // Extract fields from schema and inject _mcp_fields
        if let Ok(manager) = self.schema_manager.lock() {
            let fields = manager.extract_fields(name);
            if !fields.is_empty() {
                args["_mcp_fields"] = serde_json::json!(fields);
            }
        }

        let params = serde_json::json!({
            "name": name,
            "arguments": args,
        });

        let result: ToolCallResult = self.transport.call("tools/call", params).await?;

        // Extract text content
        for block in result.content {
            if block.type_ == "text" {
                if let Some(text) = block.text {
                    // Try to parse as JSON
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        return Ok(json);
                    }
                    return Ok(serde_json::json!(text));
                }
            }
        }

        Ok(serde_json::json!({}))
    }

    /// 调用 MCP 工具并反序列化为指定类型
    pub async fn call_raw<T: DeserializeOwned>(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> Result<T> {
        let result = self.call_tool(name, args).await?;
        let typed: T = serde_json::from_value(result)?;
        Ok(typed)
    }
}
