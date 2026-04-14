//! find 命令: 在目录树中查找文件
//!
//! 支持的选项:
//! - `-name <pattern>` — 按文件名匹配
//! - `-type f|d` — 按类型过滤
//! - `-maxdepth N` — 最大深度

use super::registry::{Command, CommandContext, CommandOutput};
use crate::shell::fs::{self, FileType, IFileSystem};
use crate::submit_command;
use anyhow::Result;
use async_trait::async_trait;

#[derive(Default)]
pub struct FindCommand;

#[async_trait]
impl Command for FindCommand {
    fn name(&self) -> &str {
        "find"
    }

    async fn execute(
        &self,
        args: &[String],
        ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        let opts = parse_find_args(args);

        let search_path = if let Some(ref p) = opts.path {
            fs::join_path(ctx.cwd, p)
        } else {
            ctx.cwd.to_string()
        };

        let mut results = Vec::new();
        find_recursive(ctx.fs, &search_path, &opts, 0, &mut results).await?;

        let output = results.join("\n");
        let output = if output.is_empty() {
            output
        } else {
            format!("{output}\n")
        };

        Ok(CommandOutput::success(output))
    }
}

struct FindOptions {
    path: Option<String>,
    name_pattern: Option<String>,
    type_filter: Option<FileType>,
    max_depth: Option<usize>,
}

fn parse_find_args(args: &[String]) -> FindOptions {
    let mut opts = FindOptions {
        path: None,
        name_pattern: None,
        type_filter: None,
        max_depth: None,
    };

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "-name" => {
                i += 1;
                if i < args.len() {
                    opts.name_pattern = Some(args[i].clone());
                }
            }
            "-type" => {
                i += 1;
                if i < args.len() {
                    opts.type_filter = match args[i].as_str() {
                        "f" => Some(FileType::File),
                        "d" => Some(FileType::Directory),
                        _ => None,
                    };
                }
            }
            "-maxdepth" => {
                i += 1;
                if i < args.len() {
                    opts.max_depth = args[i].parse().ok();
                }
            }
            _ if !arg.starts_with('-') && opts.path.is_none() => {
                opts.path = Some(arg.clone());
            }
            _ => {}
        }
        i += 1;
    }

    opts
}

#[async_recursion::async_recursion]
async fn find_recursive(
    fs: &dyn IFileSystem,
    path: &str,
    opts: &FindOptions,
    depth: usize,
    results: &mut Vec<String>,
) -> Result<()> {
    // 检查深度限制
    if let Some(max) = opts.max_depth {
        if depth > max {
            return Ok(());
        }
    }

    let Ok(entries) = fs.read_dir(path).await else {
        return Ok(());
    };

    for entry in entries {
        let full_path = fs::join_path(path, &entry.name);

        // 类型过滤
        if let Some(type_filter) = opts.type_filter {
            if entry.file_type != type_filter {
                if entry.file_type.is_dir() {
                    // 即使不匹配类型，目录仍需递归
                    find_recursive(fs, &full_path, opts, depth + 1, results).await?;
                }
                continue;
            }
        }

        // 名称过滤
        if let Some(ref pattern) = opts.name_pattern {
            if matches_find_pattern(&entry.name, pattern) {
                results.push(full_path.clone());
            }
        } else {
            results.push(full_path.clone());
        }

        // 递归目录
        if entry.file_type.is_dir() {
            find_recursive(fs, &full_path, opts, depth + 1, results).await?;
        }
    }

    Ok(())
}

/// find -name 的匹配逻辑
fn matches_find_pattern(name: &str, pattern: &str) -> bool {
    // 移除引号
    let pattern = pattern.trim_matches('"').trim_matches('\'');

    if pattern == "*" {
        return true;
    }

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
submit_command!(FindCommand);
