use crate::mcp::{
    HttpTransport, ResourceDescriptor, ResourcesListResult, ResourcesReadResult, SchemaManager,
    ToolCallResult, ToolSchema, ToolsListResult,
};
use anyhow::Result;
use serde::de::DeserializeOwned;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
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

    /// 返回当前使用的 `access_token（用于缓存判断`）
    pub fn access_token(&self) -> Option<&str> {
        self.transport.access_token()
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

    /// List every resource advertised by the MCP server, following pagination.
    pub async fn list_resources(&self) -> Result<Vec<ResourceDescriptor>> {
        let mut resources = Vec::new();
        let mut cursor: Option<String> = None;
        let mut seen_cursors = HashSet::new();

        loop {
            let params = cursor.as_ref().map_or_else(
                || serde_json::json!({}),
                |value| serde_json::json!({"cursor": value}),
            );
            let result: ResourcesListResult = self.transport.call("resources/list", params).await?;
            resources.extend(result.resources);

            let Some(next_cursor) = result.next_cursor.filter(|value| !value.is_empty()) else {
                break;
            };
            if !seen_cursors.insert(next_cursor.clone()) {
                anyhow::bail!("MCP resources/list returned a repeated cursor: {next_cursor}");
            }
            cursor = Some(next_cursor);
        }

        Ok(resources)
    }

    /// Read a resource by the URI returned from `resources/list`.
    pub async fn read_resource(&self, uri: &str) -> Result<ResourcesReadResult> {
        self.transport
            .call("resources/read", serde_json::json!({"uri": uri}))
            .await
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

        // 解析为 ToolCallResult（日志已在 transport 层统一记录）
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

        // 如果没有 text block，返回空对象
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{extract::State, routing::post, Json, Router};
    use serde_json::{json, Value};

    #[derive(Clone, Default)]
    struct MockState {
        requests: Arc<Mutex<Vec<Value>>>,
    }

    async fn handle_mcp(State(state): State<MockState>, Json(request): Json<Value>) -> Json<Value> {
        state.requests.lock().unwrap().push(request.clone());
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request["method"].as_str().unwrap_or_default();
        let params = &request["params"];

        let result = match method {
            "resources/list" if params.get("cursor").is_none() => json!({
                "resources": [{
                    "uri": "lexiang://docs/block-mdx/v0",
                    "name": "block-mdx-v0",
                    "mimeType": "text/plain"
                }],
                "nextCursor": "next-page"
            }),
            "resources/list" => json!({
                "resources": [{
                    "uri": "lexiang://docs/block-view-dsl/v0",
                    "name": "block-view-dsl-v0",
                    "mimeType": "text/plain"
                }]
            }),
            "resources/read" => json!({
                "contents": [{
                    "uri": params["uri"],
                    "mimeType": "text/plain",
                    "text": "# Runtime DSL"
                }]
            }),
            _ => json!({}),
        };

        Json(json!({"jsonrpc": "2.0", "id": id, "result": result}))
    }

    async fn mock_client() -> (McpClient, MockState, tokio::task::JoinHandle<()>) {
        let state = MockState::default();
        let app = Router::new()
            .route("/mcp", post(handle_mcp))
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let client =
            McpClient::new(format!("http://{address}/mcp"), Some("token".to_string())).unwrap();
        (client, state, server)
    }

    #[tokio::test]
    async fn lists_all_resource_pages_and_passes_cursor() {
        let (client, state, server) = mock_client().await;
        let resources = client.list_resources().await.unwrap();
        server.abort();

        assert_eq!(resources.len(), 2);
        assert_eq!(resources[0].name, "block-mdx-v0");
        assert_eq!(resources[1].name, "block-view-dsl-v0");

        let requests = state.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[0]["params"].get("cursor").is_none());
        assert_eq!(requests[1]["params"]["cursor"], "next-page");
    }

    #[tokio::test]
    async fn reads_resource_by_uri() {
        let (client, state, server) = mock_client().await;
        let result = client
            .read_resource("lexiang://docs/block-mdx/v0")
            .await
            .unwrap();
        server.abort();

        assert_eq!(result.contents[0].text.as_deref(), Some("# Runtime DSL"));
        let requests = state.requests.lock().unwrap();
        assert_eq!(requests[0]["params"]["uri"], "lexiang://docs/block-mdx/v0");
    }
}
