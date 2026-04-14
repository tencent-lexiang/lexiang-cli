use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: Option<InputSchema>,
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
