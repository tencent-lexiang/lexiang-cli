//! grep 命令: 搜索匹配模式的行
//!
//! 支持的选项:
//! - `-i` — 忽略大小写
//! - `-r` / `-R` — 递归搜索
//! - `-n` — 显示行号
//! - `-c` — 只显示匹配计数
//! - `-l` — 只显示匹配文件名
//! - `-v` — 反向匹配
//! - `--include` — 文件名过滤
//! - `-C N` / `--context N` — 上下文行数

use super::registry::{Command, CommandContext, CommandOutput};
use crate::shell::fs::{self, FileType, IFileSystem};
use crate::submit_command;
use anyhow::Result;
use async_trait::async_trait;

#[derive(Default)]
pub struct GrepCommand;

#[async_trait]
impl Command for GrepCommand {
    fn name(&self) -> &str {
        "grep"
    }

    async fn execute(
        &self,
        args: &[String],
        ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        let opts = match parse_grep_args(args) {
            Ok(opts) => opts,
            Err(e) => return Ok(CommandOutput::error(format!("grep: {e}"))),
        };

        // 如果有 stdin (管道模式), 对 stdin 内容搜索
        if let Some(ref input) = ctx.stdin {
            let result = grep_content(input, &opts, None);
            return Ok(CommandOutput::success(result));
        }

        // 没有文件参数
        if opts.paths.is_empty() {
            return Ok(CommandOutput::error(
                "grep: missing file operand".to_string(),
            ));
        }

        let mut output = String::new();
        let multiple_files = opts.paths.len() > 1 || opts.recursive;

        for path_arg in &opts.paths {
            let path = fs::join_path(ctx.cwd, path_arg);

            if opts.recursive {
                grep_recursive(ctx.fs, &path, &opts, multiple_files, &mut output).await?;
            } else {
                match ctx.fs.read_file(&path).await {
                    Ok(content) => {
                        let prefix = if multiple_files {
                            Some(path_arg.as_str())
                        } else {
                            None
                        };
                        output.push_str(&grep_content(&content, &opts, prefix));
                    }
                    Err(e) => {
                        output.push_str(&format!("grep: {path_arg}: {e}\n"));
                    }
                }
            }
        }

        if output.is_empty() && !opts.count_only {
            Ok(CommandOutput {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 1, // grep 没有匹配返回 1
            })
        } else {
            Ok(CommandOutput::success(output))
        }
    }
}

struct GrepOptions {
    pattern: String,
    paths: Vec<String>,
    ignore_case: bool,
    recursive: bool,
    line_number: bool,
    count_only: bool,
    files_only: bool,
    invert: bool,
    include: Option<String>,
    context_lines: usize,
}

fn parse_grep_args(args: &[String]) -> Result<GrepOptions, String> {
    let mut opts = GrepOptions {
        pattern: String::new(),
        paths: Vec::new(),
        ignore_case: false,
        recursive: false,
        line_number: false,
        count_only: false,
        files_only: false,
        invert: false,
        include: None,
        context_lines: 0,
    };

    let mut i = 0;
    let mut pattern_set = false;

    while i < args.len() {
        let arg = &args[i];

        if arg.starts_with("--include=") {
            opts.include = Some(arg.strip_prefix("--include=").unwrap().to_string());
        } else if arg == "--include" {
            i += 1;
            if i < args.len() {
                opts.include = Some(args[i].clone());
            }
        } else if arg == "--context" || arg == "-C" {
            i += 1;
            if i < args.len() {
                opts.context_lines = args[i].parse().unwrap_or(0);
            }
        } else if arg.starts_with('-') && !arg.starts_with("--") && arg.len() > 1 {
            // 短选项组合: -rni
            let chars: Vec<char> = arg[1..].chars().collect();
            let mut j = 0;
            while j < chars.len() {
                match chars[j] {
                    'i' => opts.ignore_case = true,
                    'r' | 'R' => opts.recursive = true,
                    'n' => opts.line_number = true,
                    'c' => opts.count_only = true,
                    'l' => opts.files_only = true,
                    'v' => opts.invert = true,
                    'C' => {
                        // -C3 或 -C 3
                        let rest: String = chars[j + 1..].iter().collect();
                        if !rest.is_empty() {
                            opts.context_lines = rest.parse().unwrap_or(0);
                            j = chars.len(); // 消耗完
                            continue;
                        } else {
                            i += 1;
                            if i < args.len() {
                                opts.context_lines = args[i].parse().unwrap_or(0);
                            }
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
        } else if !pattern_set {
            opts.pattern = arg.clone();
            pattern_set = true;
        } else {
            opts.paths.push(arg.clone());
        }

        i += 1;
    }

    if !pattern_set {
        return Err("missing pattern".to_string());
    }

    // 如果没有路径参数，默认当前目录 (递归) 或 stdin
    if opts.paths.is_empty() && opts.recursive {
        opts.paths.push(".".to_string());
    }

    // 递归模式默认显示行号
    if opts.recursive {
        opts.line_number = true;
    }

    Ok(opts)
}

/// 在内容中搜索匹配行
fn grep_content(content: &str, opts: &GrepOptions, file_prefix: Option<&str>) -> String {
    let mut output = String::new();
    let mut match_count = 0;

    let lines: Vec<&str> = content.lines().collect();

    for (line_idx, line) in lines.iter().enumerate() {
        let matched = if opts.ignore_case {
            line.to_lowercase().contains(&opts.pattern.to_lowercase())
        } else {
            line.contains(&opts.pattern)
        };

        let matched = if opts.invert { !matched } else { matched };

        if matched {
            match_count += 1;

            if opts.files_only {
                if let Some(prefix) = file_prefix {
                    return format!("{prefix}\n");
                }
                return String::new();
            }

            if !opts.count_only {
                // 上下文行 (before)
                if opts.context_lines > 0 {
                    let start = line_idx.saturating_sub(opts.context_lines);
                    for (ctx_idx, ctx_line) in
                        lines.iter().enumerate().skip(start).take(line_idx - start)
                    {
                        format_grep_line(
                            &mut output,
                            file_prefix,
                            opts.line_number,
                            ctx_idx + 1,
                            ctx_line,
                            '-',
                        );
                    }
                }

                format_grep_line(
                    &mut output,
                    file_prefix,
                    opts.line_number,
                    line_idx + 1,
                    line,
                    ':',
                );

                // 上下文行 (after)
                if opts.context_lines > 0 {
                    let end = (line_idx + opts.context_lines + 1).min(lines.len());
                    for (ctx_idx, ctx_line) in lines
                        .iter()
                        .enumerate()
                        .skip(line_idx + 1)
                        .take(end - line_idx - 1)
                    {
                        format_grep_line(
                            &mut output,
                            file_prefix,
                            opts.line_number,
                            ctx_idx + 1,
                            ctx_line,
                            '-',
                        );
                    }
                    if end < lines.len() {
                        output.push_str("--\n");
                    }
                }
            }
        }
    }

    if opts.count_only {
        if let Some(prefix) = file_prefix {
            return format!("{prefix}:{match_count}\n");
        }
        return format!("{match_count}\n");
    }

    output
}

fn format_grep_line(
    output: &mut String,
    file_prefix: Option<&str>,
    show_line_number: bool,
    line_number: usize,
    line: &str,
    separator: char,
) {
    if let Some(prefix) = file_prefix {
        output.push_str(prefix);
        output.push(separator);
    }
    if show_line_number {
        output.push_str(&format!("{line_number}{separator}"));
    }
    output.push_str(line);
    output.push('\n');
}

/// 递归搜索目录
#[async_recursion::async_recursion]
async fn grep_recursive(
    fs: &dyn IFileSystem,
    path: &str,
    opts: &GrepOptions,
    show_filename: bool,
    output: &mut String,
) -> Result<()> {
    let stat = fs.stat(path).await;

    match stat {
        Ok(stat) if stat.file_type == FileType::File => {
            // 检查 --include 过滤
            if let Some(ref pattern) = opts.include {
                let filename = fs::basename(path);
                if !matches_glob(&filename, pattern) {
                    return Ok(());
                }
            }

            if let Ok(content) = fs.read_file(path).await {
                let prefix = if show_filename { Some(path) } else { None };
                output.push_str(&grep_content(&content, opts, prefix));
            }
        }
        Ok(stat) if stat.file_type == FileType::Directory => {
            if let Ok(entries) = fs.read_dir(path).await {
                for entry in entries {
                    let child_path = fs::join_path(path, &entry.name);
                    grep_recursive(fs, &child_path, opts, true, output).await?;
                }
            }
        }
        _ => {}
    }

    Ok(())
}

/// 简单的 glob 匹配 (支持 *.ext 和 *pattern*)
fn matches_glob(name: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    // 移除可能的引号
    let pattern = pattern.trim_matches('"').trim_matches('\'');

    if let Some(ext) = pattern.strip_prefix("*.") {
        return name.ends_with(&format!(".{ext}"));
    }

    if pattern.starts_with('*') && pattern.ends_with('*') {
        let inner = &pattern[1..pattern.len() - 1];
        return name.contains(inner);
    }

    if let Some(suffix) = pattern.strip_prefix('*') {
        return name.ends_with(suffix);
    }

    if let Some(prefix) = pattern.strip_suffix('*') {
        return name.starts_with(prefix);
    }

    name == pattern
}

// ─── 命令注册 ────────────────────────────────────────────
submit_command!(GrepCommand);
