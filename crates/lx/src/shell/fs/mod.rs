//! Virtual filesystem abstraction layer.
//!
//! Provides the `IFileSystem` trait (inspired by just-bash's `IFileSystem` interface)
//! and built-in implementations:
//! - `InMemoryFs`: pure in-memory filesystem for testing and /tmp
//! - `MountableFs`: path-prefix router that composes multiple filesystems
//! - `OverlayFs`: copy-on-write overlay (read from base, write to memory)

pub mod in_memory;
pub mod lexiang;
pub mod mountable;
pub mod overlay;
pub mod types;
pub mod worktree;

pub use in_memory::InMemoryFs;
pub use lexiang::LexiangFs;
pub use mountable::MountableFs;
pub use overlay::OverlayFs;
pub use types::*;
pub use worktree::WorktreeFs;

use anyhow::Result;
use async_trait::async_trait;
use std::any::Any;

/// 虚拟文件系统接口 (对标 just-bash `IFileSystem`)
///
/// 所有 shell 命令通过此 trait 操作文件，不直接访问底层存储。
/// 不同的实现可以将操作路由到内存、本地磁盘、或远端 MCP API。
#[async_trait]
pub trait IFileSystem: Send + Sync {
    /// 读取文件完整内容
    async fn read_file(&self, path: &str) -> Result<String>;

    /// 写入文件内容 (只读文件系统返回 EROFS 错误)
    async fn write_file(&self, path: &str, content: &str) -> Result<()>;

    /// 追加内容到文件 (只读文件系统返回 EROFS 错误)
    async fn append_file(&self, path: &str, content: &str) -> Result<()>;

    /// 读取目录内容，返回直接子项列表
    async fn read_dir(&self, path: &str) -> Result<Vec<DirEntry>>;

    /// 获取文件/目录的状态信息
    async fn stat(&self, path: &str) -> Result<FileStat>;

    /// 检查路径是否存在
    async fn exists(&self, path: &str) -> Result<bool>;

    /// 创建目录 (recursive=true 时等价于 mkdir -p)
    async fn mkdir(&self, path: &str, recursive: bool) -> Result<()>;

    /// 删除文件或目录 (recursive=true 时递归删除)
    async fn remove(&self, path: &str, recursive: bool) -> Result<()>;

    /// 文件系统是否只读
    fn is_read_only(&self) -> bool;

    /// 转为 Any 引用，用于向下转型 (downcast)
    /// 某些命令 (如 grep) 需要检查底层是否是 `LexiangFs` 以利用搜索 API
    fn as_any(&self) -> &dyn Any;
}

/// 路径规范化工具
pub fn normalize_path(path: &str) -> String {
    if path.is_empty() {
        return "/".to_string();
    }

    let mut parts: Vec<&str> = Vec::new();
    let is_absolute = path.starts_with('/');

    for part in path.split('/') {
        match part {
            "" | "." => continue,
            ".." => {
                if !parts.is_empty() {
                    parts.pop();
                }
            }
            _ => parts.push(part),
        }
    }

    let joined = parts.join("/");
    if is_absolute {
        format!("/{joined}")
    } else if joined.is_empty() {
        ".".to_string()
    } else {
        joined
    }
}

/// 连接路径
pub fn join_path(base: &str, child: &str) -> String {
    if child.starts_with('/') {
        return normalize_path(child);
    }
    let combined = if base.ends_with('/') {
        format!("{base}{child}")
    } else {
        format!("{base}/{child}")
    };
    normalize_path(&combined)
}

/// 获取父目录路径
pub fn parent_path(path: &str) -> String {
    let normalized = normalize_path(path);
    if normalized == "/" {
        return "/".to_string();
    }
    match normalized.rfind('/') {
        Some(0) => "/".to_string(),
        Some(idx) => normalized[..idx].to_string(),
        None => ".".to_string(),
    }
}

/// 获取文件/目录名
pub fn basename(path: &str) -> String {
    let normalized = normalize_path(path);
    if normalized == "/" {
        return "/".to_string();
    }
    normalized
        .rsplit('/')
        .next()
        .unwrap_or(&normalized)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("/"), "/");
        assert_eq!(normalize_path(""), "/");
        assert_eq!(normalize_path("/foo/bar"), "/foo/bar");
        assert_eq!(normalize_path("/foo//bar"), "/foo/bar");
        assert_eq!(normalize_path("/foo/./bar"), "/foo/bar");
        assert_eq!(normalize_path("/foo/../bar"), "/bar");
        assert_eq!(normalize_path("/foo/bar/.."), "/foo");
        assert_eq!(normalize_path("/foo/bar/../baz"), "/foo/baz");
        assert_eq!(normalize_path("foo/bar"), "foo/bar");
        assert_eq!(normalize_path("./foo"), "foo");
    }

    #[test]
    fn test_join_path() {
        assert_eq!(join_path("/foo", "bar"), "/foo/bar");
        assert_eq!(join_path("/foo/", "bar"), "/foo/bar");
        assert_eq!(join_path("/foo", "/bar"), "/bar");
        assert_eq!(join_path("/foo", "bar/baz"), "/foo/bar/baz");
        assert_eq!(join_path("/foo", "../bar"), "/bar");
    }

    #[test]
    fn test_parent_path() {
        assert_eq!(parent_path("/"), "/");
        assert_eq!(parent_path("/foo"), "/");
        assert_eq!(parent_path("/foo/bar"), "/foo");
        assert_eq!(parent_path("/foo/bar/baz"), "/foo/bar");
    }

    #[test]
    fn test_basename() {
        assert_eq!(basename("/"), "/");
        assert_eq!(basename("/foo"), "foo");
        assert_eq!(basename("/foo/bar"), "bar");
        assert_eq!(basename("/foo/bar.md"), "bar.md");
    }
}
