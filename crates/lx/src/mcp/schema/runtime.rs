use crate::datadir;
use crate::mcp::schema::types::{ListCategoriesResponse, McpSchemaCollection};
use crate::mcp::McpClient;
use anyhow::{Context, Result};
use std::path::PathBuf;

const OVERRIDE_FILE: &str = "tools/override.json";
const CUSTOM_FILE: &str = "tools/custom.json";

/// Schema 运行时管理器
pub struct RuntimeSchemaManager {
    /// 配置目录 (~/.lexiang/)
    config_dir: PathBuf,
}

impl RuntimeSchemaManager {
    pub fn new() -> Self {
        Self {
            config_dir: datadir::datadir(),
        }
    }

    /// 获取 override schema 路径
    fn override_path(&self) -> PathBuf {
        self.config_dir.join(OVERRIDE_FILE)
    }

    /// 获取 custom schema 路径
    fn custom_path(&self) -> PathBuf {
        self.config_dir.join(CUSTOM_FILE)
    }

    /// 加载运行时 schema（override + custom）
    pub fn load(&self) -> Result<Option<McpSchemaCollection>> {
        // 先尝试加载 override
        let mut schema = self.load_override()?;

        // 然后加载 custom 覆盖
        if let Some(custom) = self.load_custom()? {
            if let Some(ref mut base) = schema {
                // 合并 custom 到 base
                for (name, tool) in custom.tools {
                    base.tools.insert(name, tool);
                }
            } else {
                schema = Some(custom);
            }
        }

        Ok(schema)
    }

    /// 加载 override schema
    fn load_override(&self) -> Result<Option<McpSchemaCollection>> {
        let path = self.override_path();
        if !path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let schema: McpSchemaCollection = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        Ok(Some(schema))
    }

    /// 加载 custom schema
    fn load_custom(&self) -> Result<Option<McpSchemaCollection>> {
        let path = self.custom_path();
        if !path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let schema: McpSchemaCollection = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        Ok(Some(schema))
    }

    /// 保存 schema 到 override 文件
    pub fn save_override(&self, schema: &McpSchemaCollection) -> Result<()> {
        let path = self.override_path();

        // 确保目录存在
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {}", parent.display()))?;
        }

        let content = serde_json::to_string_pretty(schema)?;
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write {}", path.display()))?;

        Ok(())
    }

    /// 从 MCP Server 同步最新 schema
    pub async fn sync_from_server(&self, client: &McpClient) -> Result<McpSchemaCollection> {
        // 1. 获取所有 category
        let categories_result: ListCategoriesResponse = client
            .call_raw("list_tool_categories", serde_json::json!({}))
            .await
            .context("Failed to fetch tool categories")?;

        // 2. 构建 schema collection
        let mut schema = McpSchemaCollection::from_categories(categories_result.categories);

        // 3. 获取每个 tool 的完整 schema
        let tools = client.list_tools().await?;
        for tool in tools {
            if let Some(existing) = schema.tools.get_mut(&tool.name) {
                let full = crate::mcp::schema::types::McpToolSchema::from_protocol(&tool);
                existing.input_schema = full.input_schema;
            }
        }

        // 4. 合并 unlisted tools（编译时嵌入的，不会被 sync 覆盖）
        let unlisted = super::embedded::load_unlisted_schemas();
        let unlisted_count = unlisted.len();
        for (name, tool) in unlisted {
            schema
                .tools
                .entry(name)
                .or_insert_with(|| crate::mcp::schema::types::McpToolSchema::from_protocol(&tool));
        }
        if unlisted_count > 0 {
            eprintln!(
                "  Preserved {} unlisted tool(s) from schemas/unlisted.json",
                unlisted_count
            );
        }

        // 5. 保存到 override
        self.save_override(&schema)?;

        Ok(schema)
    }

    /// 获取 schema 版本信息
    pub fn get_version_info(&self) -> SchemaVersionInfo {
        let override_version = self.load_override().ok().flatten().map(|s| s.version);

        let custom_modified = self
            .custom_path()
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());

        SchemaVersionInfo {
            embedded_version: env!("CARGO_PKG_VERSION").to_string(),
            override_version,
            custom_modified,
        }
    }
}

impl Default for RuntimeSchemaManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Schema 版本信息
#[derive(Debug)]
pub struct SchemaVersionInfo {
    pub embedded_version: String,
    pub override_version: Option<String>,
    pub custom_modified: Option<String>,
}

impl std::fmt::Display for SchemaVersionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Schema Versions:")?;
        writeln!(f, "  Embedded: {} (built-in)", self.embedded_version)?;
        if let Some(ref v) = self.override_version {
            writeln!(f, "  Override: {} (synced)", v)?;
        } else {
            writeln!(f, "  Override: (not synced)")?;
        }
        if let Some(ref m) = self.custom_modified {
            writeln!(f, "  Custom:   {} (user)", m)?;
        }
        Ok(())
    }
}
