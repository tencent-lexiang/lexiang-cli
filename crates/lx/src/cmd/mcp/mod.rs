use crate::config::Config;
use crate::mcp::{McpClient, ResourcesReadResult};
use anyhow::{Context, Result};

use super::output::{print_output, FieldFilter};

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

pub async fn list_resources(config: &Config, format: &str) -> Result<()> {
    let access_token = crate::auth::get_access_token(config).await?;
    let client = McpClient::new(&config.mcp.url, Some(access_token))?;
    let resources = client.list_resources().await?;
    let value = serde_json::json!({"resources": resources});

    print_output(&value, format, &FieldFilter::new(None, true))
}

pub async fn read_resource(config: &Config, uri: &str, format: &str) -> Result<()> {
    let access_token = crate::auth::get_access_token(config).await?;
    let client = McpClient::new(&config.mcp.url, Some(access_token))?;
    let result = client.read_resource(uri).await?;

    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&result)?),
        "text" => print!("{}", single_text_resource(&result)?),
        _ => anyhow::bail!("unsupported resource output format: {format}"),
    }

    Ok(())
}

fn single_text_resource(result: &ResourcesReadResult) -> Result<&str> {
    let [content] = result.contents.as_slice() else {
        anyhow::bail!(
            "resource returned {} content items; use --format json to preserve boundaries",
            result.contents.len()
        );
    };
    content
        .text
        .as_deref()
        .context("resource has no text content; use --format json for blob content")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::ResourceContents;
    use std::collections::BTreeMap;

    fn content(text: Option<&str>, blob: Option<&str>) -> ResourceContents {
        ResourceContents {
            uri: "lexiang://docs/example/v1".to_string(),
            mime_type: Some("text/markdown".to_string()),
            text: text.map(str::to_string),
            blob: blob.map(str::to_string),
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn extracts_exact_single_text_resource() {
        let result = ResourcesReadResult {
            contents: vec![content(Some("# DSL\nbody"), None)],
            extra: BTreeMap::new(),
        };

        assert_eq!(single_text_resource(&result).unwrap(), "# DSL\nbody");
    }

    #[test]
    fn rejects_blob_and_multiple_contents_in_text_mode() {
        let blob = ResourcesReadResult {
            contents: vec![content(None, Some("YWJj"))],
            extra: BTreeMap::new(),
        };
        assert!(single_text_resource(&blob)
            .unwrap_err()
            .to_string()
            .contains("--format json"));

        let multiple = ResourcesReadResult {
            contents: vec![content(Some("one"), None), content(Some("two"), None)],
            extra: BTreeMap::new(),
        };
        assert!(single_text_resource(&multiple)
            .unwrap_err()
            .to_string()
            .contains("boundaries"));
    }
}
