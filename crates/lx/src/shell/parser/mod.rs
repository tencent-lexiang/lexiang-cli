//! Shell 命令解析器
//!
//! 将输入字符串解析为 AST:
//! - `lexer` — 词法分析 (字符串 → Token 流)
//! - `ast` — AST 节点定义
//! - `parser` — 语法分析 (Token 流 → AST)

pub mod ast;
pub mod lexer;
#[allow(clippy::module_inception)]
pub mod parser;

pub use ast::*;
pub use parser::{parse, ParseError};
