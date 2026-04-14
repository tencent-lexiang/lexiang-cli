//! 文件路径与 `entry_id` 映射管理
//!
//! 管理 `.lxworktree/entries.json` 文件，维护本地文件路径与远端 entry 的映射关系。

#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Entries 映射文件名
pub const ENTRIES_FILE: &str = "entries.json";

/// Entry 类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EntryType {
    Folder,
    Page,
    File,
    Smartsheet,
}

impl EntryType {
    /// 判断是否为目录类型
    pub fn is_folder(&self) -> bool {
        matches!(self, EntryType::Folder)
    }
}

/// Entry 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryInfo {
    /// Entry ID（远端唯一标识）
    pub entry_id: String,
    /// Entry 类型
    pub entry_type: EntryType,
    /// 远端最后更新时间 (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_updated_at: Option<String>,
}

/// 文件路径 → Entry 信息映射
pub type EntriesMap = HashMap<String, EntryInfo>;

/// Entries 管理器
pub struct EntriesManager;

impl EntriesManager {
    /// 获取 entries 文件路径
    pub fn entries_path(worktree_path: &Path) -> PathBuf {
        worktree_path
            .join(super::config::CONFIG_DIR)
            .join(ENTRIES_FILE)
    }

    /// 加载 entries 映射
    pub fn load(worktree_path: &Path) -> Result<EntriesMap> {
        let path = Self::entries_path(worktree_path);
        if !path.exists() {
            return Ok(HashMap::new());
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read entries: {}", path.display()))?;
        let entries: EntriesMap = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse entries: {}", path.display()))?;
        Ok(entries)
    }

    /// 保存 entries 映射
    pub fn save(worktree_path: &Path, entries: &EntriesMap) -> Result<()> {
        let config_dir = worktree_path.join(super::config::CONFIG_DIR);
        fs::create_dir_all(&config_dir)
            .with_context(|| format!("Failed to create config dir: {}", config_dir.display()))?;

        let path = Self::entries_path(worktree_path);
        let content = serde_json::to_string_pretty(entries)?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write entries: {}", path.display()))?;
        Ok(())
    }

    /// 添加 entry
    pub fn add(
        entries: &mut EntriesMap,
        relative_path: String,
        entry_id: String,
        entry_type: EntryType,
        remote_updated_at: Option<String>,
    ) {
        entries.insert(
            relative_path,
            EntryInfo {
                entry_id,
                entry_type,
                remote_updated_at,
            },
        );
    }

    /// 删除 entry
    pub fn remove(entries: &mut EntriesMap, relative_path: &str) -> Option<EntryInfo> {
        entries.remove(relative_path)
    }

    /// 更新 entry ID（用于新建文件 push 后获取 `entry_id`）
    pub fn update_entry_id(
        entries: &mut EntriesMap,
        relative_path: &str,
        new_entry_id: String,
    ) -> bool {
        if let Some(info) = entries.get_mut(relative_path) {
            info.entry_id = new_entry_id;
            true
        } else {
            false
        }
    }

    /// 根据文件路径查找 `entry_id`
    pub fn get_entry_id<'a>(entries: &'a EntriesMap, relative_path: &str) -> Option<&'a String> {
        entries.get(relative_path).map(|info| &info.entry_id)
    }

    /// 根据 `entry_id` 查找文件路径
    pub fn get_path_by_entry_id<'a>(entries: &'a EntriesMap, entry_id: &str) -> Option<&'a String> {
        entries
            .iter()
            .find(|(_, info)| info.entry_id == entry_id)
            .map(|(path, _)| path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entries_manager() {
        let mut entries = HashMap::new();

        EntriesManager::add(
            &mut entries,
            "产品文档/API指南.md".to_string(),
            "entry123".to_string(),
            EntryType::Page,
            Some("2026-01-01T00:00:00Z".to_string()),
        );

        assert_eq!(
            EntriesManager::get_entry_id(&entries, "产品文档/API指南.md"),
            Some(&"entry123".to_string())
        );

        assert!(EntriesManager::update_entry_id(
            &mut entries,
            "产品文档/API指南.md",
            "entry456".to_string()
        ));

        assert_eq!(
            EntriesManager::get_entry_id(&entries, "产品文档/API指南.md"),
            Some(&"entry456".to_string())
        );
    }
}
