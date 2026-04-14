//! `WorktreeFs`: 基于本地 worktree 目录的文件系统
//!
//! 将 `IFileSystem` trait 映射到本地磁盘上的 worktree 目录:
//! - `read_file(path)` → `std::fs::read_to_string(worktree_path / path)`
//! - `read_dir(path)` → `std::fs::read_dir(worktree_path / path)`
//! - `stat(path)` → `std::fs::metadata(worktree_path / path)`
//!
//! 路径格式 (挂载在 /kb 后):
//! ```text
//! /kb           → worktree_path/
//! /kb/产品文档   → worktree_path/产品文档/
//! /kb/README.md → worktree_path/README.md
//! ```
//!
//! 只读模式：写操作返回 EROFS 错误（与 `LexiangFs` 行为一致）。

use super::types::*;
use super::IFileSystem;
use anyhow::{bail, Result};
use async_trait::async_trait;
use std::any::Any;
use std::path::{Path, PathBuf};

/// 基于本地 worktree 目录的文件系统
///
/// 将虚拟路径映射到磁盘上的 worktree 目录，过滤掉 `.lxworktree/` 和 `.git/` 等元数据目录。
pub struct WorktreeFs {
    /// Worktree 根目录的绝对路径
    root: PathBuf,
    /// 知识库名称 (用于显示)
    space_name: String,
    /// 知识库 ID
    space_id: String,
}

/// 需要从目录列表中隐藏的元数据目录
const HIDDEN_DIRS: &[&str] = &[".lxworktree", ".git"];

impl WorktreeFs {
    /// 创建新的 `WorktreeFs`
    ///
    /// `root` 必须是一个包含 `.lxworktree/` 目录的 worktree 路径。
    pub fn new(root: PathBuf, space_name: String, space_id: String) -> Self {
        Self {
            root,
            space_name,
            space_id,
        }
    }

    /// 获取 worktree 根目录
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 获取知识库名称
    pub fn space_name(&self) -> &str {
        &self.space_name
    }

    /// 获取知识库 ID
    pub fn space_id(&self) -> &str {
        &self.space_id
    }

    /// 将虚拟路径转换为磁盘绝对路径
    ///
    /// 虚拟路径 "/" → worktree root
    /// 虚拟路径 "/foo/bar" → `worktree_root`/foo/bar
    fn to_real_path(&self, virtual_path: &str) -> Result<PathBuf> {
        let normalized = super::normalize_path(virtual_path);

        // 去掉开头的 /
        let relative = normalized.strip_prefix('/').unwrap_or(&normalized);

        let real_path = if relative.is_empty() {
            self.root.clone()
        } else {
            self.root.join(relative)
        };

        // 安全检查: 防止路径逃逸
        let canonical_root = self
            .root
            .canonicalize()
            .unwrap_or_else(|_| self.root.clone());
        let canonical_path = real_path
            .canonicalize()
            .unwrap_or_else(|_| real_path.clone());

        if !canonical_path.starts_with(&canonical_root) {
            bail!("Path escapes worktree root: {}", virtual_path);
        }

        // 禁止访问隐藏的元数据目录
        for component in canonical_path
            .strip_prefix(&canonical_root)
            .unwrap_or(&canonical_path)
            .components()
        {
            if let std::path::Component::Normal(name) = component {
                let name_str = name.to_string_lossy();
                if HIDDEN_DIRS.contains(&name_str.as_ref()) {
                    bail!("Permission denied: {}", virtual_path);
                }
            }
        }

        Ok(real_path)
    }

    /// 判断文件名是否应该被隐藏
    fn is_hidden(name: &str) -> bool {
        HIDDEN_DIRS.contains(&name)
    }
}

#[async_trait]
impl IFileSystem for WorktreeFs {
    async fn read_file(&self, path: &str) -> Result<String> {
        let real_path = self.to_real_path(path)?;

        if !real_path.exists() {
            bail!("No such file or directory: {path}");
        }

        let meta = std::fs::metadata(&real_path)?;
        if meta.is_dir() {
            bail!("Is a directory: {path}");
        }

        let content = std::fs::read_to_string(&real_path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::InvalidData {
                anyhow::anyhow!("Binary file, cannot display as text: {path}")
            } else {
                anyhow::anyhow!("Failed to read {path}: {e}")
            }
        })?;

        Ok(content)
    }

    async fn write_file(&self, _path: &str, _content: &str) -> Result<()> {
        bail!("EROFS: read-only file system (use 'git push' to sync changes)")
    }

    async fn append_file(&self, _path: &str, _content: &str) -> Result<()> {
        bail!("EROFS: read-only file system")
    }

    async fn read_dir(&self, path: &str) -> Result<Vec<DirEntry>> {
        let real_path = self.to_real_path(path)?;

        if !real_path.exists() {
            bail!("No such file or directory: {path}");
        }

        let meta = std::fs::metadata(&real_path)?;
        if !meta.is_dir() {
            bail!("Not a directory: {path}");
        }

        let mut entries = Vec::new();

        for entry in std::fs::read_dir(&real_path)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();

            // 跳过隐藏的元数据目录
            if Self::is_hidden(&name) {
                continue;
            }

            let meta = entry.metadata()?;
            let file_type = if meta.is_dir() {
                FileType::Directory
            } else {
                FileType::File
            };

            let modified = meta.modified().ok();

            entries.push(DirEntry {
                name,
                file_type,
                size: meta.len(),
                modified,
                metadata: None,
            });
        }

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
        let real_path = self.to_real_path(path)?;

        if !real_path.exists() {
            bail!("No such file or directory: {path}");
        }

        let meta = std::fs::metadata(&real_path)?;
        let file_type = if meta.is_dir() {
            FileType::Directory
        } else {
            FileType::File
        };

        let modified = meta.modified().ok();
        let created = meta.created().ok();
        let accessed = meta.accessed().ok();

        Ok(FileStat {
            file_type,
            size: meta.len(),
            created,
            modified,
            accessed,
            readonly: true, // 通过 shell 操作时默认只读
            metadata: None,
        })
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        match self.to_real_path(path) {
            Ok(real_path) => Ok(real_path.exists()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_worktree() -> (tempfile::TempDir, WorktreeFs) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();

        // 创建 .lxworktree 目录 (应该被隐藏)
        fs::create_dir_all(root.join(".lxworktree")).unwrap();
        fs::write(
            root.join(".lxworktree/config.json"),
            r#"{"space_id":"s1","space_name":"test"}"#,
        )
        .unwrap();

        // 创建 .git 目录 (应该被隐藏)
        fs::create_dir_all(root.join(".git")).unwrap();

        // 创建测试内容
        fs::create_dir_all(root.join("产品文档")).unwrap();
        fs::write(root.join("README.md"), "# Hello\n\nWelcome!\n").unwrap();
        fs::write(
            root.join("产品文档/API指南.md"),
            "# API Guide\n\nOAuth 2.0\n",
        )
        .unwrap();
        fs::write(root.join("产品文档/部署.md"), "# Deploy\n\nDocker\n").unwrap();

        let wfs = WorktreeFs::new(root, "测试库".to_string(), "space_001".to_string());
        (dir, wfs)
    }

    #[tokio::test]
    async fn test_read_dir_root() {
        let (_dir, fs) = setup_test_worktree();
        let entries = fs.read_dir("/").await.unwrap();

        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"产品文档"));
        assert!(names.contains(&"README.md"));
        // 元数据目录不应出现
        assert!(!names.contains(&".lxworktree"));
        assert!(!names.contains(&".git"));
    }

    #[tokio::test]
    async fn test_read_dir_subfolder() {
        let (_dir, fs) = setup_test_worktree();
        let entries = fs.read_dir("/产品文档").await.unwrap();

        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"API指南.md"));
        assert!(names.contains(&"部署.md"));
    }

    #[tokio::test]
    async fn test_read_file() {
        let (_dir, fs) = setup_test_worktree();
        let content = fs.read_file("/README.md").await.unwrap();
        assert!(content.contains("# Hello"));
        assert!(content.contains("Welcome"));
    }

    #[tokio::test]
    async fn test_read_nested_file() {
        let (_dir, fs) = setup_test_worktree();
        let content = fs.read_file("/产品文档/API指南.md").await.unwrap();
        assert!(content.contains("API Guide"));
        assert!(content.contains("OAuth"));
    }

    #[tokio::test]
    async fn test_stat_dir() {
        let (_dir, fs) = setup_test_worktree();
        let stat = fs.stat("/产品文档").await.unwrap();
        assert!(stat.file_type.is_dir());
        assert!(stat.readonly);
    }

    #[tokio::test]
    async fn test_stat_file() {
        let (_dir, fs) = setup_test_worktree();
        let stat = fs.stat("/README.md").await.unwrap();
        assert!(stat.file_type.is_file());
        assert!(stat.size > 0);
    }

    #[tokio::test]
    async fn test_exists() {
        let (_dir, fs) = setup_test_worktree();
        assert!(fs.exists("/").await.unwrap());
        assert!(fs.exists("/README.md").await.unwrap());
        assert!(fs.exists("/产品文档").await.unwrap());
        assert!(!fs.exists("/不存在.md").await.unwrap());
    }

    #[tokio::test]
    async fn test_read_only() {
        let (_dir, fs) = setup_test_worktree();
        assert!(fs.is_read_only());
        assert!(fs.write_file("/test.md", "hello").await.is_err());
        assert!(fs.mkdir("/newdir", false).await.is_err());
        assert!(fs.remove("/README.md", false).await.is_err());
    }

    #[tokio::test]
    async fn test_hidden_dirs_denied() {
        let (_dir, fs) = setup_test_worktree();
        // 不应该能访问 .lxworktree 内容
        assert!(fs.read_file("/.lxworktree/config.json").await.is_err());
        assert!(fs.read_dir("/.lxworktree").await.is_err());
    }

    #[tokio::test]
    async fn test_is_directory_error() {
        let (_dir, fs) = setup_test_worktree();
        let result = fs.read_file("/产品文档").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Is a directory"));
    }

    #[tokio::test]
    async fn test_not_a_directory_error() {
        let (_dir, fs) = setup_test_worktree();
        let result = fs.read_dir("/README.md").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Not a directory"));
    }

    // ── 集成: WorktreeFs + Bash ──

    #[tokio::test]
    async fn test_with_bash() {
        use crate::shell::bash::Bash;

        let (_dir, fs) = setup_test_worktree();
        let mut bash = Bash::new(Box::new(fs)).with_cwd("/");

        // ls
        let output = bash.exec("ls /").await.unwrap();
        assert!(output.stdout.contains("产品文档"));
        assert!(output.stdout.contains("README.md"));
        assert!(!output.stdout.contains(".lxworktree"));

        // cat
        let output = bash.exec("cat /README.md").await.unwrap();
        assert!(output.stdout.contains("# Hello"));

        // grep
        let output = bash.exec("grep -r OAuth /产品文档").await.unwrap();
        assert!(output.stdout.contains("OAuth"));

        // find
        let output = bash.exec("find / -name '*.md' -type f").await.unwrap();
        assert!(output.stdout.contains("README.md"));
        assert!(output.stdout.contains("API指南.md"));

        // pipe
        let output = bash.exec("ls /产品文档 | grep API").await.unwrap();
        assert!(output.stdout.contains("API"));
    }
}
