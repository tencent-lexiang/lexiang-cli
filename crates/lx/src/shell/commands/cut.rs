//! cut 命令: 按列/字符/字段提取文本
//!
//! 对标 POSIX cut 标准参数:
//! - `-d DELIM`  — 指定字段分隔符 (默认 TAB)
//! - `-f LIST`   — 选择字段 (如 1, 1-3, 2,4)
//! - `-c LIST`   — 选择字符位置
//! - `-s`        — 仅输出包含分隔符的行 (配合 -f)
//! - `--output-delimiter=STR` — 输出分隔符 (默认与输入一致)

use super::registry::{Command, CommandContext, CommandOutput};
use crate::shell::fs;
use crate::submit_command;
use anyhow::Result;
use async_trait::async_trait;

#[derive(Default)]
pub struct CutCommand;

#[async_trait]
impl Command for CutCommand {
    fn name(&self) -> &str {
        "cut"
    }

    async fn execute(
        &self,
        args: &[String],
        ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        let opts = match parse_cut_args(args) {
            Ok(o) => o,
            Err(e) => return Ok(CommandOutput::error(format!("cut: {e}"))),
        };

        // 获取输入
        let content = if let Some(ref stdin) = ctx.stdin {
            stdin.clone()
        } else if let Some(ref file) = opts.file {
            let path = fs::join_path(ctx.cwd, file);
            match ctx.fs.read_file(&path).await {
                Ok(c) => c,
                Err(e) => return Ok(CommandOutput::error(format!("cut: {file}: {e}"))),
            }
        } else {
            return Ok(CommandOutput::error(
                "cut: you must specify a list of bytes, characters, or fields".to_string(),
            ));
        };

        let mut output = String::new();

        for line in content.lines() {
            match opts.mode {
                CutMode::Fields => {
                    let delim = &opts.delimiter;
                    let parts: Vec<&str> = line.split(delim.as_str()).collect();

                    // -s: 跳过不包含分隔符的行
                    if opts.only_delimited && !line.contains(delim.as_str()) {
                        continue;
                    }

                    let out_delim = opts.output_delimiter.as_deref().unwrap_or(delim.as_str());

                    let selected: Vec<String> = opts
                        .ranges
                        .iter()
                        .filter_map(|&(start, end)| {
                            let mut fields = Vec::new();
                            for i in start..=end {
                                if i >= 1 && (i as usize) <= parts.len() {
                                    fields.push(parts[i as usize - 1]);
                                }
                            }
                            if fields.is_empty() {
                                None
                            } else {
                                Some(fields.join(out_delim))
                            }
                        })
                        .collect();

                    if !selected.is_empty() {
                        output.push_str(&selected.join(out_delim));
                    }
                    output.push('\n');
                }
                CutMode::Characters => {
                    let chars: Vec<char> = line.chars().collect();
                    let mut selected = String::new();

                    for &(start, end) in &opts.ranges {
                        for i in start..=end {
                            if i >= 1 && (i as usize) <= chars.len() {
                                selected.push(chars[i as usize - 1]);
                            }
                        }
                    }

                    output.push_str(&selected);
                    output.push('\n');
                }
            }
        }

        Ok(CommandOutput::success(output))
    }
}

// ─── Types ───────────────────────────────────────────────

#[derive(Clone, Copy)]
enum CutMode {
    Fields,
    Characters,
}

struct CutOptions {
    mode: CutMode,
    delimiter: String,
    ranges: Vec<(i32, i32)>,
    only_delimited: bool,
    output_delimiter: Option<String>,
    file: Option<String>,
}

// ─── Argument parsing ────────────────────────────────────

fn parse_cut_args(args: &[String]) -> std::result::Result<CutOptions, String> {
    let mut opts = CutOptions {
        mode: CutMode::Fields,
        delimiter: "\t".to_string(),
        ranges: Vec::new(),
        only_delimited: false,
        output_delimiter: None,
        file: None,
    };

    let mut i = 0;
    let mut list_str: Option<String> = None;
    let mut mode_set = false;

    while i < args.len() {
        let arg = &args[i];

        if arg == "-d" {
            i += 1;
            if i < args.len() {
                opts.delimiter = args[i].clone();
            } else {
                return Err("option requires an argument -- 'd'".into());
            }
        } else if let Some(d) = arg.strip_prefix("-d") {
            opts.delimiter = d.to_string();
        } else if arg == "-f" {
            opts.mode = CutMode::Fields;
            mode_set = true;
            i += 1;
            if i < args.len() {
                list_str = Some(args[i].clone());
            } else {
                return Err("option requires an argument -- 'f'".into());
            }
        } else if let Some(f) = arg.strip_prefix("-f") {
            opts.mode = CutMode::Fields;
            mode_set = true;
            list_str = Some(f.to_string());
        } else if arg == "-c" {
            opts.mode = CutMode::Characters;
            mode_set = true;
            i += 1;
            if i < args.len() {
                list_str = Some(args[i].clone());
            } else {
                return Err("option requires an argument -- 'c'".into());
            }
        } else if let Some(c) = arg.strip_prefix("-c") {
            opts.mode = CutMode::Characters;
            mode_set = true;
            list_str = Some(c.to_string());
        } else if arg == "-s" || arg == "--only-delimited" {
            opts.only_delimited = true;
        } else if let Some(od) = arg.strip_prefix("--output-delimiter=") {
            opts.output_delimiter = Some(od.to_string());
        } else if !arg.starts_with('-') {
            opts.file = Some(arg.clone());
        }

        i += 1;
    }

    if !mode_set || list_str.is_none() {
        return Err(
            "you must specify a list of bytes, characters, or fields\nTry 'cut --help' for more information.".into(),
        );
    }

    opts.ranges = parse_range_list(&list_str.unwrap())?;

    Ok(opts)
}

/// 解析 "1,3-5,7" → [(1,1), (3,5), (7,7)]
fn parse_range_list(list: &str) -> std::result::Result<Vec<(i32, i32)>, String> {
    let mut ranges = Vec::new();

    for part in list.split(',') {
        let part = part.trim();
        if part.contains('-') {
            let mut split = part.splitn(2, '-');
            let start_str = split.next().unwrap_or("");
            let end_str = split.next().unwrap_or("");

            let start: i32 = if start_str.is_empty() {
                1
            } else {
                start_str
                    .parse()
                    .map_err(|_| format!("invalid range: '{part}'"))?
            };
            let end: i32 = if end_str.is_empty() {
                i32::MAX / 2 // open range: "3-" means 3 to end
            } else {
                end_str
                    .parse()
                    .map_err(|_| format!("invalid range: '{part}'"))?
            };

            if start > end {
                return Err(format!("invalid decreasing range: '{part}'"));
            }
            ranges.push((start, end));
        } else {
            let n: i32 = part
                .parse()
                .map_err(|_| format!("invalid field value: '{part}'"))?;
            ranges.push((n, n));
        }
    }

    Ok(ranges)
}

// ─── 命令注册 ────────────────────────────────────────────
submit_command!(CutCommand);
