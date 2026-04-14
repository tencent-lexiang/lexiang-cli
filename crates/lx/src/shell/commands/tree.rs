//! tree 命令: 以树状图显示目录结构
//!
//! 支持的选项:
//! - `-L N` / `--depth N` — 最大显示深度
//! - `-d` — 只显示目录
//! - `--noreport` — 不显示统计信息

use super::registry::{Command, CommandContext, CommandOutput};
use crate::shell::fs::{self, IFileSystem};
use crate::submit_command;
use anyhow::Result;
use async_trait::async_trait;

#[derive(Default)]
pub struct TreeCommand;

#[async_trait]
impl Command for TreeCommand {
    fn name(&self) -> &str {
        "tree"
    }

    async fn execute(
        &self,
        args: &[String],
        ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        let opts = parse_tree_args(args);

        let path = if let Some(ref p) = opts.path {
            fs::join_path(ctx.cwd, p)
        } else {
            ctx.cwd.to_string()
        };

        let mut output = String::new();
        output.push_str(&fs::basename(&path));
        output.push('\n');

        let mut stats = TreeStats::default();
        tree_recursive(ctx.fs, &path, &opts, "", 0, &mut output, &mut stats).await?;

        if !opts.no_report {
            output.push_str(&format!(
                "\n{} directories, {} files\n",
                stats.dirs, stats.files
            ));
        }

        Ok(CommandOutput::success(output))
    }
}

struct TreeOptions {
    path: Option<String>,
    max_depth: Option<usize>,
    dirs_only: bool,
    no_report: bool,
}

#[derive(Default)]
struct TreeStats {
    dirs: usize,
    files: usize,
}

fn parse_tree_args(args: &[String]) -> TreeOptions {
    let mut opts = TreeOptions {
        path: None,
        max_depth: None,
        dirs_only: false,
        no_report: false,
    };

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "-L" | "--depth" => {
                i += 1;
                if i < args.len() {
                    opts.max_depth = args[i].parse().ok();
                }
            }
            "-d" => opts.dirs_only = true,
            "--noreport" => opts.no_report = true,
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
async fn tree_recursive(
    fs: &dyn IFileSystem,
    path: &str,
    opts: &TreeOptions,
    prefix: &str,
    depth: usize,
    output: &mut String,
    stats: &mut TreeStats,
) -> Result<()> {
    if let Some(max) = opts.max_depth {
        if depth >= max {
            return Ok(());
        }
    }

    let Ok(mut entries) = fs.read_dir(path).await else {
        return Ok(());
    };

    if opts.dirs_only {
        entries.retain(|e| e.file_type.is_dir());
    }

    let count = entries.len();

    for (i, entry) in entries.iter().enumerate() {
        let is_last = i == count - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let child_prefix = if is_last { "    " } else { "│   " };

        let name = if entry.file_type.is_dir() {
            stats.dirs += 1;
            format!("{}/", entry.name)
        } else {
            stats.files += 1;
            entry.name.clone()
        };

        output.push_str(&format!("{prefix}{connector}{name}\n"));

        if entry.file_type.is_dir() {
            let child_path = fs::join_path(path, &entry.name);
            let new_prefix = format!("{prefix}{child_prefix}");
            tree_recursive(fs, &child_path, opts, &new_prefix, depth + 1, output, stats).await?;
        }
    }

    Ok(())
}

// ─── 命令注册 ────────────────────────────────────────────
submit_command!(TreeCommand);
