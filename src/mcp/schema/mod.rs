pub mod dynamic;
pub mod embedded;
pub mod generator;
pub mod runtime;
pub mod types;

pub use generator::{build_tool_args, CommandGenerator};
pub use runtime::RuntimeSchemaManager;
pub use types::*;

use crate::mcp::ToolSchema;
use std::collections::HashMap;

/// Schema manager - merges multi-layer schemas (embedded + unlisted < override < custom < dynamic)
pub struct SchemaManager {
    /// Schemas embedded at compile time (includes unlisted tools)
    embedded: HashMap<String, ToolSchema>,
    /// Schemas fetched dynamically at runtime
    dynamic: HashMap<String, ToolSchema>,
    /// Complete schema collection (includes category information)
    collection: Option<McpSchemaCollection>,
}

impl SchemaManager {
    pub fn new() -> Self {
        Self {
            embedded: embedded::load_embedded_schemas(),
            dynamic: HashMap::new(),
            collection: None,
        }
    }

    /// Load from runtime configuration
    pub fn load_from_runtime() -> Self {
        let mut manager = Self::new();

        // 尝试加载运行时 schema
        let runtime = RuntimeSchemaManager::new();
        if let Ok(Some(collection)) = runtime.load() {
            manager.collection = Some(collection);
        } else {
            // Fallback 到 embedded schema
            manager.collection = embedded::load_embedded_collection();
        }

        manager
    }

    /// 获取工具 schema（兼容旧接口）
    #[allow(dead_code)]
    pub fn get_tool_schema(&self, name: &str) -> Option<ToolSchema> {
        self.dynamic
            .get(name)
            .cloned()
            .or_else(|| self.embedded.get(name).cloned())
    }

    /// 更新动态 schema（兼容旧接口）
    pub fn update_dynamic(&mut self, tools: Vec<ToolSchema>) {
        self.dynamic.clear();
        for tool in tools {
            self.dynamic.insert(tool.name.clone(), tool);
        }
    }

    /// 从 outputSchema 提取全量字段路径列表，用于 `_mcp_fields` 参数。
    ///
    /// MCP server 仅默认返回 `x-default-returned: true` 的字段，
    /// CLI 需要通过 `_mcp_fields` 请求全量字段（如 `created_at`、`edited_at`），
    /// 然后在客户端侧做筛选/过滤。
    pub fn extract_fields(&self, name: &str) -> Vec<String> {
        // 优先从 collection（包含 outputSchema）获取
        if let Some(mcp_schema) = self.get_mcp_tool_schema(name) {
            if let Some(ref output_schema) = mcp_schema.output_schema {
                return extract_output_fields(output_schema);
            }
        }
        Vec::new()
    }

    // === 新接口 ===

    /// 获取完整的 `McpToolSchema`
    #[allow(dead_code)]
    pub fn get_mcp_tool_schema(&self, name: &str) -> Option<&McpToolSchema> {
        self.collection.as_ref()?.tools.get(name)
    }

    /// 获取 namespace 下的所有工具
    pub fn get_tools_by_namespace(&self, namespace: &str) -> Vec<&McpToolSchema> {
        self.collection
            .as_ref()
            .map(|c| c.get_tools_by_namespace(namespace))
            .unwrap_or_default()
    }

    /// 获取所有 category
    pub fn get_categories(&self) -> Vec<&McpCategory> {
        self.collection
            .as_ref()
            .map(|c| c.categories.iter().collect())
            .unwrap_or_default()
    }

    /// 设置 schema collection
    #[allow(dead_code)]
    pub fn set_collection(&mut self, collection: McpSchemaCollection) {
        self.collection = Some(collection);
    }

    /// 检查是否有 schema
    #[allow(dead_code)]
    pub fn has_schema(&self) -> bool {
        self.collection.is_some() || !self.embedded.is_empty() || !self.dynamic.is_empty()
    }
}

impl Default for SchemaManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 从 outputSchema JSON 递归提取全量字段路径。
///
/// 支持：
/// - 顶层 properties（如 `message`、`total_pages`）
/// - 嵌套 object 的 properties（如 `entries.id`、`entries.name`）
/// - array items 的 properties（`items.properties` → 展开为 `parent.child`）
/// - `$ref` / `$defs` 引用解析
fn extract_output_fields(schema: &serde_json::Value) -> Vec<String> {
    let mut fields = Vec::new();
    let defs = schema.get("$defs").or_else(|| schema.get("definitions"));
    collect_fields(schema, "", defs, &mut fields, 0);
    fields
}

fn collect_fields(
    schema: &serde_json::Value,
    prefix: &str,
    defs: Option<&serde_json::Value>,
    fields: &mut Vec<String>,
    depth: u8,
) {
    if depth > 5 {
        return; // 防止无限递归
    }

    // 处理 $ref
    if let Some(ref_str) = schema.get("$ref").and_then(|v| v.as_str()) {
        if let Some(resolved) = resolve_ref(ref_str, defs) {
            collect_fields(resolved, prefix, defs, fields, depth + 1);
        }
        return;
    }

    let type_str = schema.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match type_str {
        "object" => {
            if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
                for (key, prop_schema) in props {
                    let full_path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", prefix, key)
                    };

                    let prop_type = prop_schema.get("type").and_then(|v| v.as_str());
                    let has_ref = prop_schema.get("$ref").is_some();

                    match prop_type {
                        Some("object") if prop_schema.get("properties").is_some() => {
                            // 嵌套 object：递归展开
                            collect_fields(prop_schema, &full_path, defs, fields, depth + 1);
                        }
                        Some("array") => {
                            // array：收集自身路径，并展开 items
                            fields.push(full_path.clone());
                            if let Some(items) = prop_schema.get("items") {
                                collect_fields(items, &full_path, defs, fields, depth + 1);
                            }
                        }
                        _ if has_ref => {
                            // $ref：先添加自身路径，再解析引用
                            fields.push(full_path.clone());
                            if let Some(ref_str) = prop_schema.get("$ref").and_then(|v| v.as_str())
                            {
                                if let Some(resolved) = resolve_ref(ref_str, defs) {
                                    let resolved_type =
                                        resolved.get("type").and_then(|v| v.as_str());
                                    if resolved_type == Some("object")
                                        && resolved.get("properties").is_some()
                                    {
                                        collect_fields(
                                            resolved,
                                            &full_path,
                                            defs,
                                            fields,
                                            depth + 1,
                                        );
                                    }
                                }
                            }
                        }
                        _ => {
                            // 叶子节点（string, number, boolean 等）
                            fields.push(full_path);
                        }
                    }
                }
            }
        }
        "array" => {
            // 顶层 array：展开 items
            if let Some(items) = schema.get("items") {
                collect_fields(items, prefix, defs, fields, depth + 1);
            }
        }
        _ => {
            // 叶子类型，如果有 prefix 就加入
            if !prefix.is_empty() {
                fields.push(prefix.to_string());
            }
        }
    }
}

/// 解析 `$ref` 引用（支持 `#/$defs/Name` 格式）
fn resolve_ref<'a>(
    ref_str: &str,
    defs: Option<&'a serde_json::Value>,
) -> Option<&'a serde_json::Value> {
    let name = ref_str
        .strip_prefix("#/$defs/")
        .or_else(|| ref_str.strip_prefix("#/definitions/"))?;
    defs?.get(name)
}
