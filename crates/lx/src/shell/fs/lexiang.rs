//! `LexiangFs`: 乐享知识库 MCP 后端文件系统
//!
//! 将 `IFileSystem` trait 映射到乐享 MCP API:
//! - `read_file(path)` → `block_list_block_children` (读取页面内容)
//! - `read_dir(path)` → `entry_list_children` (列出子条目)
//! - `stat(path)` → `entry_describe_entry` (获取条目详情)
//! - `exists(path)` → `entry_describe_entry` (检查是否存在)
//!
//! 路径格式: `/kb/{space_name}/{folder}/{page}`
//! 通过 `PathResolver` 将路径翻译为 `entry_id`

use super::types::*;
use super::IFileSystem;
use crate::shell::fs::normalize_path;
use anyhow::{bail, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Mutex;

// ═══════════════════════════════════════════════════════════
//  McpCaller re-export (定义在 mcp/caller.rs)
// ═══════════════════════════════════════════════════════════
pub use crate::mcp::caller::{McpCaller, RealMcpCaller};

// ═══════════════════════════════════════════════════════════
//  MCP API 数据结构
// ═══════════════════════════════════════════════════════════

/// MCP `entry_describe_entry` 返回结构
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpEntry {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub entry_type: String, // "page" | "folder" | "file"
    #[serde(default)]
    pub target_id: Option<String>,
    #[serde(default)]
    pub space_id: Option<String>,
    #[serde(default)]
    pub has_children: Option<bool>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// MCP `entry_list_children` 返回结构
#[derive(Debug, Clone, Deserialize)]
pub struct McpEntryListResult {
    #[serde(default)]
    pub entries: Vec<McpEntry>,
}

/// MCP block 结构
#[derive(Debug, Clone, Deserialize)]
pub struct McpBlock {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub block_type: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub children: Option<Vec<McpBlock>>,
}

/// MCP `block_list_block_children` 返回
#[derive(Debug, Clone, Deserialize)]
pub struct McpBlockListResult {
    #[serde(default)]
    pub blocks: Vec<McpBlock>,
}

/// MCP `space_describe_space` 返回
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpSpace {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub root_entry_id: Option<String>,
    #[serde(default)]
    pub team_id: Option<String>,
}

/// MCP `entry_describe_ai_parse_content` 返回
#[derive(Debug, Clone, Deserialize)]
pub struct McpAiParseResult {
    #[serde(default)]
    pub markdown: Option<String>,
    #[serde(default)]
    pub html: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
}

// ═══════════════════════════════════════════════════════════
//  PathResolver: 路径 ↔ entry_id 双向映射
// ═══════════════════════════════════════════════════════════

/// 缓存条目
#[derive(Debug, Clone)]
struct CacheEntry {
    entry: McpEntry,
    children_loaded: bool,
}

/// 路径解析器 — 将虚拟路径翻译为 `entry_id，带` LRU 缓存
pub struct PathResolver {
    /// `space_id` → space 信息
    space: McpSpace,
    /// path → entry 映射缓存
    path_cache: Mutex<HashMap<String, CacheEntry>>,
    /// `entry_id` → path 反向映射
    id_to_path: Mutex<HashMap<String, String>>,
}

impl PathResolver {
    /// 创建 `PathResolver`
    pub fn new(space: McpSpace) -> Self {
        let mut path_cache = HashMap::new();
        let mut id_to_path = HashMap::new();

        // 根目录即 root_entry_id
        if let Some(ref root_id) = space.root_entry_id {
            let root_entry = McpEntry {
                id: root_id.clone(),
                name: space.name.clone(),
                entry_type: "folder".to_string(),
                target_id: None,
                space_id: Some(space.id.clone()),
                has_children: Some(true),
                created_at: None,
                updated_at: None,
            };
            path_cache.insert(
                "/".to_string(),
                CacheEntry {
                    entry: root_entry,
                    children_loaded: false,
                },
            );
            id_to_path.insert(root_id.clone(), "/".to_string());
        }

        Self {
            space,
            path_cache: Mutex::new(path_cache),
            id_to_path: Mutex::new(id_to_path),
        }
    }

    /// 路径 → `entry_id` (先查缓存，再查 MCP)
    pub async fn resolve_path(&self, path: &str, mcp: &dyn McpCaller) -> Result<McpEntry> {
        let normalized = normalize_path(path);

        // 1. 缓存命中
        {
            let cache = self.path_cache.lock().unwrap();
            if let Some(cached) = cache.get(&normalized) {
                return Ok(cached.entry.clone());
            }
        }

        // 2. 逐级解析路径
        let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();

        let mut current_path = "/".to_string();
        let mut current_entry = {
            let cache = self.path_cache.lock().unwrap();
            cache
                .get("/")
                .map(|c| c.entry.clone())
                .ok_or_else(|| anyhow::anyhow!("root entry not found in cache"))?
        };

        for part in &parts {
            // 检查当前目录的子项是否已加载
            let children_loaded = {
                let cache = self.path_cache.lock().unwrap();
                cache
                    .get(&current_path)
                    .map(|c| c.children_loaded)
                    .unwrap_or(false)
            };

            if !children_loaded {
                // 加载子项
                self.load_children(&current_path, &current_entry.id, mcp)
                    .await?;
            }

            // 查找子项
            let child_path = if current_path == "/" {
                format!("/{part}")
            } else {
                format!("{current_path}/{part}")
            };

            current_entry = {
                let cache = self.path_cache.lock().unwrap();
                cache
                    .get(&child_path)
                    .map(|c| c.entry.clone())
                    .ok_or_else(|| anyhow::anyhow!("No such file or directory: {child_path}"))?
            };

            current_path = child_path;
        }

        Ok(current_entry)
    }

    /// `entry_id` → 路径
    pub fn resolve_id(&self, entry_id: &str) -> Option<String> {
        let id_map = self.id_to_path.lock().unwrap();
        id_map.get(entry_id).cloned()
    }

    /// 加载某个目录的子项到缓存
    async fn load_children(
        &self,
        parent_path: &str,
        parent_id: &str,
        mcp: &dyn McpCaller,
    ) -> Result<Vec<McpEntry>> {
        let result = mcp
            .call_tool(
                "entry_list_children",
                serde_json::json!({
                    "parent_id": parent_id,
                }),
            )
            .await?;

        // MCP 返回可能嵌套: { "data": { "entries": [...] } } 或 { "entries": [...] } 或直接 [...]
        let entries: Vec<McpEntry> = if let Some(entries_val) = result
            .get("data")
            .and_then(|d| d.get("entries"))
            .or_else(|| result.get("entries"))
        {
            serde_json::from_value(entries_val.clone()).unwrap_or_default()
        } else if result.is_array() {
            serde_json::from_value(result.clone()).unwrap_or_default()
        } else if let Some(data) = result.get("data") {
            if data.is_array() {
                serde_json::from_value(data.clone()).unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // 更新缓存
        {
            let mut cache = self.path_cache.lock().unwrap();
            let mut id_map = self.id_to_path.lock().unwrap();

            // 标记父目录已加载
            if let Some(parent_cache) = cache.get_mut(parent_path) {
                parent_cache.children_loaded = true;
            }

            for entry in &entries {
                let child_path = if parent_path == "/" {
                    format!("/{}", entry.name)
                } else {
                    format!("{}/{}", parent_path, entry.name)
                };

                cache.insert(
                    child_path.clone(),
                    CacheEntry {
                        entry: entry.clone(),
                        children_loaded: false,
                    },
                );
                id_map.insert(entry.id.clone(), child_path);
            }
        }

        Ok(entries)
    }

    /// 获取已缓存的子项 (不发 MCP 调用)
    pub fn cached_children(&self, path: &str) -> Option<Vec<McpEntry>> {
        let normalized = normalize_path(path);
        let cache = self.path_cache.lock().unwrap();

        let prefix = if normalized == "/" {
            "/".to_string()
        } else {
            format!("{normalized}/")
        };

        let mut children = Vec::new();
        for (p, c) in cache.iter() {
            if p == &normalized {
                continue;
            }
            if normalized == "/" {
                // 根目录：只匹配 /xxx (不含子路径)
                let rest = p.trim_start_matches('/');
                if !rest.is_empty() && !rest.contains('/') {
                    children.push(c.entry.clone());
                }
            } else if let Some(rest) = p.strip_prefix(&prefix) {
                if !rest.contains('/') {
                    children.push(c.entry.clone());
                }
            }
        }

        if children.is_empty() {
            None
        } else {
            children.sort_by(|a, b| a.name.cmp(&b.name));
            Some(children)
        }
    }

    /// 使某个路径的缓存失效
    pub fn invalidate(&self, path: &str) {
        let normalized = normalize_path(path);
        let mut cache = self.path_cache.lock().unwrap();
        let mut id_map = self.id_to_path.lock().unwrap();

        // 移除该路径及其所有子路径
        let prefix = format!("{normalized}/");
        let to_remove: Vec<String> = cache
            .keys()
            .filter(|k| *k == &normalized || k.starts_with(&prefix))
            .cloned()
            .collect();

        for key in &to_remove {
            if let Some(removed) = cache.remove(key) {
                id_map.remove(&removed.entry.id);
            }
        }

        // 标记父目录需要重新加载
        let parent = super::parent_path(&normalized);
        if let Some(parent_cache) = cache.get_mut(&parent) {
            parent_cache.children_loaded = false;
        }
    }

    /// 获取 space 信息
    pub fn space(&self) -> &McpSpace {
        &self.space
    }
}

// ═══════════════════════════════════════════════════════════
//  LexiangFs: IFileSystem 实现
// ═══════════════════════════════════════════════════════════

/// 乐享知识库文件系统
///
/// 将 `IFileSystem` 的文件操作映射到 MCP API:
/// - 目录 = folder entry
/// - 文件 = page entry (内容为 markdown)
///
/// # 路径格式
/// ```text
/// /                     → space root (root_entry_id)
/// /产品文档              → folder "产品文档"
/// /产品文档/API说明.md    → page "API说明"
/// ```
pub struct LexiangFs {
    resolver: PathResolver,
    mcp: Box<dyn McpCaller>,
}

impl LexiangFs {
    /// 创建新的 `LexiangFs`
    pub fn new(space: McpSpace, mcp: Box<dyn McpCaller>) -> Self {
        Self {
            resolver: PathResolver::new(space),
            mcp,
        }
    }

    /// 获取 `PathResolver` 引用 (用于外部路径查询)
    pub fn resolver(&self) -> &PathResolver {
        &self.resolver
    }

    /// 读取页面内容 (`entry_describe_ai_parse_content` → markdown)
    async fn read_page_content(&self, entry_id: &str) -> Result<String> {
        let result = self
            .mcp
            .call_tool(
                "entry_describe_ai_parse_content",
                serde_json::json!({
                    "entry_id": entry_id,
                }),
            )
            .await?;

        // MCP 返回可能嵌套在 data 中: { "data": { "markdown": "..." } }
        let content = result.get("data").unwrap_or(&result);

        // 优先 markdown，其次 text，最后 html
        if let Some(md) = content.get("markdown").and_then(|v| v.as_str()) {
            if !md.is_empty() {
                return Ok(md.to_string());
            }
        }
        if let Some(text) = content.get("text").and_then(|v| v.as_str()) {
            if !text.is_empty() {
                return Ok(text.to_string());
            }
        }
        if let Some(html) = content.get("html").and_then(|v| v.as_str()) {
            return Ok(html.to_string());
        }

        Ok(String::new())
    }

    /// 将 `McpEntry` 转换为 `DirEntry`
    fn mcp_entry_to_dir_entry(entry: &McpEntry) -> DirEntry {
        let file_type = match entry.entry_type.as_str() {
            "folder" => FileType::Directory,
            _ => FileType::File, // page, file 都视为文件
        };

        let name = if file_type == FileType::File && !entry.name.contains('.') {
            // 页面没有扩展名时，自动加 .md
            format!("{}.md", entry.name)
        } else {
            entry.name.clone()
        };

        DirEntry {
            name,
            file_type,
            size: 0,
            modified: None,
            metadata: Some(EntryMetadata {
                entry_id: Some(entry.id.clone()),
                space_id: entry.space_id.clone(),
                entry_type: Some(entry.entry_type.clone()),
                creator: None,
            }),
        }
    }

    /// 将 `McpEntry` 转换为 `FileStat`
    fn mcp_entry_to_stat(entry: &McpEntry) -> FileStat {
        let file_type = match entry.entry_type.as_str() {
            "folder" => FileType::Directory,
            _ => FileType::File,
        };

        FileStat {
            file_type,
            size: 0,
            created: None,
            modified: None,
            accessed: None,
            readonly: true, // 知识库默认只读
            metadata: Some(EntryMetadata {
                entry_id: Some(entry.id.clone()),
                space_id: entry.space_id.clone(),
                entry_type: Some(entry.entry_type.clone()),
                creator: None,
            }),
        }
    }

    /// 解析路径，支持带 .md 后缀的页面名称查找
    async fn resolve_path_flex(&self, path: &str) -> Result<McpEntry> {
        let normalized = normalize_path(path);

        // 直接尝试
        if let Ok(entry) = self
            .resolver
            .resolve_path(&normalized, self.mcp.as_ref())
            .await
        {
            return Ok(entry);
        }

        // 如果路径以 .md 结尾，尝试去掉扩展名
        if normalized.ends_with(".md") {
            let without_ext = &normalized[..normalized.len() - 3];
            if let Ok(entry) = self
                .resolver
                .resolve_path(without_ext, self.mcp.as_ref())
                .await
            {
                return Ok(entry);
            }
        }

        // 如果路径不以 .md 结尾，尝试加上 .md
        let with_ext = format!("{normalized}.md");
        if let Ok(entry) = self
            .resolver
            .resolve_path(&with_ext, self.mcp.as_ref())
            .await
        {
            return Ok(entry);
        }

        bail!("No such file or directory: {normalized}")
    }
}

#[async_trait]
impl IFileSystem for LexiangFs {
    async fn read_file(&self, path: &str) -> Result<String> {
        let entry = self.resolve_path_flex(path).await?;

        match entry.entry_type.as_str() {
            "folder" => bail!("Is a directory: {path}"),
            "page" => self.read_page_content(&entry.id).await,
            "file" => {
                // 附件类型，尝试获取描述
                let result = self
                    .mcp
                    .call_tool(
                        "entry_describe_entry",
                        serde_json::json!({ "entry_id": entry.id }),
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&result)?)
            }
            other => bail!("Unsupported entry type: {other}"),
        }
    }

    async fn write_file(&self, _path: &str, _content: &str) -> Result<()> {
        bail!("EROFS: read-only file system (use MCP tools directly for write operations)")
    }

    async fn append_file(&self, _path: &str, _content: &str) -> Result<()> {
        bail!("EROFS: read-only file system")
    }

    async fn read_dir(&self, path: &str) -> Result<Vec<DirEntry>> {
        let normalized = normalize_path(path);
        let entry = self
            .resolver
            .resolve_path(&normalized, self.mcp.as_ref())
            .await?;

        if entry.entry_type != "folder" {
            bail!("Not a directory: {path}");
        }

        // 通过 MCP 加载子项
        let children = self
            .resolver
            .load_children(&normalized, &entry.id, self.mcp.as_ref())
            .await?;

        let mut entries: Vec<DirEntry> =
            children.iter().map(Self::mcp_entry_to_dir_entry).collect();

        // 排序：目录优先，然后按名称
        entries.sort_by(|a, b| {
            let type_ord = match (&a.file_type, &b.file_type) {
                (FileType::Directory, FileType::File) => std::cmp::Ordering::Less,
                (FileType::File, FileType::Directory) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            };
            type_ord.then_with(|| a.name.cmp(&b.name))
        });

        Ok(entries)
    }

    async fn stat(&self, path: &str) -> Result<FileStat> {
        let entry = self.resolve_path_flex(path).await?;
        Ok(Self::mcp_entry_to_stat(&entry))
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        match self.resolve_path_flex(path).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn mkdir(&self, _path: &str, _recursive: bool) -> Result<()> {
        bail!("EROFS: read-only file system")
    }

    async fn remove(&self, _path: &str, _recursive: bool) -> Result<()> {
        bail!("EROFS: read-only file system")
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ═══════════════════════════════════════════════════════════
//  测试
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Mock MCP 调用器 — 模拟知识库数据
    struct MockMcpCaller {
        call_count: AtomicUsize,
    }

    impl MockMcpCaller {
        fn new() -> Self {
            Self {
                call_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl McpCaller for MockMcpCaller {
        async fn call_tool(
            &self,
            tool_name: &str,
            args: serde_json::Value,
        ) -> Result<serde_json::Value> {
            self.call_count.fetch_add(1, Ordering::Relaxed);

            match tool_name {
                "entry_list_children" => {
                    let parent_id = args["parent_id"].as_str().unwrap_or("");
                    match parent_id {
                        "root_001" => Ok(serde_json::json!({
                            "entries": [
                                {
                                    "id": "folder_001",
                                    "name": "产品文档",
                                    "entry_type": "folder",
                                    "space_id": "space_001",
                                    "has_children": true
                                },
                                {
                                    "id": "page_001",
                                    "name": "README",
                                    "entry_type": "page",
                                    "space_id": "space_001",
                                    "has_children": false
                                }
                            ]
                        })),
                        "folder_001" => Ok(serde_json::json!({
                            "entries": [
                                {
                                    "id": "page_002",
                                    "name": "API说明",
                                    "entry_type": "page",
                                    "space_id": "space_001",
                                    "has_children": false
                                },
                                {
                                    "id": "page_003",
                                    "name": "部署指南",
                                    "entry_type": "page",
                                    "space_id": "space_001",
                                    "has_children": false
                                }
                            ]
                        })),
                        _ => Ok(serde_json::json!({ "entries": [] })),
                    }
                }
                "entry_describe_ai_parse_content" => {
                    let entry_id = args["entry_id"].as_str().unwrap_or("");
                    match entry_id {
                        "page_001" => Ok(serde_json::json!({
                            "markdown": "# README\n\nWelcome to the knowledge base.\n",
                            "text": "README\nWelcome to the knowledge base.",
                        })),
                        "page_002" => Ok(serde_json::json!({
                            "markdown": "# API说明\n\n## OAuth 2.0\n\nToken based authentication.\n",
                        })),
                        "page_003" => Ok(serde_json::json!({
                            "markdown": "# 部署指南\n\n## Docker\n\ndocker compose up\n",
                        })),
                        _ => Ok(serde_json::json!({ "markdown": "" })),
                    }
                }
                "entry_describe_entry" => {
                    let entry_id = args["entry_id"].as_str().unwrap_or("");
                    Ok(serde_json::json!({
                        "id": entry_id,
                        "name": "test",
                        "entry_type": "file",
                    }))
                }
                _ => bail!("Unknown tool: {tool_name}"),
            }
        }
    }

    fn mock_space() -> McpSpace {
        McpSpace {
            id: "space_001".to_string(),
            name: "测试知识库".to_string(),
            root_entry_id: Some("root_001".to_string()),
            team_id: Some("team_001".to_string()),
        }
    }

    fn create_test_fs() -> LexiangFs {
        LexiangFs::new(mock_space(), Box::new(MockMcpCaller::new()))
    }

    // ── PathResolver 测试 ──

    #[tokio::test]
    async fn test_resolver_root() {
        let resolver = PathResolver::new(mock_space());
        let mcp = MockMcpCaller::new();
        let entry = resolver.resolve_path("/", &mcp).await.unwrap();
        assert_eq!(entry.id, "root_001");
    }

    #[tokio::test]
    async fn test_resolver_first_level() {
        let resolver = PathResolver::new(mock_space());
        let mcp = MockMcpCaller::new();

        let entry = resolver.resolve_path("/产品文档", &mcp).await.unwrap();
        assert_eq!(entry.id, "folder_001");
        assert_eq!(entry.entry_type, "folder");
    }

    #[tokio::test]
    async fn test_resolver_nested() {
        let resolver = PathResolver::new(mock_space());
        let mcp = MockMcpCaller::new();

        let entry = resolver
            .resolve_path("/产品文档/API说明", &mcp)
            .await
            .unwrap();
        assert_eq!(entry.id, "page_002");
        assert_eq!(entry.entry_type, "page");
    }

    #[tokio::test]
    async fn test_resolver_cache_hit() {
        let resolver = PathResolver::new(mock_space());
        let mcp = MockMcpCaller::new();

        // 第一次查询 (触发 MCP 调用)
        resolver.resolve_path("/产品文档", &mcp).await.unwrap();
        let count1 = mcp.call_count.load(Ordering::Relaxed);

        // 第二次查询 (应该命中缓存)
        resolver.resolve_path("/产品文档", &mcp).await.unwrap();
        let count2 = mcp.call_count.load(Ordering::Relaxed);

        assert_eq!(count1, count2, "second resolve should hit cache");
    }

    #[tokio::test]
    async fn test_resolver_id_to_path() {
        let resolver = PathResolver::new(mock_space());
        let mcp = MockMcpCaller::new();

        resolver
            .resolve_path("/产品文档/API说明", &mcp)
            .await
            .unwrap();

        let path = resolver.resolve_id("page_002");
        assert_eq!(path, Some("/产品文档/API说明".to_string()));
    }

    #[tokio::test]
    async fn test_resolver_not_found() {
        let resolver = PathResolver::new(mock_space());
        let mcp = MockMcpCaller::new();

        let result = resolver.resolve_path("/不存在的目录", &mcp).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resolver_invalidate() {
        let resolver = PathResolver::new(mock_space());
        let mcp = MockMcpCaller::new();

        resolver.resolve_path("/产品文档", &mcp).await.unwrap();
        resolver.invalidate("/产品文档");

        // 缓存应该已失效，resolve_id 返回 None
        assert!(resolver.resolve_id("folder_001").is_none());
    }

    // ── LexiangFs 测试 ──

    #[tokio::test]
    async fn test_fs_read_dir_root() {
        let fs = create_test_fs();
        let entries = fs.read_dir("/").await.unwrap();

        assert_eq!(entries.len(), 2);
        // 目录优先
        assert_eq!(entries[0].name, "产品文档");
        assert!(entries[0].file_type.is_dir());
        // 页面文件 (自动加 .md)
        assert_eq!(entries[1].name, "README.md");
        assert!(entries[1].file_type.is_file());
    }

    #[tokio::test]
    async fn test_fs_read_dir_subfolder() {
        let fs = create_test_fs();
        let entries = fs.read_dir("/产品文档").await.unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "API说明.md");
        assert_eq!(entries[1].name, "部署指南.md");
    }

    #[tokio::test]
    async fn test_fs_read_file() {
        let fs = create_test_fs();
        let content = fs.read_file("/README").await.unwrap();
        assert!(content.contains("# README"));
        assert!(content.contains("Welcome"));
    }

    #[tokio::test]
    async fn test_fs_read_file_with_md_ext() {
        let fs = create_test_fs();
        // 带 .md 后缀也应该能找到
        let content = fs.read_file("/README.md").await.unwrap();
        assert!(content.contains("# README"));
    }

    #[tokio::test]
    async fn test_fs_read_nested_file() {
        let fs = create_test_fs();
        let content = fs.read_file("/产品文档/API说明").await.unwrap();
        assert!(content.contains("# API说明"));
        assert!(content.contains("OAuth 2.0"));
    }

    #[tokio::test]
    async fn test_fs_stat_folder() {
        let fs = create_test_fs();
        let stat = fs.stat("/产品文档").await.unwrap();
        assert!(stat.file_type.is_dir());
        assert!(stat.readonly);
        assert!(stat.metadata.is_some());
    }

    #[tokio::test]
    async fn test_fs_stat_page() {
        let fs = create_test_fs();
        let stat = fs.stat("/README").await.unwrap();
        assert!(stat.file_type.is_file());
        assert!(stat.metadata.unwrap().entry_id == Some("page_001".to_string()));
    }

    #[tokio::test]
    async fn test_fs_exists() {
        let fs = create_test_fs();
        assert!(fs.exists("/").await.unwrap());
        assert!(fs.exists("/产品文档").await.unwrap());
        assert!(fs.exists("/README").await.unwrap());
        assert!(!fs.exists("/不存在").await.unwrap());
    }

    #[tokio::test]
    async fn test_fs_read_only() {
        let fs = create_test_fs();
        assert!(fs.is_read_only());
        assert!(fs.write_file("/test.md", "hello").await.is_err());
        assert!(fs.mkdir("/dir", false).await.is_err());
        assert!(fs.remove("/README", false).await.is_err());
    }

    #[tokio::test]
    async fn test_fs_read_dir_not_a_dir() {
        let fs = create_test_fs();
        let result = fs.read_dir("/README").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fs_read_dir_of_folder_is_dir() {
        let fs = create_test_fs();
        let result = fs.read_file("/产品文档").await;
        assert!(result.is_err());
    }

    // ── 集成: LexiangFs + Bash ──

    #[tokio::test]
    async fn test_fs_with_bash() {
        use crate::shell::bash::Bash;

        let fs = create_test_fs();
        let mut bash = Bash::new(Box::new(fs)).with_cwd("/");

        // ls /
        let output = bash.exec("ls /").await.unwrap();
        assert!(output.stdout.contains("产品文档"));
        assert!(output.stdout.contains("README.md"));

        // cat 读取页面
        let output = bash.exec("cat /README").await.unwrap();
        assert!(output.stdout.contains("# README"));

        // tree
        let output = bash.exec("tree /").await.unwrap();
        assert!(output.stdout.contains("产品文档"));
        assert!(output.stdout.contains("README"));

        // grep
        let output = bash.exec("grep -r OAuth /产品文档").await.unwrap();
        assert!(output.stdout.contains("OAuth"));

        // 管道
        let output = bash.exec("ls /产品文档 | grep API").await.unwrap();
        assert!(output.stdout.contains("API"));

        // find
        let output = bash.exec("find / -name '*.md' -type f").await.unwrap();
        assert!(output.stdout.contains("README"));
    }
}
