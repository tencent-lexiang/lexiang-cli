//! fzf 命令: 对 stdin 进行模糊搜索
//!
//! 在虚拟 shell 中，fzf 没有交互式 UI，退化为模糊 grep:
//! - `find . | fzf -q readme` → 从输入中模糊匹配含 "readme" 的行
//! - `find . | fzf -f readme` → 同上 (filter 模式)
//! - `find . | fzf` → 无 query 时返回所有输入 (相当于 cat)
//!
//! 模糊匹配算法: 对每个输入行计算匹配分数，按分数降序输出。
//! 匹配规则:
//! - 每个 query 字符必须按顺序出现在目标中 (subsequence match)
//! - 连续匹配得分更高
//! - 匹配位置越靠前得分越高

use super::registry::{Command, CommandContext, CommandOutput};
use crate::submit_command;
use anyhow::Result;
use async_trait::async_trait;

#[derive(Default)]
pub struct FzfCommand;

#[async_trait]
impl Command for FzfCommand {
    fn name(&self) -> &str {
        "fzf"
    }

    async fn execute(
        &self,
        args: &[String],
        ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        let opts = parse_fzf_args(args);

        // 获取输入
        let input = match &ctx.stdin {
            Some(s) => s.clone(),
            None => {
                return Ok(CommandOutput::error(
                    "fzf: no input (pipe data to fzf)".to_string(),
                ))
            }
        };

        let lines: Vec<&str> = input.lines().collect();

        // 无 query 时返回所有输入
        if opts.query.is_empty() {
            let mut output = String::new();
            let limit = opts.limit.unwrap_or(lines.len());
            for line in lines.iter().take(limit) {
                output.push_str(line);
                output.push('\n');
            }
            return Ok(CommandOutput::success(output));
        }

        // 模糊匹配并排序
        let mut scored: Vec<(i64, &str)> = lines
            .iter()
            .filter_map(|line| fuzzy_score(line, &opts.query).map(|score| (score, *line)))
            .collect();

        // 按分数降序排序
        scored.sort_by(|a, b| b.0.cmp(&a.0));

        // 输出
        let mut output = String::new();
        let limit = opts.limit.unwrap_or(scored.len());
        for (_, line) in scored.iter().take(limit) {
            output.push_str(line);
            output.push('\n');
        }

        if output.is_empty() {
            Ok(CommandOutput {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 1,
            })
        } else {
            Ok(CommandOutput::success(output))
        }
    }
}

struct FzfOptions {
    query: String,
    limit: Option<usize>,
}

fn parse_fzf_args(args: &[String]) -> FzfOptions {
    let mut opts = FzfOptions {
        query: String::new(),
        limit: None,
    };

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        match arg.as_str() {
            "-q" | "--query" | "-f" | "--filter" => {
                i += 1;
                if i < args.len() {
                    opts.query = args[i].clone();
                }
            }
            "-n" | "--limit" => {
                i += 1;
                if i < args.len() {
                    opts.limit = args[i].parse().ok();
                }
            }
            _ if arg.starts_with("--query=") => {
                opts.query = arg.strip_prefix("--query=").unwrap_or("").to_string();
            }
            _ if arg.starts_with("--filter=") => {
                opts.query = arg.strip_prefix("--filter=").unwrap_or("").to_string();
            }
            // 位置参数作为 query
            _ if !arg.starts_with('-') && opts.query.is_empty() => {
                opts.query = arg.clone();
            }
            _ => {} // 忽略其他选项
        }

        i += 1;
    }

    opts
}

/// 模糊匹配评分
///
/// 返回 None 表示不匹配。
/// 返回 Some(score) 表示匹配，分数越高越好。
///
/// 算法:
/// 1. query 中的每个字符必须按顺序出现在 target 中 (子序列匹配)
/// 2. 连续匹配加分
/// 3. 匹配位置越靠前加分
/// 4. 大小写精确匹配加分
fn fuzzy_score(target: &str, query: &str) -> Option<i64> {
    let target_lower = target.to_lowercase();
    let query_lower = query.to_lowercase();

    let target_chars: Vec<char> = target_lower.chars().collect();
    let query_chars: Vec<char> = query_lower.chars().collect();

    if query_chars.is_empty() {
        return Some(0);
    }

    let mut score: i64 = 0;
    let mut target_idx = 0;
    let mut prev_match_idx: Option<usize> = None;

    for &qch in &query_chars {
        let mut found = false;
        while target_idx < target_chars.len() {
            if target_chars[target_idx] == qch {
                // 匹配!
                found = true;

                // 位置靠前加分 (max 50)
                let position_bonus = 50_i64.saturating_sub(target_idx as i64);
                score += position_bonus.max(0);

                // 连续匹配加分
                if let Some(prev) = prev_match_idx {
                    if target_idx == prev + 1 {
                        score += 30; // 连续匹配大加分
                    }
                }

                // 原始大小写完全匹配加分
                let orig_chars: Vec<char> = target.chars().collect();
                let orig_query: Vec<char> = query.chars().collect();
                if target_idx < orig_chars.len() {
                    let qi = query_chars.iter().position(|&c| c == qch).unwrap_or(0);
                    if qi < orig_query.len() && orig_chars[target_idx] == orig_query[qi] {
                        score += 5;
                    }
                }

                prev_match_idx = Some(target_idx);
                target_idx += 1;
                break;
            }
            target_idx += 1;
        }

        if !found {
            return None; // query 字符无法在 target 中找到
        }
    }

    Some(score)
}

// ─── 命令注册 ────────────────────────────────────────────
submit_command!(FzfCommand);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::commands::CommandContext;
    use crate::shell::fs::InMemoryFs;
    use crate::shell::interpreter::Environment;

    #[test]
    fn test_fuzzy_score_exact() {
        assert!(fuzzy_score("readme.md", "readme").is_some());
        assert!(fuzzy_score("README.md", "readme").is_some());
    }

    #[test]
    fn test_fuzzy_score_subsequence() {
        // "rmd" 应该匹配 "readme.md" (r...e...a...d...m...e...m...d)
        assert!(fuzzy_score("readme.md", "rmd").is_some());
    }

    #[test]
    fn test_fuzzy_score_no_match() {
        assert!(fuzzy_score("readme.md", "xyz").is_none());
    }

    #[test]
    fn test_fuzzy_score_ordering() {
        // "readme.md" 应该比 "src/readme.md" 分数高 (位置更靠前)
        let score1 = fuzzy_score("readme.md", "readme").unwrap();
        let score2 = fuzzy_score("src/very/deep/readme.md", "readme").unwrap();
        assert!(score1 > score2);
    }

    #[tokio::test]
    async fn test_fzf_filter() {
        let fs = InMemoryFs::new();
        let mut env = Environment::default();
        let input = "readme.md\nsrc/main.rs\ndocs/api.md\nCargo.toml\n";

        let mut ctx = CommandContext {
            fs: &fs,
            env: &mut env,
            stdin: Some(input.to_string()),
            cwd: "/",
        };

        let cmd = FzfCommand;
        let result = cmd
            .execute(&["-q".to_string(), "md".to_string()], &mut ctx)
            .await
            .unwrap();

        assert!(result.stdout.contains("readme.md"));
        assert!(result.stdout.contains("api.md"));
        assert!(!result.stdout.contains("Cargo.toml"));
    }

    #[tokio::test]
    async fn test_fzf_no_query() {
        let fs = InMemoryFs::new();
        let mut env = Environment::default();
        let input = "a\nb\nc\n";

        let mut ctx = CommandContext {
            fs: &fs,
            env: &mut env,
            stdin: Some(input.to_string()),
            cwd: "/",
        };

        let cmd = FzfCommand;
        let result = cmd.execute(&[], &mut ctx).await.unwrap();
        assert_eq!(result.stdout, "a\nb\nc\n");
    }
}
