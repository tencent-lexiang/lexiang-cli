//! awk 命令 (常用子集)
//!
//! 支持 AI Agent 最高频的 awk 用法，对标标准参数:
//!
//! - `-F SEP`                 — 字段分隔符 (默认空白)
//! - `'{print $1, $3}'`       — 字段提取
//! - `'{print $0}'`           — 整行
//! - `'{print NR, $0}'`       — 行号
//! - `'/pattern/ {print $0}'` — 正则过滤
//! - `'BEGIN {} {} END {}'`   — BEGIN/END 块
//! - `NR==N`                  — 指定行号
//! - `NF`                     — 字段数 (用于 print)
//!
//! 不支持的高级特性: 变量赋值、数组、多规则、自定义函数等。

use super::registry::{Command, CommandContext, CommandOutput};
use crate::shell::fs;
use crate::submit_command;
use anyhow::Result;
use async_trait::async_trait;

#[derive(Default)]
pub struct AwkCommand;

#[async_trait]
impl Command for AwkCommand {
    fn name(&self) -> &str {
        "awk"
    }

    async fn execute(
        &self,
        args: &[String],
        ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        let opts = match parse_awk_args(args) {
            Ok(o) => o,
            Err(e) => return Ok(CommandOutput::error(format!("awk: {e}"))),
        };

        // 获取输入
        let content = if let Some(ref stdin) = ctx.stdin {
            stdin.clone()
        } else if let Some(ref file) = opts.file {
            let path = fs::join_path(ctx.cwd, file);
            match ctx.fs.read_file(&path).await {
                Ok(c) => c,
                Err(e) => return Ok(CommandOutput::error(format!("awk: {file}: {e}"))),
            }
        } else {
            return Ok(CommandOutput::error(
                "awk: no input file or pipe".to_string(),
            ));
        };

        let mut output = String::new();

        // BEGIN 块
        if let Some(ref begin) = opts.begin_action {
            if let Some(line) = execute_action(begin, "", &[], 0, 0, &opts) {
                output.push_str(&line);
                output.push('\n');
            }
        }

        let lines: Vec<&str> = content.lines().collect();
        let nr_total = lines.len();

        for (idx, line) in lines.iter().enumerate() {
            let nr = idx + 1;

            // 按分隔符拆分字段
            let fields: Vec<&str> = if opts.field_sep == " " {
                line.split_whitespace().collect()
            } else {
                line.split(&opts.field_sep).collect()
            };

            // 条件检查
            let matched = match &opts.condition {
                Condition::None => true,
                Condition::Pattern(pat) => {
                    if opts.pattern_ignore_case {
                        line.to_lowercase().contains(&pat.to_lowercase())
                    } else {
                        line.contains(pat.as_str())
                    }
                }
                Condition::NrEquals(n) => nr == *n,
                Condition::NrRange(start, end) => nr >= *start && nr <= *end,
            };

            if !matched {
                continue;
            }

            // 执行动作
            if let Some(ref action) = opts.action {
                if let Some(result) = execute_action(action, line, &fields, nr, nr_total, &opts) {
                    output.push_str(&result);
                    output.push('\n');
                }
            } else {
                // 默认: 打印整行
                output.push_str(line);
                output.push('\n');
            }
        }

        // END 块
        if let Some(ref end_action) = opts.end_action {
            if let Some(line) = execute_action(end_action, "", &[], nr_total, nr_total, &opts) {
                output.push_str(&line);
                output.push('\n');
            }
        }

        Ok(CommandOutput::success(output))
    }
}

// ─── Types ───────────────────────────────────────────────

#[derive(Debug)]
#[allow(dead_code)]
enum Condition {
    None,
    Pattern(String),
    NrEquals(usize),
    NrRange(usize, usize),
}

#[derive(Debug)]
enum Action {
    /// print $1, $3 — 输出指定字段 (用 OFS 分隔)
    PrintFields(Vec<FieldRef>),
    /// 原始 print 表达式 (回退: 原样输出)
    PrintRaw(String),
}

#[derive(Debug)]
enum FieldRef {
    /// $0, $1, $2, ...
    Dollar(usize),
    /// NR
    Nr,
    /// NF
    Nf,
    /// 字面量字符串 "text"
    Literal(String),
}

struct AwkOptions {
    field_sep: String,
    condition: Condition,
    action: Option<Action>,
    begin_action: Option<Action>,
    end_action: Option<Action>,
    output_sep: String,
    file: Option<String>,
    pattern_ignore_case: bool,
}

// ─── Argument parsing ────────────────────────────────────

fn parse_awk_args(args: &[String]) -> std::result::Result<AwkOptions, String> {
    let mut opts = AwkOptions {
        field_sep: " ".to_string(),
        condition: Condition::None,
        action: None,
        begin_action: None,
        end_action: None,
        output_sep: " ".to_string(),
        file: None,
        pattern_ignore_case: false,
    };

    let mut i = 0;
    let mut program: Option<String> = None;

    while i < args.len() {
        let arg = &args[i];

        if arg == "-F" {
            i += 1;
            if i < args.len() {
                opts.field_sep = args[i].clone();
            } else {
                return Err("option requires an argument -- 'F'".into());
            }
        } else if let Some(sep) = arg.strip_prefix("-F") {
            opts.field_sep = sep.to_string();
        } else if arg == "-v" {
            // -v OFS=, 等简单变量赋值
            i += 1;
            if i < args.len() {
                let assignment = &args[i];
                if let Some(val) = assignment.strip_prefix("OFS=") {
                    opts.output_sep = val.to_string();
                }
                // 其他变量暂不支持
            }
        } else if !arg.starts_with('-') {
            if program.is_none() {
                program = Some(arg.clone());
            } else {
                opts.file = Some(arg.clone());
            }
        }

        i += 1;
    }

    if let Some(prog) = program {
        parse_awk_program(&prog, &mut opts)?;
    } else {
        return Err("no program text".into());
    }

    Ok(opts)
}

/// 解析 awk 程序文本: '/pattern/ {print $1}', 'NR==3', '{print $1, $2}', etc.
fn parse_awk_program(prog: &str, opts: &mut AwkOptions) -> std::result::Result<(), String> {
    let prog = prog.trim();

    // BEGIN{...} ... END{...} 模式
    let mut remaining = prog;

    // 提取 BEGIN 块
    if let Some(begin_start) = remaining.find("BEGIN") {
        let after_begin = &remaining[begin_start + 5..].trim_start();
        if after_begin.starts_with('{') {
            if let Some(end) = find_matching_brace(after_begin) {
                let begin_body = &after_begin[1..end];
                opts.begin_action = Some(parse_action_body(begin_body));
                remaining = after_begin[end + 1..].trim();
            }
        }
    }

    // 提取 END 块
    if let Some(end_start) = remaining.find("END") {
        let before_end = &remaining[..end_start].trim();
        let after_end = &remaining[end_start + 3..].trim_start();
        if after_end.starts_with('{') {
            if let Some(end) = find_matching_brace(after_end) {
                let end_body = &after_end[1..end];
                opts.end_action = Some(parse_action_body(end_body));
                remaining = before_end;
            }
        }
    }

    let remaining = remaining.trim();
    if remaining.is_empty() {
        return Ok(());
    }

    // /pattern/ {action} 模式
    if let Some(stripped) = remaining.strip_prefix('/') {
        if let Some(end_slash) = stripped.find('/') {
            let pattern = &stripped[..end_slash];
            opts.condition = Condition::Pattern(pattern.to_string());
            let rest = stripped[end_slash + 1..].trim();
            if rest.starts_with('{') && rest.ends_with('}') {
                let body = &rest[1..rest.len() - 1];
                opts.action = Some(parse_action_body(body));
            } else if rest.is_empty() {
                // /pattern/ 不带 action → 默认 print $0
                opts.action = None; // 执行时默认 print 整行
            }
            return Ok(());
        }
    }

    // NR==N 或 NR>=N && NR<=M
    if let Some(nr_val) = remaining.strip_prefix("NR==") {
        let n: usize = nr_val
            .trim()
            .parse()
            .map_err(|_| "invalid NR value".to_string())?;
        opts.condition = Condition::NrEquals(n);
        return Ok(());
    }

    // {action} 纯动作
    if remaining.starts_with('{') && remaining.ends_with('}') {
        let body = &remaining[1..remaining.len() - 1];
        opts.action = Some(parse_action_body(body));
        return Ok(());
    }

    // 回退: 当作 pattern
    opts.condition = Condition::Pattern(remaining.to_string());
    Ok(())
}

/// 解析 action 体: "print $1, $3" → `Action::PrintFields`
fn parse_action_body(body: &str) -> Action {
    let body = body.trim();

    if let Some(print_expr) = body
        .strip_prefix("print ")
        .or_else(|| body.strip_prefix("print\t"))
    {
        let fields = parse_print_fields(print_expr.trim());
        Action::PrintFields(fields)
    } else if body == "print" {
        // 无参 print → $0
        Action::PrintFields(vec![FieldRef::Dollar(0)])
    } else {
        Action::PrintRaw(body.to_string())
    }
}

/// 解析 print 的参数列表: "$1, $3, NR, \"text\""
fn parse_print_fields(expr: &str) -> Vec<FieldRef> {
    let mut fields = Vec::new();

    // 按逗号分割（简化：不处理嵌套引号中的逗号）
    for part in expr.split(',') {
        let part = part.trim();
        if let Some(n_str) = part.strip_prefix('$') {
            if let Ok(n) = n_str.parse::<usize>() {
                fields.push(FieldRef::Dollar(n));
            }
        } else if part == "NR" {
            fields.push(FieldRef::Nr);
        } else if part == "NF" {
            fields.push(FieldRef::Nf);
        } else if (part.starts_with('"') && part.ends_with('"'))
            || (part.starts_with('\'') && part.ends_with('\''))
        {
            fields.push(FieldRef::Literal(part[1..part.len() - 1].to_string()));
        } else if !part.is_empty() {
            // 当作字面量
            fields.push(FieldRef::Literal(part.to_string()));
        }
    }

    if fields.is_empty() {
        fields.push(FieldRef::Dollar(0));
    }

    fields
}

/// 找到匹配的右大括号位置
fn find_matching_brace(s: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, ch) in s.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

// ─── Execution ───────────────────────────────────────────

fn execute_action(
    action: &Action,
    line: &str,
    fields: &[&str],
    nr: usize,
    _nr_total: usize,
    opts: &AwkOptions,
) -> Option<String> {
    match action {
        Action::PrintFields(refs) => {
            let parts: Vec<String> = refs
                .iter()
                .map(|r| match r {
                    FieldRef::Dollar(0) => line.to_string(),
                    FieldRef::Dollar(n) => (*fields.get(n - 1).unwrap_or(&"")).to_string(),
                    FieldRef::Nr => nr.to_string(),
                    FieldRef::Nf => fields.len().to_string(),
                    FieldRef::Literal(s) => s.clone(),
                })
                .collect();
            Some(parts.join(&opts.output_sep))
        }
        Action::PrintRaw(s) => Some(s.clone()),
    }
}

// ─── 命令注册 ────────────────────────────────────────────
submit_command!(AwkCommand);
