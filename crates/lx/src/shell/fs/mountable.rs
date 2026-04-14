//! `MountableFs`: 挂载点路由器
//!
//! 将不同的 `IFileSystem` 实现挂载到不同路径前缀，
//! 类似 Linux 的 mount 机制。
//!
//! 示例:
//! ```text
//!   /        → InMemoryFs (base)
//!   /kb      → LexiangFs (知识库)
//!   /workspace → OverlayFs (本地 worktree)
//! ```

use super::types::*;
use super::IFileSystem;
use anyhow::{bail, Result};
use async_trait::async_trait;
use std::any::Any;

/// 挂载点
struct MountPoint {
    /// 挂载路径 (如 "/kb")
    path: String,
    /// 对应的文件系统实现
    fs: Box<dyn IFileSystem>,
}

/// 挂载点路由器
pub struct MountableFs {
    /// 基础文件系统 (处理所有未被挂载点匹配的路径)
    base: Box<dyn IFileSystem>,
    /// 挂载点列表 (按路径长度倒序排列，确保最长前缀优先匹配)
    mounts: Vec<MountPoint>,
}

impl MountableFs {
    /// 创建新的挂载点路由器
    pub fn new(base: Box<dyn IFileSystem>) -> Self {
        Self {
            base,
            mounts: Vec::new(),
        }
    }

    /// 添加挂载点
    pub fn mount(mut self, path: &str, fs: Box<dyn IFileSystem>) -> Self {
        let path = super::normalize_path(path);
        self.mounts.push(MountPoint { path, fs });
        // 按路径长度倒序排列，确保最长前缀优先匹配
        self.mounts.sort_by(|a, b| b.path.len().cmp(&a.path.len()));
        self
    }

    /// 路由: 根据路径前缀找到对应的 fs 和子路径
    fn route(&self, path: &str) -> (&dyn IFileSystem, String) {
        let normalized = super::normalize_path(path);

        for mount in &self.mounts {
            if normalized == mount.path {
                // 精确匹配挂载点 → 子路径为 "/"
                return (mount.fs.as_ref(), "/".to_string());
            }
            if let Some(rest) = normalized.strip_prefix(&mount.path) {
                if rest.starts_with('/') {
                    // 前缀匹配 + 路径分隔符
                    return (mount.fs.as_ref(), rest.to_string());
                }
            }
        }

        // 未匹配任何挂载点，回退到 base
        (self.base.as_ref(), normalized)
    }

    /// 列出根目录时，需要合并 base 的内容和所有挂载点
    async fn read_root_dir(&self) -> Result<Vec<DirEntry>> {
        let mut entries = self.base.read_dir("/").await.unwrap_or_default();

        // 添加所有顶层挂载点作为目录
        for mount in &self.mounts {
            // 只添加一级挂载点 (如 /kb, 不添加 /kb/sub)
            let mount_name = mount.path.trim_start_matches('/');
            if !mount_name.is_empty() && !mount_name.contains('/') {
                // 检查是否已经存在
                if !entries.iter().any(|e| e.name == mount_name) {
                    entries.push(DirEntry::directory(mount_name));
                }
            }
        }

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
}

#[async_trait]
impl IFileSystem for MountableFs {
    async fn read_file(&self, path: &str) -> Result<String> {
        let (fs, sub_path) = self.route(path);
        fs.read_file(&sub_path).await
    }

    async fn write_file(&self, path: &str, content: &str) -> Result<()> {
        let (fs, sub_path) = self.route(path);
        fs.write_file(&sub_path, content).await
    }

    async fn append_file(&self, path: &str, content: &str) -> Result<()> {
        let (fs, sub_path) = self.route(path);
        fs.append_file(&sub_path, content).await
    }

    async fn read_dir(&self, path: &str) -> Result<Vec<DirEntry>> {
        let normalized = super::normalize_path(path);

        // 根目录需要特殊处理：合并 base 和挂载点
        if normalized == "/" {
            return self.read_root_dir().await;
        }

        // 检查路径是否是某个挂载点的父目录
        // 例如路径是 "/" 且有挂载点 "/kb/sub"，需要显示 "kb" 目录
        let prefix = format!("{normalized}/");
        let mut has_virtual_children = false;
        let mut virtual_dirs: Vec<String> = Vec::new();

        for mount in &self.mounts {
            if let Some(rest) = mount.path.strip_prefix(&prefix) {
                if let Some(first_part) = rest.split('/').next() {
                    if !first_part.is_empty() {
                        has_virtual_children = true;
                        if !virtual_dirs.contains(&first_part.to_string()) {
                            virtual_dirs.push(first_part.to_string());
                        }
                    }
                }
            }
        }

        let (fs, sub_path) = self.route(path);
        let mut entries = fs.read_dir(&sub_path).await?;

        if has_virtual_children {
            for dir_name in virtual_dirs {
                if !entries.iter().any(|e| e.name == dir_name) {
                    entries.push(DirEntry::directory(dir_name));
                }
            }
            entries.sort_by(|a, b| a.name.cmp(&b.name));
        }

        Ok(entries)
    }

    async fn stat(&self, path: &str) -> Result<FileStat> {
        let normalized = super::normalize_path(path);

        // 检查是否精确匹配某个挂载点路径的中间部分
        // 例如有挂载点 "/kb"，访问 "/" 应该能看到 kb 目录
        for mount in &self.mounts {
            if mount.path == normalized {
                return mount.fs.stat("/").await;
            }
        }

        let (fs, sub_path) = self.route(path);
        fs.stat(&sub_path).await
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        let normalized = super::normalize_path(path);

        // 挂载点本身总是存在的
        for mount in &self.mounts {
            if mount.path == normalized {
                return Ok(true);
            }
        }

        let (fs, sub_path) = self.route(path);
        fs.exists(&sub_path).await
    }

    async fn mkdir(&self, path: &str, recursive: bool) -> Result<()> {
        let (fs, sub_path) = self.route(path);
        fs.mkdir(&sub_path, recursive).await
    }

    async fn remove(&self, path: &str, recursive: bool) -> Result<()> {
        let normalized = super::normalize_path(path);

        // 不允许删除挂载点
        for mount in &self.mounts {
            if mount.path == normalized {
                bail!("Cannot remove mount point: {}", mount.path);
            }
        }

        let (fs, sub_path) = self.route(path);
        fs.remove(&sub_path, recursive).await
    }

    fn is_read_only(&self) -> bool {
        // 如果所有挂载的 fs 都是只读的，则整体只读
        self.base.is_read_only() && self.mounts.iter().all(|m| m.fs.is_read_only())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::super::in_memory::InMemoryFs;
    use super::*;

    #[tokio::test]
    async fn test_mount_routing() {
        let base = InMemoryFs::new().with_file("/tmp/test.txt", "base file");

        let kb = InMemoryFs::new()
            .with_dir("/docs")
            .with_file("/docs/guide.md", "# Guide")
            .with_file("/readme.md", "KB readme");

        let fs = MountableFs::new(Box::new(base)).mount("/kb", Box::new(kb));

        // 读取挂载点内的文件
        let content = fs.read_file("/kb/readme.md").await.unwrap();
        assert_eq!(content, "KB readme");

        let content = fs.read_file("/kb/docs/guide.md").await.unwrap();
        assert_eq!(content, "# Guide");

        // 读取 base 的文件
        let content = fs.read_file("/tmp/test.txt").await.unwrap();
        assert_eq!(content, "base file");
    }

    #[tokio::test]
    async fn test_root_dir_merge() {
        let base = InMemoryFs::new().with_dir("/tmp");
        let kb = InMemoryFs::new().with_file("/readme.md", "hello");

        let fs = MountableFs::new(Box::new(base)).mount("/kb", Box::new(kb));

        let entries = fs.read_dir("/").await.unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"tmp"));
        assert!(names.contains(&"kb"));
    }

    #[tokio::test]
    async fn test_mount_point_exists() {
        let base = InMemoryFs::new();
        let kb = InMemoryFs::new();

        let fs = MountableFs::new(Box::new(base)).mount("/kb", Box::new(kb));

        assert!(fs.exists("/kb").await.unwrap());
        assert!(!fs.exists("/nope").await.unwrap());
    }
}
