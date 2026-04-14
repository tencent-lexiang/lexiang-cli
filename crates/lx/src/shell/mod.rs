//! lx-shell: 虚拟 Shell 引擎
//!
//! 用 Rust 还原 just-bash 的核心能力，让 AI Agent 用 UNIX 命令操作乐享知识库。
//!
//! # 模块结构
//! - `fs` — `IFileSystem` trait + `InMemoryFs` / `OverlayFs` / `MountableFs`
//! - `parser` — Shell 命令解析器 (Lexer + AST + Parser)
//! - `interpreter` — 命令执行引擎 (管道、重定向、变量替换)
//! - `commands` — 内置命令实现 (ls, cat, grep, find, tree, ...)
//! - `bash` — Bash 主入口，组装所有组件

pub mod bash;
pub mod commands;
pub mod fs;
pub mod interpreter;
pub mod parser;
