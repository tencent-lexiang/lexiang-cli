//! 命令注册表 + Command trait

use crate::shell::fs::IFileSystem;
use crate::shell::interpreter::Environment;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

/// 命令执行上下文
pub struct CommandContext<'a> {
    /// 虚拟文件系统
    pub fs: &'a dyn IFileSystem,
    /// Shell 环境
    pub env: &'a mut Environment,
    /// 管道输入 (前一个命令的 stdout)
    pub stdin: Option<String>,
    /// 当前工作目录
    pub cwd: &'a str,
}

/// 命令输出
#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl CommandOutput {
    /// 成功输出
    pub fn success(stdout: String) -> Self {
        Self {
            stdout,
            stderr: String::new(),
            exit_code: 0,
        }
    }

    /// 错误输出
    pub fn error(message: String) -> Self {
        Self {
            stdout: String::new(),
            stderr: message,
            exit_code: 1,
        }
    }
}

/// 命令 trait: 每个内置命令都实现此 trait
#[async_trait]
pub trait Command: Send + Sync {
    /// 命令名称
    fn name(&self) -> &str;

    /// 命令别名 (如 "ll" → "ls -l")
    fn aliases(&self) -> &[&str] {
        &[]
    }

    /// 执行命令
    async fn execute(&self, args: &[String], ctx: &mut CommandContext<'_>)
        -> Result<CommandOutput>;
}

/// 命令注册表
pub struct CommandRegistry {
    commands: HashMap<String, Box<dyn Command>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    /// 注册一个命令
    pub fn register(&mut self, cmd: Box<dyn Command>) {
        // 注册别名
        for alias in cmd.aliases() {
            // 注意: 别名指向同一个命令名
            self.commands.insert(
                (*alias).to_string(),
                Box::new(AliasCommand {
                    alias: (*alias).to_string(),
                    target: cmd.name().to_string(),
                }),
            );
        }
        self.commands.insert(cmd.name().to_string(), cmd);
    }

    /// 查找命令
    pub fn get(&self, name: &str) -> Option<&dyn Command> {
        self.commands.get(name).map(std::convert::AsRef::as_ref)
    }

    /// 列出所有已注册的命令名
    pub fn list_commands(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .commands
            .keys()
            .map(std::string::String::as_str)
            .collect();
        names.sort();
        names
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRegistry {
    /// 从 inventory 自动收集所有 `submit_command!` 注册的命令
    fn collect() -> Self {
        let mut registry = Self::new();
        for entry in inventory::iter::<CommandEntry> {
            registry.register((entry.factory)());
        }
        registry
    }
}

// ─── 分布式命令注册 (类似 derive) ─────────────────────────

/// 命令注册项 — 由 `submit_command!` 宏自动提交到全局收集器
pub struct CommandEntry {
    pub factory: fn() -> Box<dyn Command>,
}

// 让 inventory crate 能够收集 CommandEntry
inventory::collect!(CommandEntry);

/// 声明式命令注册宏
///
/// 在每个命令模块末尾使用，类似 `#[derive(...)]` 的效果:
///
/// ```rust
/// pub struct LsCommand;
///
/// #[async_trait]
/// impl Command for LsCommand { ... }
///
/// // 一行注册 ✨
/// submit_command!(LsCommand);
/// ```
///
/// 支持两种形式:
/// - `submit_command!(MyCommand)` — 注册实现了 Default 的命令
/// - `submit_command!(|| Box::new(ReadOnlyGuardCommand::new("rm")))` — 注册自定义构造
#[macro_export]
macro_rules! submit_command {
    // 形式 1: 简单结构体 (需实现 Default 或是 unit struct)
    ($cmd_ty:ty) => {
        inventory::submit! {
            $crate::shell::commands::registry::CommandEntry {
                factory: || -> Box<dyn $crate::shell::commands::registry::Command> {
                    Box::new(<$cmd_ty>::default())
                },
            }
        }
    };
    // 形式 2: 自定义工厂闭包
    (|| $factory:expr) => {
        inventory::submit! {
            $crate::shell::commands::registry::CommandEntry {
                factory: || -> Box<dyn $crate::shell::commands::registry::Command> {
                    $factory
                },
            }
        }
    };
}

/// 从 inventory 收集所有已注册命令，创建 `CommandRegistry`
pub fn create_default_registry() -> CommandRegistry {
    CommandRegistry::collect()
}

/// 别名命令 (转发到目标命令)
struct AliasCommand {
    alias: String,
    target: String,
}

#[async_trait]
impl Command for AliasCommand {
    fn name(&self) -> &str {
        &self.alias
    }

    async fn execute(
        &self,
        _args: &[String],
        _ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        // 别名在 executor 层面处理，不应该直接执行
        Ok(CommandOutput::error(format!(
            "{}: alias for {} (should be resolved by executor)",
            self.alias, self.target
        )))
    }
}
