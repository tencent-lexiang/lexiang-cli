use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// MCP Tool Schema - 完整的工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolSchema {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "inputSchema")]
    pub input_schema: Option<McpInputSchema>,
    #[serde(
        default,
        rename = "outputSchema",
        skip_serializing_if = "Option::is_none"
    )]
    pub output_schema: Option<serde_json::Value>,
    /// 从 category 解析出来的 namespace（如 "team", "space"）
    #[serde(skip)]
    pub namespace: Option<String>,
    /// 从 name 解析出来的命令名（如 "list", "describe"）
    #[serde(skip)]
    pub command_name: Option<String>,
}

impl McpToolSchema {
    /// 从 protocol 层 `ToolSchema` 转换
    pub fn from_protocol(ts: &crate::mcp::ToolSchema) -> Self {
        Self {
            name: ts.name.clone(),
            description: ts.description.clone(),
            input_schema: ts
                .input_schema
                .as_ref()
                .map(|is| McpInputSchema::from_protocol(is.clone())),
            output_schema: None,
            namespace: None,
            command_name: None,
        }
    }

    /// 从 MCP server `get_tool_schema` (format=json) 的响应解析
    ///
    /// 兼容多种返回格式：
    /// - 直接 `{name, description, inputSchema}`
    /// - 嵌套 `{tool: {...}}` 或 `{schema: {...}}`
    /// - `schema` 字段为 JSON **字符串**（需要二次解析）
    pub fn from_raw_response(tool_name: &str, response: &serde_json::Value) -> Self {
        // 优先尝试直接在顶层找 inputSchema
        if response.get("inputSchema").is_some() || response.get("input_schema").is_some() {
            return Self::parse_tool_obj(tool_name, response);
        }

        // 尝试 {tool: {...}} 嵌套
        if let Some(inner) = response.get("tool") {
            return Self::parse_tool_obj(tool_name, inner);
        }

        // 尝试 {schema: "..."} — schema 可能是字符串（需要二次 parse）或对象
        if let Some(schema_val) = response.get("schema") {
            let parsed = if let Some(s) = schema_val.as_str() {
                // schema 是 JSON 字符串，二次解析
                serde_json::from_str::<serde_json::Value>(s).ok()
            } else {
                // schema 已经是对象
                Some(schema_val.clone())
            };

            if let Some(inner) = parsed {
                let mut result = Self::parse_tool_obj(tool_name, &inner);
                // 用外层的 description 作为 fallback
                if result.description.is_none() {
                    result.description = response
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(std::string::ToString::to_string);
                }
                return result;
            }
        }

        // fallback：直接从顶层解析
        Self::parse_tool_obj(tool_name, response)
    }

    /// 从一个包含 inputSchema/outputSchema 字段的 JSON 对象解析
    fn parse_tool_obj(tool_name: &str, obj: &serde_json::Value) -> Self {
        let description = obj
            .get("description")
            .and_then(|v| v.as_str())
            .map(std::string::ToString::to_string);

        let raw_input = obj.get("inputSchema").or_else(|| obj.get("input_schema"));

        let raw_output = obj.get("outputSchema").or_else(|| obj.get("output_schema"));

        Self {
            name: tool_name.to_string(),
            description,
            input_schema: raw_input.and_then(McpInputSchema::from_value),
            output_schema: raw_output.cloned(),
            namespace: None,
            command_name: None,
        }
    }

    /// 转换为 protocol 层 `ToolSchema`
    pub fn to_protocol(&self) -> crate::mcp::ToolSchema {
        crate::mcp::ToolSchema {
            name: self.name.clone(),
            description: self.description.clone(),
            input_schema: self.input_schema.as_ref().map(McpInputSchema::to_protocol),
        }
    }
}

/// 输入参数 Schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpInputSchema {
    #[serde(rename = "type", default = "default_type")]
    pub type_: String,
    #[serde(default)]
    pub properties: HashMap<String, McpPropertySchema>,
    #[serde(default)]
    pub required: Vec<String>,
    #[serde(
        default,
        rename = "additionalProperties",
        skip_serializing_if = "Option::is_none"
    )]
    pub additional_properties: Option<bool>,
}

fn default_type() -> String {
    "object".to_string()
}

impl McpInputSchema {
    /// 从 protocol 层 InputSchema（properties 为 Map<String, Value>）转换
    pub fn from_protocol(is: crate::mcp::InputSchema) -> Self {
        Self {
            type_: is.type_,
            properties: is
                .properties
                .into_iter()
                .map(|(k, v)| (k, McpPropertySchema::from_value_or_fallback(&v)))
                .collect(),
            required: is.required,
            additional_properties: None,
        }
    }

    /// 从 JSON Value（包含 type/properties/required 字段）解析
    pub fn from_value(value: &serde_json::Value) -> Option<Self> {
        let type_ = value
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("object")
            .to_string();

        let properties = value
            .get("properties")
            .and_then(|v| v.as_object())
            .map(|props| {
                props
                    .iter()
                    .map(|(k, v)| (k.clone(), McpPropertySchema::from_value_or_fallback(v)))
                    .collect()
            })
            .unwrap_or_default();

        let required = value
            .get("required")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(std::string::ToString::to_string))
                    .collect()
            })
            .unwrap_or_default();

        Some(Self {
            type_,
            properties,
            required,
            additional_properties: value
                .get("additionalProperties")
                .and_then(serde_json::Value::as_bool),
        })
    }

    /// 转换为 protocol 层 `InputSchema`
    pub fn to_protocol(&self) -> crate::mcp::InputSchema {
        let mut properties = serde_json::Map::new();
        for (k, v) in &self.properties {
            properties.insert(k.clone(), serde_json::to_value(v).unwrap_or_default());
        }
        crate::mcp::InputSchema {
            type_: self.type_.clone(),
            properties,
            required: self.required.clone(),
        }
    }
}

/// 参数属性 Schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPropertySchema {
    #[serde(rename = "type")]
    pub type_: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
    /// 数组元素类型
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<McpPropertySchema>>,
}

impl McpPropertySchema {
    /// 从 JSON Value 解析，失败时返回 fallback（type=string, 无 description）
    pub fn from_value_or_fallback(value: &serde_json::Value) -> Self {
        serde_json::from_value(value.clone()).unwrap_or_else(|_| Self {
            type_: value
                .get("type")
                .and_then(|t| t.as_str())
                .map(std::string::ToString::to_string),
            description: value
                .get("description")
                .and_then(|d| d.as_str())
                .map(std::string::ToString::to_string),
            default: None,
            enum_values: None,
            items: None,
        })
    }
}

/// MCP Category - 工具分类
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCategory {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tool_count: u32,
    #[serde(default)]
    pub tools: Vec<McpCategoryTool>,
}

/// Category 内的工具摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCategoryTool {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// `list_tool_categories` 的响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCategoriesResponse {
    pub categories: Vec<McpCategory>,
}

/// 完整的 Schema 集合（用于持久化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSchemaCollection {
    /// Schema 版本/更新时间
    pub version: String,
    /// 所有 category
    pub categories: Vec<McpCategory>,
    /// 所有 tool schema（按 name 索引）
    pub tools: HashMap<String, McpToolSchema>,
}

impl McpSchemaCollection {
    pub fn new() -> Self {
        Self {
            version: chrono::Utc::now().to_rfc3339(),
            categories: Vec::new(),
            tools: HashMap::new(),
        }
    }

    /// 从 category 列表构建
    pub fn from_categories(categories: Vec<McpCategory>) -> Self {
        let mut tools = HashMap::new();

        for category in &categories {
            let namespace = extract_namespace(&category.name);

            for tool in &category.tools {
                let command_name = extract_command_name(&tool.name, &namespace);

                let schema = McpToolSchema {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                    input_schema: None, // 需要单独获取完整 schema
                    output_schema: None,
                    namespace: Some(namespace.clone()),
                    command_name: Some(command_name),
                };

                tools.insert(tool.name.clone(), schema);
            }
        }

        Self {
            version: chrono::Utc::now().to_rfc3339(),
            categories,
            tools,
        }
    }

    /// 获取所有 namespace
    pub fn get_namespaces(&self) -> Vec<String> {
        self.categories
            .iter()
            .map(|c| extract_namespace(&c.name))
            .collect()
    }

    /// 获取 namespace 下的所有工具
    /// 支持完整 category 名 (如 "ai.ppt") 或 namespace (如 "ppt")
    pub fn get_tools_by_namespace(&self, namespace: &str) -> Vec<&McpToolSchema> {
        // 如果输入包含点，则先提取 namespace
        let ns = extract_namespace(namespace);
        self.tools
            .values()
            .filter(|t| t.namespace.as_deref() == Some(&ns))
            .collect()
    }
}

/// 子命令唯一匹配的结果
#[allow(dead_code)]
pub struct ResolvedSubcommand {
    /// 匹配到的 namespace
    pub namespace: String,
}

impl McpSchemaCollection {
    /// 尝试将一个裸子命令名解析为某个 namespace 下的唯一命令。
    ///
    /// 例如 "whoami" 只在 contact namespace 下存在，则返回 Some(("contact", "whoami"))。
    /// 如果有多个 namespace 都包含同名命令，则返回 None（有歧义）。
    pub fn resolve_unique_subcommand(&self, subcommand: &str) -> Option<ResolvedSubcommand> {
        let mut matches: Vec<(String, String)> = Vec::new();

        for category in &self.categories {
            let namespace = extract_namespace(&category.name);
            for tool in &category.tools {
                let cmd_name = extract_command_name(&tool.name, &namespace);
                if cmd_name == subcommand {
                    matches.push((namespace.clone(), cmd_name));
                }
            }
        }

        if matches.len() == 1 {
            let (namespace, _command_name) = matches.into_iter().next().unwrap();
            Some(ResolvedSubcommand { namespace })
        } else {
            None
        }
    }
}

impl Default for McpSchemaCollection {
    fn default() -> Self {
        Self::new()
    }
}

/// 从 category name 提取 namespace
/// "teamspace.team" -> "team"
/// "knowledge.space" -> "space"
/// "ai.ppt" -> "ppt"
/// "contact" -> "contact"
pub fn extract_namespace(category: &str) -> String {
    category.rsplit('.').next().unwrap_or(category).to_string()
}

/// 从 tool name 提取命令名
/// "`team_list_teams`" -> "list"
/// "`space_describe_space`" -> "describe"
/// "`entry_create_entry`" -> "create"
/// "`block_create_block_descendant`" -> "create-descendant"
/// "`search_kb_search`" -> "kb"
/// "`tx_meeting_import_tx_meeting_record`" -> "import-record"
pub fn extract_command_name(tool_name: &str, namespace: &str) -> String {
    let parts: Vec<&str> = tool_name.split('_').collect();

    if parts.len() < 2 {
        return tool_name.replace('_', "-");
    }

    // 特殊处理常见模式
    // 1. {namespace}_{action}_{namespace}s -> {action}
    //    team_list_teams -> list
    //    space_describe_space -> describe
    // 2. {namespace}_{action}_{target} -> {action}-{target}
    //    block_create_block_descendant -> create-descendant
    // 3. search_kb_search -> kb
    // 4. tx_meeting_* -> 去掉 tx_meeting_ 前缀，并移除内部的 tx_meeting

    let namespace_lower = namespace.to_lowercase();

    // 跳过前缀（namespace 或 tx_meeting 等）
    let skip_count = if parts[0] == "tx" && parts.len() > 2 && parts[1] == "meeting" {
        2 // tx_meeting_*
    } else if parts[0] == namespace_lower
        || parts[0] == &namespace_lower[..namespace_lower.len().min(parts[0].len())]
    {
        1 // namespace_*
    } else {
        0
    };

    let remaining: Vec<&str> = parts.iter().skip(skip_count).copied().collect();

    if remaining.is_empty() {
        return tool_name.replace('_', "-");
    }

    // 检查是否是 {action}_{namespace}s 模式
    if remaining.len() == 2 {
        let action = remaining[0];
        let target = remaining[1];

        // 如果 target 是 namespace 的复数形式，只保留 action
        if target == format!("{}s", namespace_lower) || target == namespace_lower {
            return action.to_string();
        }
    }

    // 构建需要过滤的词列表
    let mut remove_words: Vec<String> =
        vec![namespace_lower.clone(), format!("{}s", namespace_lower)];
    // tx_meeting 的特殊处理：同时移除 tx 和 meeting
    if parts.len() > 2 && parts[0] == "tx" && parts[1] == "meeting" {
        remove_words.push("tx".to_string());
        remove_words.push("meeting".to_string());
    }

    // 移除重复的 namespace 词
    let filtered: Vec<&str> = remaining
        .iter()
        .filter(|&p| !remove_words.contains(&(*p).to_string()))
        .copied()
        .collect();

    if filtered.is_empty() {
        remaining.join("-")
    } else {
        filtered.join("-")
    }
}

/// 将参数名从 `snake_case` 转换为 kebab-case
pub fn to_kebab_case(s: &str) -> String {
    s.replace('_', "-")
}

/// 将参数名从 kebab-case 转换回 `snake_case`
#[allow(dead_code)]
pub fn to_snake_case(s: &str) -> String {
    s.replace('-', "_")
}

/// Unlisted tools schema collection.
///
/// These tools are NOT returned by `tools/list` but CAN be invoked via `tools/call`.
/// Maintained separately in `schemas/unlisted.json` and protected from `sync` overwrites.
///
/// `tool_names` is the source of truth (config-as-code).
/// `tools` is auto-populated by `lx tools sync-unlisted`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnlistedSchemaCollection {
    #[serde(default)]
    pub _comment: Option<String>,
    /// The authoritative list of unlisted tool names (config-as-code).
    /// `lx tools sync-unlisted` reads this list, fetches each tool's schema
    /// from the MCP server, and writes the result into `tools`.
    #[serde(default)]
    pub tool_names: Vec<String>,
    /// Auto-populated tool schemas keyed by tool name.
    #[serde(default)]
    pub tools: HashMap<String, McpToolSchema>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_namespace() {
        assert_eq!(extract_namespace("teamspace.team"), "team");
        assert_eq!(extract_namespace("knowledge.space"), "space");
        assert_eq!(extract_namespace("knowledge.entry"), "entry");
        assert_eq!(extract_namespace("ai.ppt"), "ppt");
        assert_eq!(extract_namespace("contact"), "contact");
        assert_eq!(extract_namespace("connector.meeting"), "meeting");
    }

    #[test]
    fn test_extract_command_name() {
        // Standard patterns
        assert_eq!(extract_command_name("team_list_teams", "team"), "list");
        assert_eq!(
            extract_command_name("team_describe_team", "team"),
            "describe"
        );
        assert_eq!(extract_command_name("space_list_spaces", "space"), "list");
        assert_eq!(
            extract_command_name("entry_create_entry", "entry"),
            "create"
        );

        // Block patterns
        assert_eq!(
            extract_command_name("block_create_block_descendant", "block"),
            "create-descendant"
        );
        assert_eq!(
            extract_command_name("block_list_block_children", "block"),
            "list-children"
        );

        // Search patterns
        assert_eq!(extract_command_name("search_kb_search", "search"), "kb");

        // Meeting patterns (tx_meeting_ prefix)
        assert_eq!(
            extract_command_name("tx_meeting_import_tx_meeting_record", "meeting"),
            "import-record"
        );
    }
}
