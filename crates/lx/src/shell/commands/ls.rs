//! ls 命令: 列出目录内容
//!
//! 对标 POSIX / GNU ls 标准输出格式。
//!
//! 支持的选项:
//! - `-l` — 长格式 (permissions, links, owner, group, size, date, name)
//! - `-a` — 显示隐藏文件 (以 . 开头的)
//! - `-1` — 每行一个文件
//! - `-h` — 人类可读大小 (配合 -l)
//! - `-i` — 显示 inode (`entry_id`)
//! - `-S` — 按文件大小排序 (大到小)
//! - `-t` — 按修改时间排序 (新到旧)
//! - `-r` — 反转排序顺序

use super::registry::{Command, CommandContext, CommandOutput};
use crate::shell::fs::{self, DirEntry};
use crate::submit_command;
use anyhow::Result;
use async_trait::async_trait;

#[derive(Default)]
pub struct LsCommand;

#[async_trait]
impl Command for LsCommand {
    fn name(&self) -> &str {
        "ls"
    }

    async fn execute(
        &self,
        args: &[String],
        ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        let opts = parse_ls_args(args);

        let path = if let Some(ref p) = opts.path {
            fs::join_path(ctx.cwd, p)
        } else {
            ctx.cwd.to_string()
        };

        let mut entries = match ctx.fs.read_dir(&path).await {
            Ok(entries) => entries,
            Err(e) => {
                return Ok(CommandOutput::error(format!(
                    "ls: cannot access '{}': {}",
                    path, e
                )))
            }
        };

        // 过滤隐藏文件
        if !opts.all {
            entries.retain(|e| !e.name.starts_with('.'));
        }

        // 排序
        if opts.sort_by_size {
            entries.sort_by(|a, b| b.size.cmp(&a.size));
        } else if opts.sort_by_time {
            entries.sort_by(|a, b| {
                let ta = a.modified.unwrap_or(std::time::UNIX_EPOCH);
                let tb = b.modified.unwrap_or(std::time::UNIX_EPOCH);
                tb.cmp(&ta)
            });
        } else {
            // 默认按名称排序
            entries.sort_by(|a, b| a.name.cmp(&b.name));
        }

        if opts.reverse {
            entries.reverse();
        }

        let mut output = String::new();

        if opts.long {
            // 长格式: 先输出 total 行
            let total: u64 = entries.iter().map(|e| e.size.div_ceil(1024)).sum();
            output.push_str(&format!("total {}\n", total));

            // 计算列宽对齐
            let max_size_width = entries
                .iter()
                .map(|e| {
                    if opts.human_readable {
                        human_readable_size(e.size).len()
                    } else {
                        e.size.to_string().len()
                    }
                })
                .max()
                .unwrap_or(1);

            for entry in &entries {
                let perms = format_permissions(entry);
                let links = if entry.file_type.is_dir() { "2" } else { "1" };
                let owner = "user";
                let group = "staff";
                let size = if opts.human_readable {
                    human_readable_size(entry.size)
                } else {
                    entry.size.to_string()
                };
                let date = format_date(entry.modified);
                let name = format_name(entry);

                if opts.inode {
                    let inode = format_inode(entry);
                    output.push_str(&format!(
                        "{} {} {} {} {} {:>width$} {} {}\n",
                        inode,
                        perms,
                        links,
                        owner,
                        group,
                        size,
                        date,
                        name,
                        width = max_size_width
                    ));
                } else {
                    output.push_str(&format!(
                        "{} {} {} {} {:>width$} {} {}\n",
                        perms,
                        links,
                        owner,
                        group,
                        size,
                        date,
                        name,
                        width = max_size_width
                    ));
                }
            }
        } else if opts.one_per_line {
            for entry in &entries {
                if opts.inode {
                    output.push_str(&format!("{} {}\n", format_inode(entry), format_name(entry)));
                } else {
                    output.push_str(&format_name(entry));
                    output.push('\n');
                }
            }
        } else {
            // 默认: 空格分隔，一行显示
            let names: Vec<String> = if opts.inode {
                entries
                    .iter()
                    .map(|e| format!("{} {}", format_inode(e), format_name(e)))
                    .collect()
            } else {
                entries.iter().map(format_name).collect()
            };
            if !names.is_empty() {
                output.push_str(&names.join("  "));
                output.push('\n');
            }
        }

        Ok(CommandOutput::success(output))
    }
}

// ─── Options ─────────────────────────────────────────────

struct LsOptions {
    path: Option<String>,
    long: bool,
    all: bool,
    one_per_line: bool,
    human_readable: bool,
    inode: bool,
    sort_by_size: bool,
    sort_by_time: bool,
    reverse: bool,
}

fn parse_ls_args(args: &[String]) -> LsOptions {
    let mut opts = LsOptions {
        path: None,
        long: false,
        all: false,
        one_per_line: false,
        human_readable: false,
        inode: false,
        sort_by_size: false,
        sort_by_time: false,
        reverse: false,
    };

    for arg in args {
        if arg.starts_with('-') && !arg.starts_with("--") {
            for ch in arg[1..].chars() {
                match ch {
                    'l' => opts.long = true,
                    'a' => opts.all = true,
                    '1' => opts.one_per_line = true,
                    'h' => opts.human_readable = true,
                    'i' => opts.inode = true,
                    'S' => opts.sort_by_size = true,
                    't' => opts.sort_by_time = true,
                    'r' => opts.reverse = true,
                    _ => {}
                }
            }
        } else if arg == "--all" {
            opts.all = true;
        } else if arg == "--human-readable" {
            opts.human_readable = true;
        } else if arg == "--inode" {
            opts.inode = true;
        } else if arg == "--reverse" {
            opts.reverse = true;
        } else {
            opts.path = Some(arg.clone());
        }
    }

    // -l 暗含 -1
    if opts.long {
        opts.one_per_line = true;
    }

    opts
}

// ─── Formatting helpers ──────────────────────────────────

/// 标准 ls -l 权限格式: drwxr-xr-x / -rw-r--r--
fn format_permissions(entry: &DirEntry) -> String {
    if entry.file_type.is_dir() {
        "drwxr-xr-x".to_string()
    } else {
        "-rw-r--r--".to_string()
    }
}

/// 格式化文件名 (目录加 /)
fn format_name(entry: &DirEntry) -> String {
    if entry.file_type.is_dir() {
        format!("{}/", entry.name)
    } else {
        entry.name.clone()
    }
}

/// 格式化 inode (使用 `entry_id` 或序号)
fn format_inode(entry: &DirEntry) -> String {
    if let Some(ref meta) = entry.metadata {
        if let Some(ref id) = meta.entry_id {
            // 取 entry_id 末尾 8 位做短 inode
            let short = if id.len() > 8 {
                &id[id.len() - 8..]
            } else {
                id
            };
            return short.to_string();
        }
    }
    "0".to_string()
}

/// 人类可读大小: 1024 → 1.0K, 1048576 → 1.0M
fn human_readable_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes}")
    } else if bytes < 1024 * 1024 {
        format!("{:.1}K", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1}M", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1}G", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// 标准 ls -l 日期格式: "Jan  5 12:34" 或 "Jan  5  2024" (超过 6 个月)
fn format_date(time: Option<std::time::SystemTime>) -> String {
    match time {
        Some(t) => {
            let duration = t
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::from_secs(0));
            let secs = duration.as_secs() as i64;

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::from_secs(0))
                .as_secs() as i64;

            let months = [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ];

            // 简化的日期计算（不依赖 chrono）
            let (year, month, day, hour, minute) = timestamp_to_date(secs);
            let mon_str = months.get(month as usize).unwrap_or(&"???");
            let age = now - secs;

            if age.abs() > 180 * 86400 {
                // 超过 ~6 个月: 显示年份
                format!("{} {:>2}  {}", mon_str, day, year)
            } else {
                // 最近: 显示时间
                format!("{} {:>2} {:02}:{:02}", mon_str, day, hour, minute)
            }
        }
        None => "            ".to_string(), // 12 char placeholder
    }
}

/// 简化的 Unix 时间戳 → (year, month, day, hour, minute) 转换
fn timestamp_to_date(timestamp: i64) -> (i32, i32, i32, i32, i32) {
    // 简化实现：使用标准库的 UNIX_EPOCH + Duration
    let secs_per_day: i64 = 86400;
    let days = timestamp / secs_per_day;
    let day_secs = (timestamp % secs_per_day) as i32;
    let hour = day_secs / 3600;
    let minute = (day_secs % 3600) / 60;

    // 从 1970-01-01 算天数 → 年月日
    let mut remaining_days = days as i32;
    let mut year = 1970;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let days_in_months = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 0;
    for (m, &dim) in days_in_months.iter().enumerate() {
        if remaining_days < dim {
            month = m as i32;
            break;
        }
        remaining_days -= dim;
    }

    let day = remaining_days + 1;
    (year, month, day, hour, minute)
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

// ─── 命令注册 ────────────────────────────────────────────
submit_command!(LsCommand);
