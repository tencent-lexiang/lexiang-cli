//! Worktree 配置管理
//!
//! 管理 `.lxworktree/config.json` 文件，存储 worktree 元数据。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Worktree 配置目录名
pub const CONFIG_DIR: &str = ".lxworktree";

/// Worktree 配置文件名
pub const CONFIG_FILE: &str = "config.json";

/// Worktree 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeConfig {
    /// 关联的知识库 ID
    pub space_id: String,
    /// 知识库名称
    pub space_name: String,
    /// 创建时间 (ISO 8601)
    pub created_at: String,
    /// 最后同步时间 (ISO 8601)
    pub last_sync_at: Option<String>,
    /// 远端快照 commit hash
    pub remote_snapshot_commit: Option<String>,
}

impl WorktreeConfig {
    /// 创建新配置
    pub fn new(space_id: String, space_name: String) -> Self {
        Self {
            space_id,
            space_name,
            created_at: chrono::Utc::now().to_rfc3339(),
            last_sync_at: None,
            remote_snapshot_commit: None,
        }
    }

    /// 获取 worktree 配置目录路径
    pub fn config_dir(worktree_path: &Path) -> PathBuf {
        worktree_path.join(CONFIG_DIR)
    }

    /// 获取配置文件路径
    pub fn config_path(worktree_path: &Path) -> PathBuf {
        Self::config_dir(worktree_path).join(CONFIG_FILE)
    }

    /// 从文件加载配置
    pub fn load(worktree_path: &Path) -> Result<Self> {
        let path = Self::config_path(worktree_path);
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config: {}", path.display()))?;
        let config: Self = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse config: {}", path.display()))?;
        Ok(config)
    }

    /// 保存配置到文件
    pub fn save(&self, worktree_path: &Path) -> Result<()> {
        let config_dir = Self::config_dir(worktree_path);
        fs::create_dir_all(&config_dir)
            .with_context(|| format!("Failed to create config dir: {}", config_dir.display()))?;

        let path = Self::config_path(worktree_path);
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write config: {}", path.display()))?;
        Ok(())
    }

    /// 更新最后同步时间
    #[allow(dead_code)]
    pub fn update_sync_time(&mut self) {
        self.last_sync_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// 设置远端快照 commit
    pub fn set_remote_snapshot(&mut self, commit: String) {
        self.remote_snapshot_commit = Some(commit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_new() {
        let config = WorktreeConfig::new("space123".to_string(), "测试知识库".to_string());
        assert_eq!(config.space_id, "space123");
        assert_eq!(config.space_name, "测试知识库");
        assert!(config.last_sync_at.is_none());
    }
}
