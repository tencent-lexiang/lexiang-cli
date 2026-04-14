use crate::config::Config;
use crate::mcp::McpClient;
use anyhow::Result;

pub async fn list_tools(config: &Config) -> Result<()> {
    let access_token = crate::auth::get_access_token(config).await?;
    let client = McpClient::new(&config.mcp.url, Some(access_token))?;

    let tools = client.list_tools().await?;

    println!("Available tools ({}):\n", tools.len());
    for tool in tools {
        println!("  {} - {}", tool.name, tool.description.unwrap_or_default());
    }

    Ok(())
}

pub async fn call_tool(config: &Config, name: &str, params: serde_json::Value) -> Result<()> {
    let access_token = crate::auth::get_access_token(config).await?;
    let client = McpClient::new(&config.mcp.url, Some(access_token))?;

    let result = client.call_tool(name, params).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);

    Ok(())
}
