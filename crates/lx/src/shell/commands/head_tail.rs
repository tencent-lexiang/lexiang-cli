//! head 命令: 输出文件的前 N 行
//!
//! 支持的选项:
//! - `-n N` / `-N` — 显示前 N 行 (默认 10)

use super::registry::{Command, CommandContext, CommandOutput};
use crate::shell::fs;
use crate::submit_command;
use anyhow::Result;
use async_trait::async_trait;

#[derive(Default)]
pub struct HeadCommand;

#[async_trait]
impl Command for HeadCommand {
    fn name(&self) -> &str {
        "head"
    }

    async fn execute(
        &self,
        args: &[String],
        ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        let (n, files) = parse_head_tail_args(args, 10);

        // 从 stdin 读取
        if files.is_empty() {
            if let Some(ref input) = ctx.stdin {
                let result = take_lines(input, n, true);
                return Ok(CommandOutput::success(result));
            }
            return Ok(CommandOutput::error("head: missing operand".to_string()));
        }

        let mut output = String::new();
        let multi = files.len() > 1;

        for file in &files {
            if multi {
                output.push_str(&format!("==> {file} <==\n"));
            }
            let path = fs::join_path(ctx.cwd, file);
            match ctx.fs.read_file(&path).await {
                Ok(content) => {
                    output.push_str(&take_lines(&content, n, true));
                }
                Err(e) => {
                    output.push_str(&format!("head: {file}: {e}\n"));
                }
            }
        }

        Ok(CommandOutput::success(output))
    }
}

/// tail 命令: 输出文件的后 N 行
#[derive(Default)]
pub struct TailCommand;

#[async_trait]
impl Command for TailCommand {
    fn name(&self) -> &str {
        "tail"
    }

    async fn execute(
        &self,
        args: &[String],
        ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        let (n, files) = parse_head_tail_args(args, 10);

        if files.is_empty() {
            if let Some(ref input) = ctx.stdin {
                let result = take_lines(input, n, false);
                return Ok(CommandOutput::success(result));
            }
            return Ok(CommandOutput::error("tail: missing operand".to_string()));
        }

        let mut output = String::new();
        let multi = files.len() > 1;

        for file in &files {
            if multi {
                output.push_str(&format!("==> {file} <==\n"));
            }
            let path = fs::join_path(ctx.cwd, file);
            match ctx.fs.read_file(&path).await {
                Ok(content) => {
                    output.push_str(&take_lines(&content, n, false));
                }
                Err(e) => {
                    output.push_str(&format!("tail: {file}: {e}\n"));
                }
            }
        }

        Ok(CommandOutput::success(output))
    }
}

/// 解析 head/tail 的参数: -n N 或 -N
fn parse_head_tail_args(args: &[String], default_n: usize) -> (usize, Vec<String>) {
    let mut n = default_n;
    let mut files = Vec::new();
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];
        if arg == "-n" {
            i += 1;
            if i < args.len() {
                n = args[i].parse().unwrap_or(default_n);
            }
        } else if arg.starts_with('-') && arg[1..].parse::<usize>().is_ok() {
            n = arg[1..].parse().unwrap_or(default_n);
        } else if !arg.starts_with('-') {
            files.push(arg.clone());
        }
        i += 1;
    }

    (n, files)
}

/// 取前 N 行或后 N 行
fn take_lines(content: &str, n: usize, from_start: bool) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let selected = if from_start {
        &lines[..n.min(lines.len())]
    } else {
        let start = lines.len().saturating_sub(n);
        &lines[start..]
    };

    let mut result = selected.join("\n");
    if !result.is_empty() {
        result.push('\n');
    }
    result
}

// ─── 命令注册 ────────────────────────────────────────────
submit_command!(HeadCommand);
submit_command!(TailCommand);
