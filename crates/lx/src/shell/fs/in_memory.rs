//! `InMemoryFs`: 纯内存虚拟文件系统实现
//!
//! 所有文件和目录都存储在内存中的 `HashMap` 里。
//! 主要用于：
//! - 测试 shell 命令
//! - /tmp 等临时目录
//! - 构建测试用的文件系统状态

use super::types::*;
use super::{normalize_path, parent_path, IFileSystem};
use anyhow::{bail, Result};
use async_trait::async_trait;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::SystemTime;

/// 内存中的文件/目录节点
#[derive(Debug, Clone)]
enum FsNode {
    File {
        content: String,
        modified: SystemTime,
    },
    Directory {
        modified: SystemTime,
    },
}

/// 纯内存文件系统
pub struct InMemoryFs {
    nodes: Mutex<HashMap<String, FsNode>>,
    read_only: bool,
}

impl InMemoryFs {
    /// 创建空的内存文件系统 (只有根目录)
    pub fn new() -> Self {
        let mut nodes = HashMap::new();
        nodes.insert(
            "/".to_string(),
            FsNode::Directory {
                modified: SystemTime::now(),
            },
        );
        Self {
            nodes: Mutex::new(nodes),
            read_only: false,
        }
    }

    /// 创建只读的内存文件系统
    pub fn new_read_only() -> Self {
        let mut fs = Self::new();
        fs.read_only = true;
        fs
    }

    /// 预填充文件 (用于构建测试状态)
    pub fn with_file(self, path: &str, content: &str) -> Self {
        let path = normalize_path(path);
        let mut nodes = self.nodes.lock().unwrap();
        ensure_parents_inner(&mut nodes, &path);
        nodes.insert(
            path,
            FsNode::File {
                content: content.to_string(),
                modified: SystemTime::now(),
            },
        );
        drop(nodes);
        self
    }

    /// 预填充目录
    pub fn with_dir(self, path: &str) -> Self {
        let path = normalize_path(path);
        let mut nodes = self.nodes.lock().unwrap();
        ensure_parents_inner(&mut nodes, &path);
        nodes.insert(
            path,
            FsNode::Directory {
                modified: SystemTime::now(),
            },
        );
        drop(nodes);
        self
    }

    /// 检查写权限
    fn check_writable(&self) -> Result<()> {
        if self.read_only {
            bail!("EROFS: read-only file system");
        }
        Ok(())
    }
}

/// 确保路径的所有父目录存在 (内部辅助函数)
fn ensure_parents_inner(nodes: &mut HashMap<String, FsNode>, path: &str) {
    let mut current = parent_path(path);
    while current != "/" && !nodes.contains_key(&current) {
        nodes.insert(
            current.clone(),
            FsNode::Directory {
                modified: SystemTime::now(),
            },
        );
        current = parent_path(&current);
    }
}

impl Default for InMemoryFs {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IFileSystem for InMemoryFs {
    async fn read_file(&self, path: &str) -> Result<String> {
        let path = normalize_path(path);
        let nodes = self.nodes.lock().unwrap();
        match nodes.get(&path) {
            Some(FsNode::File { content, .. }) => Ok(content.clone()),
            Some(FsNode::Directory { .. }) => bail!("Is a directory: {path}"),
            None => bail!("No such file or directory: {path}"),
        }
    }

    async fn write_file(&self, path: &str, content: &str) -> Result<()> {
        self.check_writable()?;
        let path = normalize_path(path);
        let mut nodes = self.nodes.lock().unwrap();
        ensure_parents_inner(&mut nodes, &path);
        nodes.insert(
            path,
            FsNode::File {
                content: content.to_string(),
                modified: SystemTime::now(),
            },
        );
        Ok(())
    }

    async fn append_file(&self, path: &str, content: &str) -> Result<()> {
        self.check_writable()?;
        let path = normalize_path(path);
        let mut nodes = self.nodes.lock().unwrap();

        match nodes.get_mut(&path) {
            Some(FsNode::File {
                content: existing,
                modified,
            }) => {
                existing.push_str(content);
                *modified = SystemTime::now();
                Ok(())
            }
            Some(FsNode::Directory { .. }) => bail!("Is a directory: {path}"),
            None => {
                ensure_parents_inner(&mut nodes, &path);
                nodes.insert(
                    path,
                    FsNode::File {
                        content: content.to_string(),
                        modified: SystemTime::now(),
                    },
                );
                Ok(())
            }
        }
    }

    async fn read_dir(&self, path: &str) -> Result<Vec<DirEntry>> {
        let path = normalize_path(path);
        let nodes = self.nodes.lock().unwrap();

        match nodes.get(&path) {
            Some(FsNode::Directory { .. }) => {}
            Some(FsNode::File { .. }) => bail!("Not a directory: {path}"),
            None => bail!("No such file or directory: {path}"),
        }

        let prefix = if path == "/" {
            "/".to_string()
        } else {
            format!("{path}/")
        };

        let mut entries = Vec::new();
        for (node_path, node) in nodes.iter() {
            if let Some(rest) = node_path.strip_prefix(&prefix) {
                if !rest.is_empty() && !rest.contains('/') {
                    let entry = match node {
                        FsNode::File { content, modified } => DirEntry {
                            name: rest.to_string(),
                            file_type: FileType::File,
                            size: content.len() as u64,
                            modified: Some(*modified),
                            metadata: None,
                        },
                        FsNode::Directory { modified } => DirEntry {
                            name: rest.to_string(),
                            file_type: FileType::Directory,
                            size: 0,
                            modified: Some(*modified),
                            metadata: None,
                        },
                    };
                    entries.push(entry);
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

    async fn stat(&self, path: &str) -> Result<FileStat> {
        let path = normalize_path(path);
        let nodes = self.nodes.lock().unwrap();
        match nodes.get(&path) {
            Some(FsNode::File { content, modified }) => Ok(FileStat {
                file_type: FileType::File,
                size: content.len() as u64,
                created: None,
                modified: Some(*modified),
                accessed: None,
                readonly: self.read_only,
                metadata: None,
            }),
            Some(FsNode::Directory { modified }) => Ok(FileStat {
                file_type: FileType::Directory,
                size: 0,
                created: None,
                modified: Some(*modified),
                accessed: None,
                readonly: self.read_only,
                metadata: None,
            }),
            None => bail!("No such file or directory: {path}"),
        }
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        let path = normalize_path(path);
        let nodes = self.nodes.lock().unwrap();
        Ok(nodes.contains_key(&path))
    }

    async fn mkdir(&self, path: &str, recursive: bool) -> Result<()> {
        self.check_writable()?;
        let path = normalize_path(path);
        let mut nodes = self.nodes.lock().unwrap();

        if recursive {
            let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            let mut current = String::new();
            for part in parts {
                current = format!("{current}/{part}");
                if !nodes.contains_key(&current) {
                    nodes.insert(
                        current.clone(),
                        FsNode::Directory {
                            modified: SystemTime::now(),
                        },
                    );
                }
            }
        } else {
            let parent = parent_path(&path);
            if !nodes.contains_key(&parent) {
                bail!("No such file or directory: {parent}");
            }
            if nodes.contains_key(&path) {
                bail!("File exists: {path}");
            }
            nodes.insert(
                path,
                FsNode::Directory {
                    modified: SystemTime::now(),
                },
            );
        }

        Ok(())
    }

    async fn remove(&self, path: &str, recursive: bool) -> Result<()> {
        self.check_writable()?;
        let path = normalize_path(path);

        if path == "/" {
            bail!("Cannot remove root directory");
        }

        let mut nodes = self.nodes.lock().unwrap();

        let node_exists = nodes
            .get(&path)
            .map(|n| matches!(n, FsNode::Directory { .. }));

        match node_exists {
            None => bail!("No such file or directory: {path}"),
            Some(true) => {
                // Directory
                if !recursive {
                    let prefix = format!("{path}/");
                    let has_children = nodes.keys().any(|k| k.starts_with(&prefix));
                    if has_children {
                        bail!("Directory not empty: {path}");
                    }
                }
                let prefix = format!("{path}/");
                nodes.retain(|k, _| k != &path && !k.starts_with(&prefix));
            }
            Some(false) => {
                // File
                nodes.remove(&path);
            }
        }

        Ok(())
    }

    fn is_read_only(&self) -> bool {
        self.read_only
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_file_operations() {
        let fs = InMemoryFs::new()
            .with_dir("/docs")
            .with_file("/docs/readme.md", "# Hello\nWorld")
            .with_file("/docs/guide.md", "Guide content");

        let content = fs.read_file("/docs/readme.md").await.unwrap();
        assert_eq!(content, "# Hello\nWorld");

        assert!(fs.exists("/docs").await.unwrap());
        assert!(fs.exists("/docs/readme.md").await.unwrap());
        assert!(!fs.exists("/docs/nope.md").await.unwrap());

        let stat = fs.stat("/docs/readme.md").await.unwrap();
        assert!(stat.file_type.is_file());
        assert_eq!(stat.size, 13);

        let stat = fs.stat("/docs").await.unwrap();
        assert!(stat.file_type.is_dir());
    }

    #[tokio::test]
    async fn test_read_dir() {
        let fs = InMemoryFs::new()
            .with_dir("/docs")
            .with_dir("/docs/api")
            .with_file("/docs/readme.md", "hello")
            .with_file("/docs/api/auth.md", "auth");

        let entries = fs.read_dir("/docs").await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "api");
        assert!(entries[0].file_type.is_dir());
        assert_eq!(entries[1].name, "readme.md");
        assert!(entries[1].file_type.is_file());
    }

    #[tokio::test]
    async fn test_write_and_append() {
        let fs = InMemoryFs::new();

        fs.write_file("/test.txt", "hello").await.unwrap();
        assert_eq!(fs.read_file("/test.txt").await.unwrap(), "hello");

        fs.append_file("/test.txt", " world").await.unwrap();
        assert_eq!(fs.read_file("/test.txt").await.unwrap(), "hello world");
    }

    #[tokio::test]
    async fn test_mkdir_and_remove() {
        let fs = InMemoryFs::new();

        fs.mkdir("/a/b/c", true).await.unwrap();
        assert!(fs.exists("/a").await.unwrap());
        assert!(fs.exists("/a/b").await.unwrap());
        assert!(fs.exists("/a/b/c").await.unwrap());

        fs.write_file("/a/b/c/file.txt", "x").await.unwrap();
        assert!(fs.remove("/a/b/c", false).await.is_err());

        fs.remove("/a", true).await.unwrap();
        assert!(!fs.exists("/a").await.unwrap());
    }

    #[tokio::test]
    async fn test_read_only() {
        let fs = InMemoryFs::new_read_only();
        assert!(fs.is_read_only());
        assert!(fs.write_file("/test.txt", "hello").await.is_err());
        assert!(fs.mkdir("/dir", false).await.is_err());
        assert!(fs.remove("/", false).await.is_err());
    }

    #[tokio::test]
    async fn test_error_cases() {
        let fs = InMemoryFs::new().with_file("/file.txt", "content");

        assert!(fs.read_file("/nope.txt").await.is_err());
        assert!(fs.read_dir("/file.txt").await.is_err());
        assert!(fs.read_file("/").await.is_err());
    }
}
