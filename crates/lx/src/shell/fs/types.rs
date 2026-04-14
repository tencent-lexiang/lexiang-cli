use std::time::SystemTime;

/// 文件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    /// 普通文件
    File,
    /// 目录
    Directory,
    /// 符号链接
    Symlink,
}

impl FileType {
    pub fn is_file(self) -> bool {
        self == FileType::File
    }

    pub fn is_dir(self) -> bool {
        self == FileType::Directory
    }
}

impl std::fmt::Display for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileType::File => write!(f, "file"),
            FileType::Directory => write!(f, "directory"),
            FileType::Symlink => write!(f, "symlink"),
        }
    }
}

/// 目录条目
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// 文件/目录名 (不含路径前缀)
    pub name: String,
    /// 文件类型
    pub file_type: FileType,
    /// 文件大小 (bytes)，目录为 0
    pub size: u64,
    /// 最后修改时间
    pub modified: Option<SystemTime>,
    /// 额外元数据 (如 `entry_id` 等)
    pub metadata: Option<EntryMetadata>,
}

/// 文件状态信息 (对应 stat 命令输出)
#[derive(Debug, Clone)]
pub struct FileStat {
    /// 文件类型
    pub file_type: FileType,
    /// 文件大小 (bytes)
    pub size: u64,
    /// 创建时间
    pub created: Option<SystemTime>,
    /// 最后修改时间
    pub modified: Option<SystemTime>,
    /// 最后访问时间
    pub accessed: Option<SystemTime>,
    /// 是否只读
    pub readonly: bool,
    /// 额外元数据
    pub metadata: Option<EntryMetadata>,
}

/// 扩展元数据（乐享知识库特有的信息）
#[derive(Debug, Clone, Default)]
pub struct EntryMetadata {
    /// 乐享 `entry_id`
    pub entry_id: Option<String>,
    /// 乐享 `space_id`
    pub space_id: Option<String>,
    /// 条目类型 (page / folder / file)
    pub entry_type: Option<String>,
    /// 创建者
    pub creator: Option<String>,
}

impl DirEntry {
    /// 创建文件类型的目录条目
    pub fn file(name: impl Into<String>, size: u64) -> Self {
        Self {
            name: name.into(),
            file_type: FileType::File,
            size,
            modified: None,
            metadata: None,
        }
    }

    /// 创建目录类型的目录条目
    pub fn directory(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            file_type: FileType::Directory,
            size: 0,
            modified: None,
            metadata: None,
        }
    }
}

impl FileStat {
    /// 创建文件类型的 stat
    pub fn file(size: u64) -> Self {
        Self {
            file_type: FileType::File,
            size,
            created: None,
            modified: None,
            accessed: None,
            readonly: false,
            metadata: None,
        }
    }

    /// 创建目录类型的 stat
    pub fn directory() -> Self {
        Self {
            file_type: FileType::Directory,
            size: 0,
            created: None,
            modified: None,
            accessed: None,
            readonly: false,
            metadata: None,
        }
    }

    /// 标记为只读
    pub fn with_readonly(mut self, readonly: bool) -> Self {
        self.readonly = readonly;
        self
    }
}
