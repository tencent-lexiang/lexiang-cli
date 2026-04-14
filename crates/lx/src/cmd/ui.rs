#![allow(dead_code)]
//! 统一 TUI 输出控制模块
//!
//! **设计原则**: 业务代码不应直接使用 `println!`，所有终端输出都通过本模块收口。
//!
//! - 基础原语: `info`, `success`, `warn`, `error`, `dim`, `bold`, `line`, `blank`
//! - 进度工具: `spinner`, `progress_bar`
//! - Git 操作: `print_header`, `print_status`, `print_log`, `print_commit_result`,
//!   `print_push_stats`, `print_pull_stats`, `print_dry_run_item`, `print_dry_run_complete`
//! - 列表显示: `print_file_list`, `print_worktree_list`

use console::Style;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

// ═══════════════════════════════════════════════════════════
//  样式
// ═══════════════════════════════════════════════════════════

pub struct Styles {
    pub success: Style,
    pub error: Style,
    pub warn: Style,
    pub info: Style,
    pub dim: Style,
    pub bold: Style,
    pub cyan: Style,
}

impl Default for Styles {
    fn default() -> Self {
        Self::new()
    }
}

impl Styles {
    pub fn new() -> Self {
        Self {
            success: Style::new().green().bold(),
            error: Style::new().red().bold(),
            warn: Style::new().yellow().bold(),
            info: Style::new().cyan(),
            dim: Style::new().dim(),
            bold: Style::new().bold(),
            cyan: Style::new().cyan(),
        }
    }
}

// ═══════════════════════════════════════════════════════════
//  基础输出原语 —— 所有终端输出的唯一出口
// ═══════════════════════════════════════════════════════════

/// 普通信息
pub fn info(msg: &str) {
    let s = Styles::new();
    println!("{}", s.info.apply_to(msg));
}

/// 成功信息（绿色加粗）
pub fn success(msg: &str) {
    let s = Styles::new();
    println!("{}", s.success.apply_to(msg));
}

/// 警告信息（黄色加粗）
pub fn warn(msg: &str) {
    let s = Styles::new();
    println!("{}", s.warn.apply_to(msg));
}

/// 错误信息（红色加粗，输出到 stderr）
pub fn error(msg: &str) {
    let s = Styles::new();
    eprintln!("{}", s.error.apply_to(msg));
}

/// 次要/灰色信息
pub fn dim(msg: &str) {
    let s = Styles::new();
    println!("{}", s.dim.apply_to(msg));
}

/// 粗体信息
pub fn bold(msg: &str) {
    let s = Styles::new();
    println!("{}", s.bold.apply_to(msg));
}

/// 输出一行纯文本（不带样式，用于确实需要纯文本的场合如 JSON 输出）
pub fn line(msg: &str) {
    println!("{}", msg);
}

/// 输出交互式提示（不带换行，用于等待用户输入前的提示文本）
pub fn prompt(msg: &str) {
    use std::io::Write;
    let s = Styles::new();
    print!("{}", s.warn.apply_to(msg));
    let _ = std::io::stdout().flush();
}

/// 输出空行
pub fn blank() {
    println!();
}

/// 输出一行，前面有指定层级的缩进
pub fn indented(indent: usize, msg: &str) {
    println!("{:indent$}{}", "", msg, indent = indent * 2);
}

// ═══════════════════════════════════════════════════════════
//  复合输出 —— 用于常见的 key-value / 带前缀行
// ═══════════════════════════════════════════════════════════

/// 输出 "Key: value" 格式，key 粗体
pub fn kv(key: &str, value: &str) {
    let s = Styles::new();
    println!("{} {}", s.bold.apply_to(format!("{}:", key)), value);
}

/// 输出一行带有状态标记前缀的文件路径
///   marker: "M", "D", "?", "+", "A" 等
///   color:  "warn", "error", "dim", "success", "info"
///   path:   文件路径
pub fn status_line(marker: &str, color: &str, path: &str) {
    let s = Styles::new();
    let styled_marker = match color {
        "success" => s.success.apply_to(marker),
        "error" => s.error.apply_to(marker),
        "warn" => s.warn.apply_to(marker),
        "info" => s.info.apply_to(marker),
        _ => s.dim.apply_to(marker),
    };
    println!("    {} {}", styled_marker, path);
}

/// 输出章节标题（如 "Changes not staged for commit:"）
pub fn section(title: &str) {
    let s = Styles::new();
    blank();
    println!("{}:", s.bold.apply_to(title));
}

/// 输出一行 hint（缩进的灰色提示）
pub fn hint(msg: &str) {
    let s = Styles::new();
    println!("  {}", s.dim.apply_to(msg));
}

// ═══════════════════════════════════════════════════════════
//  进度条工具
// ═══════════════════════════════════════════════════════════

/// 创建一个已知总量的进度条（用于 push 大量文件时）
pub fn progress_bar(total: u64, message: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} [{bar:30.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("━╸─"),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(120));
    pb
}

/// 创建一个 spinner（用于不确定总量的操作，如 clone/pull）
pub fn spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

// ═══════════════════════════════════════════════════════════
//  Git 操作专用输出
// ═══════════════════════════════════════════════════════════

/// 打印操作头部: "Pushing to `SpaceName` (`space_id`)"
pub fn print_header(action: &str, name: &str, detail: &str) {
    let s = Styles::new();
    println!(
        "{} {} {}",
        s.bold.apply_to(action),
        s.cyan.apply_to(name),
        s.dim.apply_to(format!("({})", detail))
    );
}

/// 打印分支状态行
pub fn print_branch_line(branch: &str, space_name: &str, space_id: &str) {
    let s = Styles::new();
    println!(
        "On branch {} · {} {}",
        s.bold.apply_to(branch),
        s.cyan.apply_to(space_name),
        s.dim.apply_to(format!("({})", space_id))
    );
}

/// 打印 commit 结果: "[master abc1234] message"
pub fn print_commit_result(branch: &str, short_hash: &str, message: &str) {
    let s = Styles::new();
    println!(
        "[{} {}] {}",
        s.bold.apply_to(branch),
        s.warn.apply_to(short_hash),
        message
    );
}

/// 打印 log 条目:  hash + message (第一行)  author · date (第二行)
pub fn print_log_entry(short_hash: &str, message: &str, author: &str, date: &str) {
    let s = Styles::new();
    println!(
        "{} {}",
        s.warn.apply_to(short_hash),
        s.bold.apply_to(message.trim())
    );
    println!("  {} · {}", s.dim.apply_to(author), s.dim.apply_to(date));
}

/// 打印 git status 的完整文件列表
pub struct StatusOutput {
    pub staged: Vec<String>,
    pub modified: Vec<String>,
    pub deleted: Vec<String>,
    pub untracked: Vec<String>,
}

pub fn print_status(status: &StatusOutput) {
    if status.staged.is_empty()
        && status.modified.is_empty()
        && status.deleted.is_empty()
        && status.untracked.is_empty()
    {
        dim("nothing to commit, working tree clean");
        return;
    }

    if !status.modified.is_empty() {
        section("Changes not staged for commit");
        hint("(use \"lx git add <file>...\" to update what will be committed)");
        for f in &status.modified {
            status_line("M", "warn", &format_status_file(f, true));
        }
    }
    if !status.deleted.is_empty() {
        section("Deleted files");
        for f in &status.deleted {
            status_line("D", "error", f);
        }
    }
    if !status.untracked.is_empty() {
        section("Untracked files");
        hint("(use \"lx git add <file>...\" to include in what will be committed)");
        for f in &status.untracked {
            status_line("?", "dim", &format_status_file(f, false));
        }
    }
}

/// 打印 diff 文件列表
pub fn print_diff_list(modified: &[String], untracked: &[String], deleted: &[String]) {
    let s = Styles::new();
    if modified.is_empty() && untracked.is_empty() && deleted.is_empty() {
        dim("No changes.");
        return;
    }
    for f in modified {
        println!("{}  {}", s.warn.apply_to("M "), f);
    }
    for f in untracked {
        println!("{}  {}", s.dim.apply_to("??"), f);
    }
    for f in deleted {
        println!("{}  {}", s.error.apply_to("D "), f);
    }
}

/// 打印 remote diff (local vs remote) 的比较结果
pub fn print_remote_diff(added: &[String], modified: &[String], deleted: &[String]) {
    let s = Styles::new();
    blank();
    if added.is_empty() && modified.is_empty() && deleted.is_empty() {
        dim("No differences with remote.");
        return;
    }
    if !added.is_empty() {
        println!(
            "{}:",
            s.bold.apply_to(format!("New locally ({})", added.len()))
        );
        for f in added {
            println!("  {} {}", s.success.apply_to("+"), f);
        }
    }
    if !modified.is_empty() {
        println!(
            "{}:",
            s.bold.apply_to(format!("Modified ({})", modified.len()))
        );
        for f in modified {
            println!("  {} {}", s.warn.apply_to("M"), f);
        }
    }
    if !deleted.is_empty() {
        println!(
            "{}:",
            s.bold
                .apply_to(format!("Deleted locally ({})", deleted.len()))
        );
        for f in deleted {
            println!("  {} {}", s.error.apply_to("-"), f);
        }
    }
}

/// 打印 diff header (用于 full diff 输出)
pub fn print_diff_header(label_a: &str, label_b: &str, path: &str) {
    let s = Styles::new();
    println!(
        "\n{} {}",
        s.error.apply_to(format!("--- {}:", label_a)),
        path
    );
    println!(
        "{} {}",
        s.success.apply_to(format!("+++ {}:", label_b)),
        path
    );
}

/// 打印 diff 行 (+ 或 -)
pub fn print_diff_line(prefix: &str, content: &str) {
    let s = Styles::new();
    match prefix {
        "-" => println!("{}{}", s.error.apply_to("-"), content),
        "+" => println!("{}{}", s.success.apply_to("+"), content),
        _ => println!(" {}", content),
    }
}

/// 打印 reset 结果
pub fn print_reset_result(commit: &str, hard: bool) {
    let s = Styles::new();
    if hard {
        println!(
            "{} {} {}",
            s.success.apply_to("HEAD is now at"),
            s.warn.apply_to(commit),
            s.dim.apply_to("(hard reset)")
        );
    } else {
        println!("{}:", s.bold.apply_to("Unstaged changes after reset"));
    }
}

/// 打印 git add 结果
pub fn print_add_result(pathspec: &str, modified: &[String], untracked: &[String]) {
    let s = Styles::new();
    if pathspec == "." {
        println!("{}", s.success.apply_to("Changes staged for commit."));
        hint("(use \"lx git commit -m <message>\" to commit)");
    } else {
        println!("{} {}", s.success.apply_to("Staged:"), pathspec);
    }

    if !untracked.is_empty() {
        section("New files");
        for f in untracked {
            println!(
                "  {} {}",
                s.success.apply_to("+"),
                format_status_file(f, false)
            );
        }
    }
    if !modified.is_empty() {
        section("Modified");
        for f in modified {
            println!("  {} {}", s.warn.apply_to("M"), format_status_file(f, true));
        }
    }
}

/// 打印 remote -v 结果
pub fn print_remote(space_id: &str, verbose: bool) {
    let s = Styles::new();
    if verbose {
        let url = format!("https://lexiangla.com/spaces/{}", space_id);
        println!(
            "{}\t{} {}",
            s.bold.apply_to("origin"),
            s.cyan.apply_to(&url),
            s.dim.apply_to("(fetch)")
        );
        println!(
            "{}\t{} {}",
            s.bold.apply_to("origin"),
            s.cyan.apply_to(&url),
            s.dim.apply_to("(push)")
        );
    } else {
        println!("{}", s.bold.apply_to("origin"));
    }
}

// ═══════════════════════════════════════════════════════════
//  Push / Pull 统计输出
// ═══════════════════════════════════════════════════════════

/// 打印推送统计结果（含文件路径明细）
pub fn print_push_stats(
    created: usize,
    updated: usize,
    deleted: usize,
    created_paths: &[String],
    updated_paths: &[String],
    deleted_paths: &[String],
    errors: &[String],
) {
    let s = Styles::new();
    blank();

    let total_success = created + updated + deleted;
    if total_success > 0 {
        println!("{} {} 个文件:", s.success.apply_to("✓"), total_success);
        for p in created_paths {
            println!("  {} {}", s.success.apply_to("+"), p);
        }
        for p in updated_paths {
            println!("  {} {}", s.info.apply_to("~"), p);
        }
        for p in deleted_paths {
            println!("  {} {}", s.dim.apply_to("-"), p);
        }
    }

    if !errors.is_empty() {
        blank();
        println!("{} {} 个文件失败:", s.error.apply_to("✗"), errors.len());
        for e in errors {
            println!("  {} {}", s.error.apply_to("×"), e);
        }
    }

    if total_success == 0 && errors.is_empty() {
        dim("没有需要处理的变更。");
    }
}

/// 打印 pull 统计结果
pub fn print_pull_stats(folders: usize, pages: usize, files: usize, errors: &[String]) {
    let s = Styles::new();
    blank();
    println!(
        "{}  {} 个目录, {} 个页面, {} 个文件",
        s.success.apply_to("✓"),
        folders,
        pages,
        files,
    );

    if !errors.is_empty() {
        blank();
        println!("{} {} 个错误:", s.warn.apply_to("⚠"), errors.len());
        for e in errors {
            println!("  {} {}", s.error.apply_to("×"), e);
        }
    }
}

/// 打印 commit ID 行: "Committed: abc12345"
pub fn print_committed(commit_id: &str) {
    let s = Styles::new();
    println!("{} {}", s.dim.apply_to("Committed:"), commit_id);
}

// ═══════════════════════════════════════════════════════════
//  Dry-run 输出
// ═══════════════════════════════════════════════════════════

/// dry-run 预览标题
pub fn print_dry_run_header(total: usize, detail: &str) {
    let s = Styles::new();
    println!(
        "{}",
        s.dim.apply_to(format!("预览 {} 个操作{}:", total, detail))
    );
}

/// 打印 dry-run 模式的单个操作预览
pub fn print_dry_run_item(action: &str, path: &str) {
    let s = Styles::new();
    let icon = match action {
        "CREATE" | "CREATE PAGE" | "CREATE FILE" => s.success.apply_to("+"),
        "UPDATE" | "UPDATE FILE" => s.info.apply_to("~"),
        "DELETE" => s.error.apply_to("-"),
        "RENAME" => s.warn.apply_to("→"),
        "MOVE" => s.warn.apply_to("↳"),
        "REVERT" | "REVERT FILE" | "RECREATE" => s.warn.apply_to("↺"),
        _ => s.dim.apply_to("·"),
    };
    println!(
        "  {} {} {}",
        icon,
        s.dim.apply_to(format!("[{}]", action)),
        path
    );
}

/// 打印 dry-run 完成提示
pub fn print_dry_run_complete() {
    blank();
    dim("(dry-run) 未执行任何变更。");
}

// ═══════════════════════════════════════════════════════════
//  Worktree 管理输出
// ═══════════════════════════════════════════════════════════

/// worktree 列表项
pub struct WorktreeItem<'a> {
    pub path: &'a str,
    pub space_name: &'a str,
    pub space_id: &'a str,
    pub created_at: &'a str,
}

pub fn print_worktree_list(items: &[WorktreeItem]) {
    let s = Styles::new();
    if items.is_empty() {
        dim("No worktrees registered.");
        return;
    }
    println!(
        "{}:",
        s.bold.apply_to(format!("Worktrees ({})", items.len()))
    );
    for wt in items {
        println!(
            "  {} {}",
            s.cyan.apply_to(wt.path),
            s.dim.apply_to(format!("({})", wt.space_name))
        );
        println!(
            "    {} {} · {} {}",
            s.dim.apply_to("Space:"),
            wt.space_id,
            s.dim.apply_to("Created:"),
            wt.created_at
        );
    }
}

/// worktree add 完成后的摘要
pub fn print_worktree_add_complete(
    folders: usize,
    pages: usize,
    files: usize,
    errors: &[String],
    canonical_path: &str,
) {
    let s = Styles::new();
    blank();
    println!(
        "{}  {} 个目录, {} 个页面, {} 个文件",
        s.success.apply_to("✓"),
        folders,
        pages,
        files,
    );
    if !errors.is_empty() {
        println!("  {} {} 个错误:", s.warn.apply_to("⚠"), errors.len());
        for err in errors {
            println!("    {} {}", s.error.apply_to("×"), err);
        }
    }
    blank();
    success("Worktree created successfully!");
    blank();
    println!("To enter the worktree, run:");
    hint(&format!("cd {}", canonical_path));
}

// ═══════════════════════════════════════════════════════════
//  文件类型支持提示
// ═══════════════════════════════════════════════════════════

/// 为 git status 中的文件路径附加类型支持提示
pub fn format_status_file(path: &str, is_tracked: bool) -> String {
    let s = Styles::new();
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let support_hint = match ext.to_lowercase().as_str() {
        "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" if !is_tracked => " ⚠ 可能不支持推送",
        _ => "",
    };

    if support_hint.is_empty() {
        path.to_string()
    } else {
        format!("{} {}", path, s.warn.apply_to(support_hint))
    }
}

// ═══════════════════════════════════════════════════════════
//  MCP 错误友好化
// ═══════════════════════════════════════════════════════════

/// 将 MCP 错误码转换为用户友好的中文提示
pub fn friendly_mcp_error(code: i32, message: &str) -> String {
    match code {
        401 | -32001 => format!("认证失败: {}。请运行 `lx login` 重新登录。", message),
        403 | -32003 => format!(
            "无权访问: {}。请确认是否有该空间/文档的访问权限，或联系空间管理员。",
            message
        ),
        404 | -32004 => format!(
            "未找到: {}。请检查 ID 是否正确，或该资源是否已被删除。",
            message
        ),
        429 | -32029 => "请求过于频繁，请稍后再试。".to_string(),
        500 | -32000 => format!("服务端内部错误: {}。如持续出现，请联系管理员。", message),
        _ => format!("MCP 错误 ({}): {}", code, message),
    }
}
