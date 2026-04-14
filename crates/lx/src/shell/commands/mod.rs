//! 内置命令模块
//!
//! 所有 shell 内置命令的实现和注册。
//!
//! ## 添加新命令
//!
//! 只需两步:
//! 1. 创建命令模块，实现 `Command` trait
//! 2. 在模块末尾添加 `submit_command!(YourCommand);`
//!
//! 命令会通过 `inventory` 自动收集到注册表中，无需在此文件手动注册。

pub mod alias;
mod awk;
pub mod bridge;
pub mod builtins;
mod cat;
mod cut;
mod find;
mod fzf;
mod grep;
mod head_tail;
mod ls;
pub mod registry;
mod tr;
mod tree;
mod wc;

pub use registry::{Command, CommandContext, CommandOutput, CommandRegistry};

/// 创建并注册所有内置命令 (自动收集)
pub fn create_default_registry() -> CommandRegistry {
    registry::create_default_registry()
}
