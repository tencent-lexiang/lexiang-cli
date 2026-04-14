use crate::json_rpc::{JsonRpcRequest, JsonRpcResponse};
use anyhow::Result;

const DEFAULT_TIMEOUT_SECS: u64 = 30;

pub struct HttpTransport {
    client: reqwest::Client,
    url: String,
    access_token: Option<String>,
}

impl HttpTransport {
    pub fn new(url: impl Into<String>, access_token: Option<String>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()?;

        Ok(Self {
            client,
            url: url.into(),
            access_token,
        })
    }

    pub async fn call<T: for<'de> serde::Deserialize<'de> + Default>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T> {
        let request = JsonRpcRequest::new(method, params);

        let mut request_builder = self.client.post(&self.url).json(&request);
        if let Some(token) = &self.access_token {
            request_builder = request_builder.bearer_auth(token);
        }

        let response = request_builder.send().await?;

        let rpc_response: JsonRpcResponse<T> = response.json().await?;

        if let Some(error) = rpc_response.error {
            anyhow::bail!("MCP error {}: {}", error.code, error.message);
        }

        rpc_response
            .result
            .ok_or_else(|| anyhow::anyhow!("No result in MCP response"))
    }
}
