//! GitHub Release 更新检查模块
//!
//! 提供从 GitHub Release 检查新版本和下载更新的能力。

mod checker;

pub use checker::{CheckResult, UpdateChecker, UpdateConfig};
