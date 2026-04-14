//! Worktree 同步模块
//!
//! 实现与远端 MCP 服务器的双向同步功能。

#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use super::config::WorktreeConfig;
use super::entries::{EntriesManager, EntriesMap, EntryType};

/// 远端 Entry 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteEntry {
    pub id: String,
    pub name: String,
    pub entry_type: String,
    pub has_children: bool,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub target_id: Option<String>,
}

/// 同步差异
#[derive(Debug, Default)]
pub struct SyncDiff {
    /// 需要从远端拉取的条目 (`entry_id`, `remote_path`)
    pub to_pull: Vec<(String, String)>,
    /// 本地已删除需要从远端删除的条目 (`entry_id`, `local_path`)
    pub to_delete_remote: Vec<(String, String)>,
    /// 本地新建需要推送到远端的条目 (`local_path`)
    pub to_push_new: Vec<String>,
    /// 本地修改需要推送到远端的条目 (`entry_id`, `local_path`)
    pub to_push_update: Vec<(String, String)>,
}

/// 同步器
pub struct WorktreeSync<'a> {
    worktree_path: &'a Path,
    config: &'a WorktreeConfig,
    entries: EntriesMap,
}

impl<'a> WorktreeSync<'a> {
    pub fn new(worktree_path: &'a Path, config: &'a WorktreeConfig) -> Result<Self> {
        let entries = EntriesManager::load(worktree_path)?;
        Ok(Self {
            worktree_path,
            config,
            entries,
        })
    }

    /// 保存 entries 映射
    pub fn save_entries(&self) -> Result<()> {
        EntriesManager::save(self.worktree_path, &self.entries)
    }

    /// 获取可变 entries 引用
    pub fn entries_mut(&mut self) -> &mut EntriesMap {
        &mut self.entries
    }

    /// 获取 entries 引用
    pub fn entries(&self) -> &EntriesMap {
        &self.entries
    }

    /// 获取 `space_id`
    pub fn space_id(&self) -> &str {
        &self.config.space_id
    }
}

/// 将远端 entry 类型转换为本地类型
#[allow(clippy::match_same_arms)]
pub fn parse_entry_type(entry_type: &str) -> EntryType {
    match entry_type {
        "folder" => EntryType::Folder,
        "page" => EntryType::Page,
        "file" => EntryType::File,
        "smartsheet" => EntryType::Smartsheet,
        _ => EntryType::Page, // 未知类型默认为 Page
    }
}

/// 将本地文件路径转为安全的文件名
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

/// 根据 entry 类型生成本地文件名
pub fn entry_to_filename(name: &str, entry_type: &EntryType) -> String {
    let safe_name = sanitize_filename(name);
    match entry_type {
        EntryType::Page => format!("{}.md", safe_name),
        EntryType::Smartsheet => format!("{}.csv", safe_name),
        EntryType::Folder | EntryType::File => safe_name,
    }
}

/// 从文件名推断 entry 类型
pub fn filename_to_entry_type(filename: &str) -> EntryType {
    if filename.ends_with(".md") {
        EntryType::Page
    } else if filename.ends_with(".csv") {
        EntryType::Smartsheet
    } else {
        EntryType::File
    }
}

/// 拉取统计
#[derive(Debug, Default)]
pub struct PullStats {
    pub folders_created: usize,
    pub pages_pulled: usize,
    pub files_pulled: usize,
    pub errors: Vec<String>,
}

/// 推送统计
#[derive(Debug, Default)]
pub struct PushStats {
    pub entries_created: usize,
    pub entries_updated: usize,
    pub entries_deleted: usize,
    pub errors: Vec<String>,
}

/// 远端目录树节点
#[derive(Debug, Clone)]
pub struct RemoteTreeNode {
    pub entry: RemoteEntry,
    pub children: Vec<RemoteTreeNode>,
    pub local_path: String,
}

impl RemoteTreeNode {
    pub fn new(entry: RemoteEntry, local_path: String) -> Self {
        Self {
            entry,
            children: Vec::new(),
            local_path,
        }
    }
}

/// 构建从 `entry_id` 到本地路径的映射
pub fn build_path_map(nodes: &[RemoteTreeNode], map: &mut HashMap<String, String>) {
    for node in nodes {
        map.insert(node.entry.id.clone(), node.local_path.clone());
        build_path_map(&node.children, map);
    }
}

/// 扁平化远端树为 (`entry_id`, `local_path`, `entry_type`, `has_children`) 列表
pub fn flatten_tree(nodes: &[RemoteTreeNode]) -> Vec<(String, String, EntryType, bool)> {
    let mut result = Vec::new();
    for node in nodes {
        let entry_type = parse_entry_type(&node.entry.entry_type);
        result.push((
            node.entry.id.clone(),
            node.local_path.clone(),
            entry_type,
            node.entry.has_children,
        ));
        result.extend(flatten_tree(&node.children));
    }
    result
}
