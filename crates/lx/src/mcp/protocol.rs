use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "inputSchema", alias = "input_schema")]
    pub input_schema: Option<InputSchema>,
    #[serde(default, rename = "outputSchema", alias = "output_schema")]
    pub output_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputSchema {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default)]
    pub properties: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub required: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ToolsListResult {
    pub tools: Vec<ToolSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceDescriptor {
    pub uri: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(
        default,
        rename = "mimeType",
        alias = "mime_type",
        skip_serializing_if = "Option::is_none"
    )]
    pub mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ResourcesListResult {
    #[serde(default)]
    pub resources: Vec<ResourceDescriptor>,
    #[serde(
        default,
        rename = "nextCursor",
        alias = "next_cursor",
        skip_serializing_if = "Option::is_none"
    )]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceContents {
    pub uri: String,
    #[serde(
        default,
        rename = "mimeType",
        alias = "mime_type",
        skip_serializing_if = "Option::is_none"
    )]
    pub mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ResourcesReadResult {
    #[serde(default)]
    pub contents: Vec<ResourceContents>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ToolCallResult {
    pub content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default)]
    pub text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_schema_reads_camel_case_input_and_output() {
        let schema: ToolSchema = serde_json::from_value(serde_json::json!({
            "name": "file_apply_upload",
            "inputSchema": {
                "type": "object",
                "properties": {"extension": {"type": "string"}}
            },
            "outputSchema": {
                "type": "object",
                "properties": {"client_upload_session": {"type": "object"}}
            }
        }))
        .unwrap();

        assert!(schema
            .input_schema
            .unwrap()
            .properties
            .contains_key("extension"));
        assert!(schema
            .output_schema
            .unwrap()
            .pointer("/properties/client_upload_session")
            .is_some());
    }

    #[test]
    fn resource_results_preserve_protocol_fields_and_extensions() {
        let list: ResourcesListResult = serde_json::from_value(serde_json::json!({
            "resources": [{
                "uri": "lexiang://docs/block-mdx/v0",
                "name": "block-mdx-v0",
                "description": "Block MDX DSL",
                "mimeType": "text/markdown",
                "annotations": {"audience": ["assistant"]}
            }],
            "nextCursor": "page-2"
        }))
        .unwrap();

        assert_eq!(list.next_cursor.as_deref(), Some("page-2"));
        assert_eq!(
            list.resources[0].mime_type.as_deref(),
            Some("text/markdown")
        );
        assert!(list.resources[0].extra.contains_key("annotations"));

        let read: ResourcesReadResult = serde_json::from_value(serde_json::json!({
            "contents": [{
                "uri": "lexiang://docs/block-mdx/v0",
                "mimeType": "text/markdown",
                "text": "# DSL"
            }],
            "request_id": "request-1"
        }))
        .unwrap();

        assert_eq!(read.contents[0].text.as_deref(), Some("# DSL"));
        assert_eq!(read.extra["request_id"], "request-1");
        assert_eq!(
            serde_json::to_value(read).unwrap()["contents"][0]["mimeType"],
            "text/markdown"
        );
    }
}
