//! Shell 解释器
//!
//! - `environment` — Shell 环境 (变量、cwd)
//! - `executor` — 命令执行引擎

pub mod environment;
pub mod executor;

pub use environment::Environment;
pub use executor::Executor;
