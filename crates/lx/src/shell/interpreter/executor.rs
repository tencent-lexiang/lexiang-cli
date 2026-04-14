//! 命令执行器: 遍历 AST 并执行命令
//!
//! 核心职责:
//! - 别名解析: rg → grep (参数翻译)
//! - 管道串联: cmd1 stdout → cmd2 stdin
//! - 变量扩展: $VAR → 值
//! - 重定向处理: > file, >> file, < file
//! - 命令列表: && || ; 逻辑控制
//!
//! 命令查找链:
//! 1. Alias 解析 (rg → grep, eza → ls, fd → find, ...)
//! 2. 内置命令 (ls, cat, grep, find, tree, ...)
//! 3. 桥接命令 (git, search, worktree, ...)
//! 4. command not found

use super::environment::Environment;
use crate::shell::commands::alias::AliasTable;
use crate::shell::commands::{CommandContext, CommandOutput, CommandRegistry};
use crate::shell::fs::IFileSystem;
use crate::shell::parser::ast::*;
use anyhow::{bail, Result};

/// 命令执行器
pub struct Executor {
    registry: CommandRegistry,
    aliases: AliasTable,
}

impl Executor {
    pub fn new(registry: CommandRegistry) -> Self {
        Self {
            registry,
            aliases: AliasTable::with_defaults(),
        }
    }

    /// 创建带自定义别名表的执行器
    pub fn with_aliases(registry: CommandRegistry, aliases: AliasTable) -> Self {
        Self { registry, aliases }
    }

    /// 获取别名表的可变引用 (用于运行时添加别名)
    pub fn aliases_mut(&mut self) -> &mut AliasTable {
        &mut self.aliases
    }

    /// 获取命令注册表的可变引用 (用于运行时注册命令)
    pub fn registry_mut(&mut self) -> &mut CommandRegistry {
        &mut self.registry
    }

    /// 获取命令注册表的不可变引用 (用于查询)
    pub fn registry_mut_ref(&self) -> &CommandRegistry {
        &self.registry
    }

    /// 获取别名表的不可变引用 (用于查询)
    pub fn aliases_ref(&self) -> &AliasTable {
        &self.aliases
    }

    /// 执行脚本
    pub async fn execute_script(
        &self,
        script: &Script,
        fs: &dyn IFileSystem,
        env: &mut Environment,
    ) -> Result<CommandOutput> {
        let mut last_output = CommandOutput::success(String::new());

        for command_list in &script.command_lists {
            last_output = self.execute_command_list(command_list, fs, env).await?;
        }

        Ok(last_output)
    }

    /// 执行命令列表 (处理 &&, ||, ; 逻辑)
    async fn execute_command_list(
        &self,
        list: &CommandList,
        fs: &dyn IFileSystem,
        env: &mut Environment,
    ) -> Result<CommandOutput> {
        let mut output = self.execute_pipeline(&list.first, fs, env).await?;
        env.set_exit_code(output.exit_code);

        for (op, pipeline) in &list.rest {
            match op {
                ListOp::And => {
                    if output.exit_code == 0 {
                        output = self.execute_pipeline(pipeline, fs, env).await?;
                        env.set_exit_code(output.exit_code);
                    }
                }
                ListOp::Or => {
                    if output.exit_code != 0 {
                        output = self.execute_pipeline(pipeline, fs, env).await?;
                        env.set_exit_code(output.exit_code);
                    }
                }
                ListOp::Sequence => {
                    output = self.execute_pipeline(pipeline, fs, env).await?;
                    env.set_exit_code(output.exit_code);
                }
            }
        }

        Ok(output)
    }

    /// 执行管道: cmd1 | cmd2 | cmd3
    /// 每个命令的 stdout 成为下一个的 stdin
    async fn execute_pipeline(
        &self,
        pipeline: &Pipeline,
        fs: &dyn IFileSystem,
        env: &mut Environment,
    ) -> Result<CommandOutput> {
        let mut stdin: Option<String> = None;
        let mut last_output = CommandOutput::success(String::new());

        for cmd in &pipeline.commands {
            let output = self
                .execute_simple_command(cmd, fs, env, stdin.take())
                .await;

            match output {
                Ok(out) => {
                    stdin = Some(out.stdout.clone());
                    last_output = out;
                }
                Err(e) => {
                    last_output = CommandOutput::error(format!("{e}"));
                    break;
                }
            }
        }

        Ok(last_output)
    }

    /// 执行单个命令
    async fn execute_simple_command(
        &self,
        cmd: &SimpleCommand,
        fs: &dyn IFileSystem,
        env: &mut Environment,
        stdin: Option<String>,
    ) -> Result<CommandOutput> {
        // 1. 扩展命令名
        let name = self.expand_word(&cmd.name, env);

        // 2. 扩展参数
        let args: Vec<String> = cmd.args.iter().map(|w| self.expand_word(w, env)).collect();

        // 3. 别名解析 — 核心新增!
        //    rg pattern → grep -rn pattern .
        //    eza -la → ls -la
        //    fd '*.md' → find . -name '*.md'
        let (resolved_name, resolved_args) =
            if let Some(expansion) = self.aliases.resolve(&name, &args) {
                (expansion.command, expansion.args)
            } else {
                (name.clone(), args)
            };

        // 4. 查找命令 (内置 + 桥接)
        let handler = self
            .registry
            .get(&resolved_name)
            .ok_or_else(|| anyhow::anyhow!("{name}: command not found"))?;

        // 5. 构建上下文
        let cwd = env.cwd().to_string();
        let mut ctx = CommandContext {
            fs,
            env,
            stdin,
            cwd: &cwd,
        };

        // 6. 执行命令
        let mut output = handler.execute(&resolved_args, &mut ctx).await?;

        // 7. 处理重定向
        for redirect in &cmd.redirects {
            let target = self.expand_word(&redirect.target, ctx.env);
            match redirect.op {
                RedirectOp::Write => {
                    let path = crate::shell::fs::join_path(ctx.cwd, &target);
                    fs.write_file(&path, &output.stdout).await?;
                    output.stdout.clear();
                }
                RedirectOp::Append => {
                    let path = crate::shell::fs::join_path(ctx.cwd, &target);
                    fs.append_file(&path, &output.stdout).await?;
                    output.stdout.clear();
                }
                RedirectOp::WriteStderr => {
                    // stderr 重定向到文件
                    let path = crate::shell::fs::join_path(ctx.cwd, &target);
                    fs.write_file(&path, &output.stderr).await?;
                    output.stderr.clear();
                }
                RedirectOp::Read => {
                    // 重定向输入 (不常用于虚拟 shell)
                    let path = crate::shell::fs::join_path(ctx.cwd, &target);
                    let _content = fs.read_file(&path).await?;
                    // stdin 已通过管道处理
                }
            }
        }

        Ok(output)
    }

    /// 扩展 Word 为字符串 (处理变量替换等)
    #[allow(clippy::only_used_in_recursion)]
    fn expand_word(&self, word: &Word, env: &Environment) -> String {
        match word {
            Word::Literal(s) | Word::SingleQuoted(s) => s.clone(),
            Word::DoubleQuoted(parts) => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        WordPart::Text(s) => result.push_str(s),
                        WordPart::Variable(name) => {
                            if let Some(value) = env.get(name) {
                                result.push_str(value);
                            }
                        }
                    }
                }
                result
            }
            Word::Variable(name) => env.get(name).unwrap_or("").to_string(),
            Word::Glob(pattern) => {
                // Glob 扩展在虚拟 shell 中比较复杂，暂时返回原始模式
                // 后续可以在命令层面实现 glob 匹配
                pattern.clone()
            }
            Word::Concat(words) => {
                let mut result = String::new();
                for w in words {
                    result.push_str(&self.expand_word(w, env));
                }
                result
            }
        }
    }
}

/// 验证命令名是否有效
pub fn validate_command_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("empty command name");
    }
    Ok(())
}
