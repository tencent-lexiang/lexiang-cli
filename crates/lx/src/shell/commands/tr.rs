//! tr 命令: 转换或删除字符
//!
//! 对标 POSIX tr 标准参数:
//! - `tr SET1 SET2`      — 将 SET1 中的字符替换为 SET2 中对应字符
//! - `tr -d SET1`        — 删除 SET1 中的字符
//! - `tr -s SET1`        — 压缩 SET1 中连续重复的字符
//! - `tr -c SET1 SET2`   — 补集: 替换不在 SET1 中的字符
//!
//! SET 语法:
//! - `a-z`       — 字符范围
//! - `A-Z`       — 大写范围
//! - `0-9`       — 数字范围
//! - `[:upper:]` — POSIX 字符类
//! - `[:lower:]`
//! - `[:digit:]`
//! - `[:alpha:]`
//! - `[:space:]`
//! - `\n`, `\t`  — 转义字符

use super::registry::{Command, CommandContext, CommandOutput};
use crate::submit_command;
use anyhow::Result;
use async_trait::async_trait;

#[derive(Default)]
pub struct TrCommand;

#[async_trait]
impl Command for TrCommand {
    fn name(&self) -> &str {
        "tr"
    }

    async fn execute(
        &self,
        args: &[String],
        ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        let opts = match parse_tr_args(args) {
            Ok(o) => o,
            Err(e) => return Ok(CommandOutput::error(format!("tr: {e}"))),
        };

        let input = ctx.stdin.clone().unwrap_or_default();
        let set1 = expand_set(&opts.set1);

        let output = if opts.delete {
            // -d: 删除 set1 中的字符
            if opts.complement {
                input.chars().filter(|c| set1.contains(c)).collect()
            } else {
                input.chars().filter(|c| !set1.contains(c)).collect()
            }
        } else if opts.squeeze && opts.set2.is_none() {
            // -s 不带 set2: 压缩 set1 中连续重复字符
            squeeze(&input, &set1, opts.complement)
        } else if let Some(ref s2) = opts.set2 {
            let set2 = expand_set(s2);
            let mut result: String = translate(&input, &set1, &set2, opts.complement);

            if opts.squeeze {
                result = squeeze(&result, &set2, false);
            }
            result
        } else {
            return Ok(CommandOutput::error(
                "tr: missing operand\nTry 'tr --help' for more information.".to_string(),
            ));
        };

        Ok(CommandOutput::success(output))
    }
}

// ─── Types ───────────────────────────────────────────────

struct TrOptions {
    set1: String,
    set2: Option<String>,
    delete: bool,
    squeeze: bool,
    complement: bool,
}

// ─── Argument parsing ────────────────────────────────────

fn parse_tr_args(args: &[String]) -> std::result::Result<TrOptions, String> {
    let mut opts = TrOptions {
        set1: String::new(),
        set2: None,
        delete: false,
        squeeze: false,
        complement: false,
    };

    let mut positional: Vec<String> = Vec::new();

    for arg in args {
        if arg.starts_with('-') && arg.len() > 1 && !arg.starts_with("--") {
            for ch in arg[1..].chars() {
                match ch {
                    'd' => opts.delete = true,
                    's' => opts.squeeze = true,
                    'c' | 'C' => opts.complement = true,
                    _ => return Err(format!("invalid option -- '{ch}'")),
                }
            }
        } else {
            positional.push(arg.clone());
        }
    }

    match positional.len() {
        0 => return Err("missing operand".into()),
        1 => {
            opts.set1 = positional[0].clone();
        }
        _ => {
            opts.set1 = positional[0].clone();
            opts.set2 = Some(positional[1].clone());
        }
    }

    Ok(opts)
}

// ─── Set expansion ───────────────────────────────────────

/// 展开字符集: "a-z" → ['a'..'z'], "[:upper:]" → ['A'..'Z'], etc.
fn expand_set(set: &str) -> Vec<char> {
    let mut chars = Vec::new();
    let set = unescape(set);
    let bytes: Vec<char> = set.chars().collect();
    let mut i = 0;

    while i < bytes.len() {
        // POSIX 字符类
        if i + 1 < bytes.len() && bytes[i] == '[' && bytes[i + 1] == ':' {
            if let Some(end) = set[i..].find(":]") {
                let class = &set[i + 2..i + end];
                chars.extend(expand_posix_class(class));
                i += end + 2;
                continue;
            }
        }

        // 范围: a-z
        if i + 2 < bytes.len() && bytes[i + 1] == '-' {
            let start = bytes[i];
            let end = bytes[i + 2];
            if start <= end {
                for c in start..=end {
                    chars.push(c);
                }
                i += 3;
                continue;
            }
        }

        chars.push(bytes[i]);
        i += 1;
    }

    chars
}

fn expand_posix_class(class: &str) -> Vec<char> {
    match class {
        "upper" => ('A'..='Z').collect(),
        "lower" => ('a'..='z').collect(),
        "digit" => ('0'..='9').collect(),
        "alpha" => {
            let mut v: Vec<char> = ('a'..='z').collect();
            v.extend('A'..='Z');
            v
        }
        "alnum" => {
            let mut v: Vec<char> = ('a'..='z').collect();
            v.extend('A'..='Z');
            v.extend('0'..='9');
            v
        }
        "space" => vec![' ', '\t', '\n', '\r', '\x0b', '\x0c'],
        "blank" => vec![' ', '\t'],
        "punct" => "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~".chars().collect(),
        _ => Vec::new(),
    }
}

fn unescape(s: &str) -> String {
    s.replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\r", "\r")
        .replace("\\\\", "\\")
}

// ─── Core operations ─────────────────────────────────────

/// 字符替换: 将 set1[i] → set2[i]
fn translate(input: &str, set1: &[char], set2: &[char], complement: bool) -> String {
    if set2.is_empty() {
        return input.to_string();
    }

    let last_set2 = *set2.last().unwrap();

    input
        .chars()
        .map(|c| {
            if complement {
                // -c: 替换不在 set1 中的字符
                if set1.contains(&c) {
                    c
                } else {
                    last_set2
                }
            } else if let Some(pos) = set1.iter().position(|&s| s == c) {
                *set2.get(pos).unwrap_or(&last_set2)
            } else {
                c
            }
        })
        .collect()
}

/// 压缩连续重复字符
fn squeeze(input: &str, set: &[char], complement: bool) -> String {
    let mut result = String::new();
    let mut prev: Option<char> = None;

    for c in input.chars() {
        let in_set = set.contains(&c);
        let should_squeeze = if complement { !in_set } else { in_set };

        if should_squeeze && prev == Some(c) {
            continue; // 压缩掉
        }

        result.push(c);
        prev = Some(c);
    }

    result
}

// ─── 命令注册 ────────────────────────────────────────────
submit_command!(TrCommand);
