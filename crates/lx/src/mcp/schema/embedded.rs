use crate::mcp::schema::types::{
    extract_command_name, extract_namespace, McpSchemaCollection, UnlistedSchemaCollection,
};
use crate::mcp::ToolSchema;
use std::collections::HashMap;

/// 编译时嵌入的 schema JSON（源文件位于项目根目录 schemas/lexiang.json）
const EMBEDDED_SCHEMA_JSON: &str = include_str!("../../../../../schemas/lexiang.json");

/// 编译时嵌入的 unlisted tools schema（源文件位于 schemas/unlisted.json）
/// 这些工具不在 tools/list 中返回，但可以通过 tools/call 调用
const UNLISTED_SCHEMA_JSON: &str = include_str!("../../../../../schemas/unlisted.json");

/// 加载嵌入的 unlisted tools schema
pub fn load_unlisted_schemas() -> HashMap<String, ToolSchema> {
    let mut map = HashMap::new();

    if let Ok(unlisted) = serde_json::from_str::<UnlistedSchemaCollection>(UNLISTED_SCHEMA_JSON) {
        for (name, tool) in unlisted.tools {
            map.insert(name, tool.to_protocol());
        }
    }

    map
}

/// 加载嵌入的 schema 到 HashMap（兼容旧接口）
pub fn load_embedded_schemas() -> HashMap<String, ToolSchema> {
    let mut map = HashMap::new();

    if let Ok(collection) = serde_json::from_str::<McpSchemaCollection>(EMBEDDED_SCHEMA_JSON) {
        for (name, tool) in collection.tools {
            map.insert(name, tool.to_protocol());
        }
    }

    // 合并 unlisted tools（不覆盖已有的同名 tool）
    for (name, tool) in load_unlisted_schemas() {
        map.entry(name).or_insert(tool);
    }

    map
}

/// 加载嵌入的完整 schema collection
pub fn load_embedded_collection() -> Option<McpSchemaCollection> {
    let mut collection: McpSchemaCollection = serde_json::from_str(EMBEDDED_SCHEMA_JSON).ok()?;

    // 反序列化后 namespace 和 command_name 是 None（因为 #[serde(skip)]）
    // 需要从 categories 重新填充
    for category in &collection.categories {
        let namespace = extract_namespace(&category.name);
        for cat_tool in &category.tools {
            if let Some(tool) = collection.tools.get_mut(&cat_tool.name) {
                tool.namespace = Some(namespace.clone());
                tool.command_name = Some(extract_command_name(&cat_tool.name, &namespace));
            }
        }
    }

    // 合并 unlisted tools 到 collection.tools（仅 schema 查询用，不生成 CLI 命令）
    if let Ok(unlisted) = serde_json::from_str::<UnlistedSchemaCollection>(UNLISTED_SCHEMA_JSON) {
        for (name, tool) in unlisted.tools {
            collection.tools.entry(name).or_insert(tool);
        }
    }

    Some(collection)
}
