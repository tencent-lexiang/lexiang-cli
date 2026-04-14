//! Worktree 注册表管理
//!
//! 管理所有 worktree 的注册信息，存储在 `~/.lexiang/worktrees.json`。

use crate::datadir;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;

/// Worktree 注册记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeRecord {
    /// Worktree 绝对路径
    pub path: String,
    /// 关联的知识库 ID
    pub space_id: String,
    /// 知识库名称
    pub space_name: String,
    /// 创建时间 (ISO 8601)
    pub created_at: String,
}

/// Worktree 注册表
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorktreeRegistry {
    /// 已注册的 worktree 列表
    pub worktrees: Vec<WorktreeRecord>,
}

impl WorktreeRegistry {
    /// 加载注册表
    pub fn load() -> Result<Self> {
        let path = datadir::worktrees_registry_path();
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read registry: {}", path.display()))?;
        let registry: Self = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse registry: {}", path.display()))?;
        Ok(registry)
    }

    /// 保存注册表
    pub fn save(&self) -> Result<()> {
        let path = datadir::worktrees_registry_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write registry: {}", path.display()))?;
        Ok(())
    }

    /// 注册 worktree
    pub fn register(&mut self, record: WorktreeRecord) -> Result<()> {
        // 检查是否已存在相同路径
        if self.worktrees.iter().any(|w| w.path == record.path) {
            anyhow::bail!("Worktree already registered: {}", record.path);
        }

        self.worktrees.push(record);
        self.save()
    }

    /// 注销 worktree
    pub fn unregister(&mut self, path: &str) -> Result<bool> {
        let initial_len = self.worktrees.len();
        self.worktrees.retain(|w| w.path != path);

        if self.worktrees.len() < initial_len {
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 根据路径查找 worktree
    pub fn find_by_path(&self, path: &str) -> Option<&WorktreeRecord> {
        self.worktrees.iter().find(|w| w.path == path)
    }

    /// 根据知识库 ID 查找 worktree
    #[allow(dead_code)]
    pub fn find_by_space_id(&self, space_id: &str) -> Vec<&WorktreeRecord> {
        self.worktrees
            .iter()
            .filter(|w| w.space_id == space_id)
            .collect()
    }

    /// 列出所有 worktree
    pub fn list(&self) -> &[WorktreeRecord] {
        &self.worktrees
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_operations() {
        let mut registry = WorktreeRegistry::default();

        let record = WorktreeRecord {
            path: "/tmp/test-worktree".to_string(),
            space_id: "space123".to_string(),
            space_name: "测试库".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        registry.worktrees.push(record.clone());
        assert_eq!(registry.list().len(), 1);

        let found = registry.find_by_path("/tmp/test-worktree");
        assert!(found.is_some());

        registry.worktrees.clear();
        assert!(!registry.unregister("/tmp/test-worktree").unwrap());
    }
}
