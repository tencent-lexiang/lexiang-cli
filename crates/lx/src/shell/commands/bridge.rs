//! CLI 命令桥接: 将 lx 的 cmd 模块命令注入虚拟 shell
//!
//! 设计目标:
//! - `git status` / `git log` → 调用 lx git 的实现
//! - `search keyword` → 调用 lx search (动态命令)
//! - `worktree list` → 调用 lx worktree list
//! - 支持运行时动态注册任意外部命令
//!
//! 原理:
//! 桥接命令实现 Command trait，但内部通过回调函数委托给外部实现。
//! 外部代码注册 `BridgeFn`，shell 执行时回调外部逻辑。

use super::registry::{Command, CommandContext, CommandOutput};
use anyhow::Result;
use async_trait::async_trait;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// 桥接函数签名
///
/// 接收: 子命令+参数 (如 git ["status"] 或 search ["keyword"])
/// 返回: (stdout, stderr, `exit_code`)
pub type BridgeFn = Arc<
    dyn Fn(Vec<String>) -> Pin<Box<dyn Future<Output = Result<(String, String, i32)>> + Send>>
        + Send
        + Sync,
>;

/// 桥接命令定义
pub struct BridgeDef {
    /// 命令名 (如 "git", "search", "worktree")
    pub name: String,
    /// 命令描述
    pub description: String,
    /// 子命令列表 (用于帮助信息，可为空)
    pub subcommands: Vec<String>,
    /// 执行回调
    pub handler: BridgeFn,
}

/// 桥接命令 — 实现 Command trait
#[allow(dead_code)]
pub struct BridgeCommand {
    cmd_name: String,
    description: String,
    subcommands: Vec<String>,
    handler: BridgeFn,
}

impl BridgeCommand {
    pub fn new(def: BridgeDef) -> Self {
        Self {
            cmd_name: def.name,
            description: def.description,
            subcommands: def.subcommands,
            handler: def.handler,
        }
    }
}

#[async_trait]
impl Command for BridgeCommand {
    fn name(&self) -> &str {
        &self.cmd_name
    }

    async fn execute(
        &self,
        args: &[String],
        _ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        let full_args: Vec<String> = args.to_vec();

        match (self.handler)(full_args).await {
            Ok((stdout, stderr, exit_code)) => Ok(CommandOutput {
                stdout,
                stderr,
                exit_code,
            }),
            Err(e) => Ok(CommandOutput::error(format!("{}: {e}", self.cmd_name))),
        }
    }
}

/// 帮助命令 — 列出所有桥接命令及其子命令
pub struct BridgeHelpCommand {
    bridges: Vec<(String, String, Vec<String>)>, // (name, description, subcommands)
}

impl BridgeHelpCommand {
    pub fn new(bridges: &[BridgeDef]) -> Self {
        Self {
            bridges: bridges
                .iter()
                .map(|b| (b.name.clone(), b.description.clone(), b.subcommands.clone()))
                .collect(),
        }
    }
}

#[async_trait]
impl Command for BridgeHelpCommand {
    fn name(&self) -> &str {
        "help"
    }

    async fn execute(
        &self,
        args: &[String],
        _ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        let mut output = String::new();

        if let Some(cmd_name) = args.first() {
            // 查找特定命令的帮助
            if let Some((name, desc, subs)) = self.bridges.iter().find(|(n, _, _)| n == cmd_name) {
                output.push_str(&format!("{name} — {desc}\n\n"));
                if !subs.is_empty() {
                    output.push_str("Subcommands:\n");
                    for sub in subs {
                        output.push_str(&format!("  {name} {sub}\n"));
                    }
                }
            } else {
                output.push_str(&format!("{cmd_name}: no help available\n"));
            }
        } else {
            output.push_str("Available bridge commands:\n\n");
            for (name, desc, _) in &self.bridges {
                output.push_str(&format!("  {name:16} {desc}\n"));
            }
            output.push_str("\nUse 'help <command>' for more info.\n");
        }

        Ok(CommandOutput::success(output))
    }
}

/// 桥接命令注册器
///
/// 便于批量创建桥接命令，然后一次性注入到 `CommandRegistry`。
pub struct BridgeRegistry {
    definitions: Vec<BridgeDef>,
}

impl BridgeRegistry {
    pub fn new() -> Self {
        Self {
            definitions: Vec::new(),
        }
    }

    /// 注册一个桥接命令
    pub fn register(
        &mut self,
        name: &str,
        description: &str,
        subcommands: Vec<&str>,
        handler: BridgeFn,
    ) {
        self.definitions.push(BridgeDef {
            name: name.to_string(),
            description: description.to_string(),
            subcommands: subcommands.iter().map(|s| (*s).to_string()).collect(),
            handler,
        });
    }

    /// 消费自身，返回所有 `BridgeCommand` 实例（用于注册到 `CommandRegistry`）
    pub fn into_commands(self) -> Vec<Box<dyn Command>> {
        let help = Box::new(BridgeHelpCommand::new(&self.definitions)) as Box<dyn Command>;

        let mut commands: Vec<Box<dyn Command>> = self
            .definitions
            .into_iter()
            .map(|def| Box::new(BridgeCommand::new(def)) as Box<dyn Command>)
            .collect();

        commands.push(help);
        commands
    }

    /// 获取定义列表引用 (用于生成 help)
    pub fn definitions(&self) -> &[BridgeDef] {
        &self.definitions
    }
}

impl Default for BridgeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 便捷宏: 从一个 async fn 创建 `BridgeFn`
///
/// 用法:
/// ```ignore
/// let handler = bridge_fn!(|args: Vec<String>| async move {
///     Ok(("output".to_string(), String::new(), 0))
/// });
/// ```
#[macro_export]
macro_rules! bridge_fn {
    ($f:expr) => {
        std::sync::Arc::new(move |args: Vec<String>| {
            let f = $f;
            Box::pin(f(args))
                as std::pin::Pin<
                    Box<
                        dyn std::future::Future<Output = anyhow::Result<(String, String, i32)>>
                            + Send,
                    >,
                >
        }) as $crate::shell::commands::bridge::BridgeFn
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::fs::InMemoryFs;
    use crate::shell::interpreter::Environment;

    #[tokio::test]
    async fn test_bridge_command() {
        let handler: BridgeFn = Arc::new(|args: Vec<String>| {
            Box::pin(async move {
                let output = format!("bridge received: {:?}", args);
                Ok((output, String::new(), 0))
            })
        });

        let cmd = BridgeCommand::new(BridgeDef {
            name: "test-cmd".to_string(),
            description: "Test bridge".to_string(),
            subcommands: vec!["sub1".to_string()],
            handler,
        });

        let fs = InMemoryFs::new();
        let mut env = Environment::default();
        let mut ctx = CommandContext {
            fs: &fs,
            env: &mut env,
            stdin: None,
            cwd: "/",
        };

        let result = cmd
            .execute(&["sub1".to_string(), "arg1".to_string()], &mut ctx)
            .await
            .unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("sub1"));
        assert!(result.stdout.contains("arg1"));
    }

    #[tokio::test]
    async fn test_bridge_registry() {
        let mut registry = BridgeRegistry::new();

        let handler: BridgeFn = Arc::new(|args: Vec<String>| {
            Box::pin(async move { Ok((format!("git: {:?}", args), String::new(), 0)) })
        });

        registry.register(
            "git",
            "Git-style commands for knowledge base",
            vec!["status", "log", "diff", "pull", "push"],
            handler,
        );

        let commands = registry.into_commands();
        assert!(commands.len() >= 2); // git + help
    }

    #[tokio::test]
    async fn test_help_command() {
        let defs = vec![
            BridgeDef {
                name: "git".to_string(),
                description: "Git operations".to_string(),
                subcommands: vec!["status".to_string(), "log".to_string()],
                handler: Arc::new(|_| Box::pin(async { Ok(("".to_string(), "".to_string(), 0)) })),
            },
            BridgeDef {
                name: "search".to_string(),
                description: "Search knowledge base".to_string(),
                subcommands: vec![],
                handler: Arc::new(|_| Box::pin(async { Ok(("".to_string(), "".to_string(), 0)) })),
            },
        ];

        let help = BridgeHelpCommand::new(&defs);
        let fs = InMemoryFs::new();
        let mut env = Environment::default();
        let mut ctx = CommandContext {
            fs: &fs,
            env: &mut env,
            stdin: None,
            cwd: "/",
        };

        // 列出所有命令
        let result = help.execute(&[], &mut ctx).await.unwrap();
        assert!(result.stdout.contains("git"));
        assert!(result.stdout.contains("search"));

        // 特定命令帮助
        let result = help.execute(&["git".to_string()], &mut ctx).await.unwrap();
        assert!(result.stdout.contains("status"));
        assert!(result.stdout.contains("log"));
    }
}
