//! `OverlayFs`: 写时复制文件系统
//!
//! 读操作穿透到底层 (lower) 文件系统，
//! 写操作保存到上层 (upper) 内存文件系统。
//! 适用于基于本地 worktree 的只读视图。

use super::in_memory::InMemoryFs;
use super::types::*;
use super::IFileSystem;
use anyhow::Result;
use async_trait::async_trait;
use std::any::Any;

/// 写时复制文件系统
pub struct OverlayFs {
    /// 底层文件系统 (通常是只读的)
    lower: Box<dyn IFileSystem>,
    /// 上层文件系统 (内存，用于存储写入)
    upper: InMemoryFs,
    /// 是否允许写入
    writable: bool,
}

impl OverlayFs {
    /// 创建只读的 `OverlayFs` (最常用场景)
    pub fn read_only(lower: Box<dyn IFileSystem>) -> Self {
        Self {
            lower,
            upper: InMemoryFs::new(),
            writable: false,
        }
    }

    /// 创建可写的 `OverlayFs`
    pub fn writable(lower: Box<dyn IFileSystem>) -> Self {
        Self {
            lower,
            upper: InMemoryFs::new(),
            writable: true,
        }
    }
}

#[async_trait]
impl IFileSystem for OverlayFs {
    async fn read_file(&self, path: &str) -> Result<String> {
        // 先查 upper，再查 lower
        if self.upper.exists(path).await.unwrap_or(false) {
            return self.upper.read_file(path).await;
        }
        self.lower.read_file(path).await
    }

    async fn write_file(&self, path: &str, content: &str) -> Result<()> {
        if !self.writable {
            anyhow::bail!("EROFS: read-only file system");
        }
        self.upper.write_file(path, content).await
    }

    async fn append_file(&self, path: &str, content: &str) -> Result<()> {
        if !self.writable {
            anyhow::bail!("EROFS: read-only file system");
        }
        // 如果 upper 没有，先从 lower 读取再写到 upper
        if !self.upper.exists(path).await.unwrap_or(false) {
            if let Ok(existing) = self.lower.read_file(path).await {
                self.upper.write_file(path, &existing).await?;
            }
        }
        self.upper.append_file(path, content).await
    }

    async fn read_dir(&self, path: &str) -> Result<Vec<DirEntry>> {
        let lower_entries = self.lower.read_dir(path).await.unwrap_or_default();
        let upper_entries = self.upper.read_dir(path).await.unwrap_or_default();

        // 合并: upper 覆盖 lower
        let mut merged = std::collections::HashMap::new();
        for entry in lower_entries {
            merged.insert(entry.name.clone(), entry);
        }
        for entry in upper_entries {
            merged.insert(entry.name.clone(), entry);
        }

        let mut entries: Vec<DirEntry> = merged.into_values().collect();
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
        if self.upper.exists(path).await.unwrap_or(false) {
            return self.upper.stat(path).await;
        }
        self.lower.stat(path).await
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        if self.upper.exists(path).await.unwrap_or(false) {
            return Ok(true);
        }
        self.lower.exists(path).await
    }

    async fn mkdir(&self, path: &str, recursive: bool) -> Result<()> {
        if !self.writable {
            anyhow::bail!("EROFS: read-only file system");
        }
        self.upper.mkdir(path, recursive).await
    }

    async fn remove(&self, path: &str, recursive: bool) -> Result<()> {
        if !self.writable {
            anyhow::bail!("EROFS: read-only file system");
        }
        // 只能删除 upper 中的文件
        self.upper.remove(path, recursive).await
    }

    fn is_read_only(&self) -> bool {
        !self.writable
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
    async fn test_read_through() {
        let lower = InMemoryFs::new()
            .with_file("/readme.md", "from lower")
            .with_dir("/docs");

        let fs = OverlayFs::read_only(Box::new(lower));

        assert_eq!(fs.read_file("/readme.md").await.unwrap(), "from lower");
        assert!(fs.exists("/docs").await.unwrap());
    }

    #[tokio::test]
    async fn test_write_to_upper() {
        let lower = InMemoryFs::new().with_file("/readme.md", "from lower");

        let fs = OverlayFs::writable(Box::new(lower));

        fs.write_file("/readme.md", "modified").await.unwrap();
        assert_eq!(fs.read_file("/readme.md").await.unwrap(), "modified");

        fs.write_file("/new.md", "new file").await.unwrap();
        assert_eq!(fs.read_file("/new.md").await.unwrap(), "new file");
    }

    #[tokio::test]
    async fn test_read_only_rejects_write() {
        let lower = InMemoryFs::new();
        let fs = OverlayFs::read_only(Box::new(lower));

        assert!(fs.write_file("/test.txt", "hello").await.is_err());
        assert!(fs.mkdir("/dir", false).await.is_err());
    }

    #[tokio::test]
    async fn test_merged_readdir() {
        let lower = InMemoryFs::new()
            .with_file("/a.md", "a")
            .with_file("/b.md", "b");

        let fs = OverlayFs::writable(Box::new(lower));
        fs.write_file("/c.md", "c").await.unwrap();

        let entries = fs.read_dir("/").await.unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"a.md"));
        assert!(names.contains(&"b.md"));
        assert!(names.contains(&"c.md"));
    }
}
