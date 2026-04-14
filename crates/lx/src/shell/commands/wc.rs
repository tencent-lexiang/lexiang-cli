//! wc 命令: 统计行数、单词数、字符数
//!
//! 支持的选项:
//! - `-l` — 只显示行数
//! - `-w` — 只显示单词数
//! - `-c` — 只显示字符数 (bytes)

use super::registry::{Command, CommandContext, CommandOutput};
use crate::shell::fs;
use crate::submit_command;
use anyhow::Result;
use async_trait::async_trait;

#[derive(Default)]
pub struct WcCommand;

#[async_trait]
impl Command for WcCommand {
    fn name(&self) -> &str {
        "wc"
    }

    async fn execute(
        &self,
        args: &[String],
        ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        let opts = parse_wc_args(args);

        // stdin 模式
        if opts.files.is_empty() {
            if let Some(ref input) = ctx.stdin {
                let stats = count_stats(input);
                return Ok(CommandOutput::success(format_wc_output(
                    &stats, &opts, None,
                )));
            }
            return Ok(CommandOutput::error("wc: missing operand".to_string()));
        }

        let mut output = String::new();
        let mut total = WcStats::default();
        let multi = opts.files.len() > 1;

        for file in &opts.files {
            let path = fs::join_path(ctx.cwd, file);
            match ctx.fs.read_file(&path).await {
                Ok(content) => {
                    let stats = count_stats(&content);
                    output.push_str(&format_wc_output(&stats, &opts, Some(file)));
                    total.lines += stats.lines;
                    total.words += stats.words;
                    total.bytes += stats.bytes;
                }
                Err(e) => {
                    output.push_str(&format!("wc: {file}: {e}\n"));
                }
            }
        }

        if multi {
            output.push_str(&format_wc_output(&total, &opts, Some("total")));
        }

        Ok(CommandOutput::success(output))
    }
}

struct WcOptions {
    files: Vec<String>,
    lines_only: bool,
    words_only: bool,
    bytes_only: bool,
}

#[derive(Default)]
struct WcStats {
    lines: usize,
    words: usize,
    bytes: usize,
}

fn parse_wc_args(args: &[String]) -> WcOptions {
    let mut opts = WcOptions {
        files: Vec::new(),
        lines_only: false,
        words_only: false,
        bytes_only: false,
    };

    for arg in args {
        if arg.starts_with('-') && !arg.starts_with("--") {
            for ch in arg[1..].chars() {
                match ch {
                    'l' => opts.lines_only = true,
                    'w' => opts.words_only = true,
                    'c' => opts.bytes_only = true,
                    _ => {}
                }
            }
        } else {
            opts.files.push(arg.clone());
        }
    }

    opts
}

fn count_stats(content: &str) -> WcStats {
    WcStats {
        lines: content.lines().count(),
        words: content.split_whitespace().count(),
        bytes: content.len(),
    }
}

fn format_wc_output(stats: &WcStats, opts: &WcOptions, name: Option<&str>) -> String {
    let mut parts = Vec::new();

    let show_all = !opts.lines_only && !opts.words_only && !opts.bytes_only;

    if show_all || opts.lines_only {
        parts.push(format!("{:>8}", stats.lines));
    }
    if show_all || opts.words_only {
        parts.push(format!("{:>8}", stats.words));
    }
    if show_all || opts.bytes_only {
        parts.push(format!("{:>8}", stats.bytes));
    }

    let mut line = parts.join("");
    if let Some(name) = name {
        line.push_str(&format!(" {name}"));
    }
    line.push('\n');
    line
}

// ─── 命令注册 ────────────────────────────────────────────
submit_command!(WcCommand);
