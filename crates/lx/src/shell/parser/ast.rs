//! AST (Abstract Syntax Tree) 节点定义
//!
//! 对标 just-bash 的 AST 结构，支持：
//! - 简单命令 (ls -la /kb)
//! - 管道 (cat file | grep pattern | head -5)
//! - 命令列表 (cmd1 && cmd2 || cmd3; cmd4)
//! - 重定向 (cmd > file, cmd >> file, cmd < file)
//! - 变量扩展 ($VAR, ${VAR})
//! - 引号 ('literal', "with $expansion")
//! - Glob 模式 (*.md, docs/*)

/// 脚本顶层结构：由一个或多个命令列表组成
#[derive(Debug, Clone)]
pub struct Script {
    pub command_lists: Vec<CommandList>,
}

/// 命令列表: 用 &&, ||, ; 连接的管道序列
#[derive(Debug, Clone)]
pub struct CommandList {
    pub first: Pipeline,
    pub rest: Vec<(ListOp, Pipeline)>,
}

/// 列表连接操作符
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListOp {
    /// `&&` — 前一个成功才执行
    And,
    /// `||` — 前一个失败才执行
    Or,
    /// `;` — 顺序执行
    Sequence,
}

/// 管道: 一个或多个简单命令用 `|` 连接
#[derive(Debug, Clone)]
pub struct Pipeline {
    pub commands: Vec<SimpleCommand>,
}

/// 简单命令: 命令名 + 参数 + 重定向
#[derive(Debug, Clone)]
pub struct SimpleCommand {
    /// 命令名 (如 "ls", "cat", "grep")
    pub name: Word,
    /// 命令参数列表
    pub args: Vec<Word>,
    /// 重定向列表
    pub redirects: Vec<Redirect>,
}

/// "词" — Shell 中的一个 token 单元
/// 可以是纯文本、引号包裹、变量引用或 glob 模式
#[derive(Debug, Clone)]
pub enum Word {
    /// 普通文字 (无引号)
    Literal(String),
    /// 单引号字符串 (不做任何扩展)
    SingleQuoted(String),
    /// 双引号字符串 (支持变量扩展)
    DoubleQuoted(Vec<WordPart>),
    /// 变量引用: $VAR 或 ${VAR}
    Variable(String),
    /// Glob 模式: *.md, docs/**
    Glob(String),
    /// 拼接词: 多个部分连接 (如 "prefix"$VAR"suffix")
    Concat(Vec<Word>),
}

/// 双引号内的部分
#[derive(Debug, Clone)]
pub enum WordPart {
    /// 纯文本
    Text(String),
    /// 变量引用
    Variable(String),
}

/// 重定向
#[derive(Debug, Clone)]
pub struct Redirect {
    /// 文件描述符 (None = 默认: > 用 stdout, < 用 stdin)
    pub fd: Option<i32>,
    /// 重定向类型
    pub op: RedirectOp,
    /// 目标文件
    pub target: Word,
}

/// 重定向操作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectOp {
    /// `>` — 覆盖写入
    Write,
    /// `>>` — 追加写入
    Append,
    /// `<` — 读取输入
    Read,
    /// `2>` — 重定向 stderr
    WriteStderr,
}

impl Word {
    /// 从 Word 提取字面量文本 (不处理扩展)
    /// 用于不需要变量扩展的场景
    pub fn as_literal(&self) -> Option<&str> {
        match self {
            Word::Literal(s) | Word::SingleQuoted(s) | Word::Glob(s) => Some(s),
            _ => None,
        }
    }
}

impl SimpleCommand {
    /// 创建一个只有名称的简单命令 (无参数，无重定向)
    pub fn new(name: Word) -> Self {
        Self {
            name,
            args: Vec::new(),
            redirects: Vec::new(),
        }
    }
}

impl Pipeline {
    /// 创建只含一个命令的管道
    pub fn single(cmd: SimpleCommand) -> Self {
        Self {
            commands: vec![cmd],
        }
    }
}

impl CommandList {
    /// 创建只含一个管道的命令列表
    pub fn single(pipeline: Pipeline) -> Self {
        Self {
            first: pipeline,
            rest: Vec::new(),
        }
    }
}

impl Script {
    /// 创建只含一个命令列表的脚本
    pub fn single(list: CommandList) -> Self {
        Self {
            command_lists: vec![list],
        }
    }
}
