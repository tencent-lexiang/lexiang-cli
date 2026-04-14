//! cat 命令: 读取并输出文件内容
//!
//! 支持的选项:
//! - `-n` — 显示行号
//! - 多个文件 — 依次输出

use super::registry::{Command, CommandContext, CommandOutput};
use crate::shell::fs;
use crate::submit_command;
use anyhow::Result;
use async_trait::async_trait;

#[derive(Default)]
pub struct CatCommand;

#[async_trait]
impl Command for CatCommand {
    fn name(&self) -> &str {
        "cat"
    }

    async fn execute(
        &self,
        args: &[String],
        ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        let mut show_line_numbers = false;
        let mut files: Vec<&str> = Vec::new();

        for arg in args {
            if arg == "-n" || arg == "--number" {
                show_line_numbers = true;
            } else if arg.starts_with('-') {
                // 忽略未知选项
            } else {
                files.push(arg);
            }
        }

        // 没有文件参数: 从 stdin 读取
        if files.is_empty() {
            if let Some(ref input) = ctx.stdin {
                if show_line_numbers {
                    return Ok(CommandOutput::success(add_line_numbers(input)));
                }
                return Ok(CommandOutput::success(input.clone()));
            }
            return Ok(CommandOutput::error("cat: missing operand".to_string()));
        }

        let mut output = String::new();
        for file in files {
            let path = fs::join_path(ctx.cwd, file);
            match ctx.fs.read_file(&path).await {
                Ok(content) => {
                    if show_line_numbers {
                        output.push_str(&add_line_numbers(&content));
                    } else {
                        output.push_str(&content);
                    }
                }
                Err(e) => {
                    return Ok(CommandOutput::error(format!("cat: {file}: {e}")));
                }
            }
        }

        Ok(CommandOutput::success(output))
    }
}

fn add_line_numbers(content: &str) -> String {
    let mut result = String::new();
    for (i, line) in content.lines().enumerate() {
        result.push_str(&format!("{:>6}\t{line}\n", i + 1));
    }
    result
}

// ─── 命令注册 ────────────────────────────────────────────
submit_command!(CatCommand);
