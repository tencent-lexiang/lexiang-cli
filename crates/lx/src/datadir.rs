//! 统一数据目录管理
//!
//! 所有 lexiang 数据统一存储在 `~/.lexiang/` 目录下，包括：
//! - auth/      OAuth 令牌
//! - tools/     MCP schema 缓存
//! - skills/    AI Agent skill 文件
//! - worktrees/ worktree 注册表

use std::fs;
use std::path::PathBuf;

/// 数据目录名
const DATA_DIR_NAME: &str = ".lexiang";

/// 数据目录管理器
#[derive(Debug, Clone)]
pub struct DataDir {
    path: PathBuf,
}

impl DataDir {
    /// 创建数据目录管理器
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// 获取数据目录路径
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// 确保目录存在
    #[allow(dead_code)]
    pub fn ensure(&self) -> std::io::Result<()> {
        fs::create_dir_all(&self.path)
    }
}

impl Default for DataDir {
    fn default() -> Self {
        Self::new(datadir())
    }
}

/// 获取统一数据目录路径 `~/.lexiang/`
pub fn datadir() -> PathBuf {
    let home = dirs::home_dir().expect("Cannot determine home directory");
    home.join(DATA_DIR_NAME)
}

/// 获取 auth 目录
pub fn auth_dir() -> PathBuf {
    let dir = datadir().join("auth");
    fs::create_dir_all(&dir).ok();
    dir
}

/// 获取 tools 目录
#[allow(dead_code)]
pub fn tools_dir() -> PathBuf {
    let dir = datadir().join("tools");
    fs::create_dir_all(&dir).ok();
    dir
}

/// 获取 skills 目录
pub fn skills_dir() -> PathBuf {
    let dir = datadir().join("skills");
    fs::create_dir_all(&dir).ok();
    dir
}

/// 获取 worktrees 注册表路径
pub fn worktrees_registry_path() -> PathBuf {
    datadir().join("worktrees.json")
}

/// 获取 worktrees 目录
#[allow(dead_code)]
pub fn worktrees_dir() -> PathBuf {
    let dir = datadir().join("worktrees");
    fs::create_dir_all(&dir).ok();
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_datadir_returns_lexiang() {
        let dir = datadir();
        assert!(dir.ends_with(DATA_DIR_NAME));
    }
}
