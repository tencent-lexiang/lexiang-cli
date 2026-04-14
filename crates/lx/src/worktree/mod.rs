//! Worktree 模块入口
//!
//! 提供类似 Git Worktree 的本地知识库管理能力。

mod config;
mod entries;
mod registry;
mod repository;
mod sync;

pub use config::WorktreeConfig;
pub use entries::{EntriesManager, EntriesMap, EntryType};
pub use registry::{WorktreeRecord, WorktreeRegistry};
pub use repository::Repository;
pub use sync::*;
