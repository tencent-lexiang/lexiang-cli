//! `lx sh` — 虚拟 Shell 交互模式
//!
//! 支持两种模式:
//!
//! ## 1. Worktree 模式（默认）
//! 在 worktree 目录下运行，`/kb` 映射到本地磁盘文件:
//! ```text
//! cd ~/my-kb && lx sh          # 自动检测 .lxworktree
//! lx sh --path ~/my-kb         # 指定 worktree 路径
//! ```
//!
//! ## 2. MCP 远程模式
//! 直接连接远程知识库（无需本地 worktree）:
//! ```text
//! lx sh --space <space_id>
//! ```
//!
//! ## 文件系统布局
//! ```text
//! /             → InMemoryFs (base, 含 /tmp)
//! /kb           → WorktreeFs (本地磁盘) 或 LexiangFs (MCP 远程)
//! ```
//!
//! ## 编程 API
//!
//! Agent / 测试代码直接用 `build_shell()` 拿到 `Bash` 实例，调 `exec()` 即可:
//! ```ignore
//! let mut bash = sh::build_shell(&config, None, None).await?;
//! let out = bash.exec("grep -r OAuth /kb | head -5").await?;
//! println!("{}", out.stdout);
//! ```

use crate::config::Config;
use crate::shell::bash::Bash;
use crate::shell::commands::bridge::BridgeFn;
use crate::shell::fs::lexiang::{McpSpace, RealMcpCaller};
use crate::shell::fs::{InMemoryFs, LexiangFs, MountableFs, WorktreeFs};
use crate::worktree::WorktreeConfig;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

// ═══════════════════════════════════════════════════════════
//  Shell 模式
// ═══════════════════════════════════════════════════════════

/// Shell 运行模式
enum ShellMode {
    /// 基于本地 worktree 目录
    Worktree {
        worktree_path: PathBuf,
        wt_config: WorktreeConfig,
    },
    /// 基于 MCP 远程 API
    Mcp { space_id: String },
}

/// 解析 Shell 运行模式
///
/// 优先级:
/// 1. `--space` → MCP 远程模式
/// 2. `--path` → 指定 worktree 路径
/// 3. 自动检测 cwd 向上查找 .lxworktree
fn resolve_mode(space: Option<&str>, path: Option<&str>) -> Result<ShellMode> {
    // 1. --space 明确指定 → MCP 模式
    if let Some(space_input) = space {
        let space_id = crate::cmd::utils::parse_space_id(space_input);
        return Ok(ShellMode::Mcp { space_id });
    }

    // 2. --path 明确指定 → Worktree 模式
    if let Some(p) = path {
        let worktree_path = PathBuf::from(p);
        let config_dir = worktree_path.join(".lxworktree");
        if !config_dir.exists() {
            anyhow::bail!(
                "Not a worktree directory: {}\n\
                 Hint: use 'lx git clone <space_id> <path>' to create a worktree first.",
                p
            );
        }
        let wt_config = WorktreeConfig::load(&worktree_path)?;
        return Ok(ShellMode::Worktree {
            worktree_path,
            wt_config,
        });
    }

    // 3. 自动检测: 从 cwd 向上查找 .lxworktree
    match crate::cmd::git::find_worktree_path() {
        Ok(worktree_path) => {
            let wt_config = WorktreeConfig::load(&worktree_path)?;
            Ok(ShellMode::Worktree {
                worktree_path,
                wt_config,
            })
        }
        Err(_) => {
            anyhow::bail!(
                "Not inside a worktree directory.\n\n\
                 Usage:\n\
                 \x20 lx sh                     # run inside a worktree directory\n\
                 \x20 lx sh --path <worktree>   # specify worktree path\n\
                 \x20 lx sh --space <space_id>  # connect to remote knowledge base via MCP\n\n\
                 To create a worktree:\n\
                 \x20 lx git clone <space_id> <path>"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════
//  核心 API: build_shell — 所有调用方的唯一入口
// ═══════════════════════════════════════════════════════════

/// 构建一个虚拟 Shell 实例
///
/// 文件系统布局:
/// - `/`    → `InMemoryFs` (base)
/// - `/kb`  → `WorktreeFs` (本地) 或 `LexiangFs` (远程)
/// - `/tmp` → `InMemoryFs` (临时区，可写)
///
/// 默认 cwd = `/kb`
pub async fn build_shell(config: &Config, space: Option<&str>, path: Option<&str>) -> Result<Bash> {
    let mode = resolve_mode(space, path)?;

    let bash = match mode {
        ShellMode::Worktree {
            worktree_path,
            wt_config,
        } => build_worktree_shell(config, worktree_path, wt_config).await?,
        ShellMode::Mcp { space_id } => build_mcp_shell(config, &space_id).await?,
    };

    Ok(bash)
}

/// Worktree 模式: /kb 映射到本地磁盘
async fn build_worktree_shell(
    config: &Config,
    worktree_path: PathBuf,
    wt_config: WorktreeConfig,
) -> Result<Bash> {
    eprintln!(
        "📚 已连接知识库: {} (worktree: {})",
        wt_config.space_name,
        worktree_path.display()
    );

    let space_name = wt_config.space_name.clone();
    let space_id = wt_config.space_id.clone();
    let wt_path_for_git = worktree_path.clone();

    // 构建文件系统: /kb → WorktreeFs (本地磁盘)
    let worktree_fs = WorktreeFs::new(worktree_path, space_name, space_id);
    let base_fs = InMemoryFs::new().with_dir("/tmp");
    let fs = MountableFs::new(Box::new(base_fs)).mount("/kb", Box::new(worktree_fs));

    let mut bash = Bash::new(Box::new(fs)).with_cwd("/kb");

    // 获取有效 token 供桥接命令使用（过期自动刷新）
    let resolved_token = crate::auth::get_access_token(config).await.ok();

    // 注入桥接命令
    register_bridge_commands(&mut bash, config, resolved_token);
    register_git_bridge(&mut bash, wt_path_for_git);

    Ok(bash)
}

/// MCP 远程模式: /kb 映射到 MCP API
async fn build_mcp_shell(config: &Config, space_id: &str) -> Result<Bash> {
    let mcp_url = &config.mcp.url;
    let access_token = Some(crate::auth::get_access_token(config).await?);

    let caller = RealMcpCaller::new(mcp_url, access_token.clone());

    use crate::shell::fs::lexiang::McpCaller;
    let space_result = caller
        .call_tool(
            "space_describe_space",
            serde_json::json!({ "space_id": space_id }),
        )
        .await?;

    // MCP 返回可能嵌套在 data 字段中
    let space_data = space_result
        .get("data")
        .and_then(|d| d.get("space").or(Some(d)))
        .unwrap_or(&space_result);

    let space: McpSpace = serde_json::from_value(space_data.clone()).map_err(|e| {
        anyhow::anyhow!(
            "Failed to parse space info: {e}\nResponse: {}",
            serde_json::to_string_pretty(&space_result).unwrap_or_default()
        )
    })?;

    eprintln!(
        "📚 已连接知识库: {} (MCP 远程模式, space_id: {})",
        space.name, space.id
    );

    // 构建文件系统: /kb → LexiangFs (MCP 远程)
    let mcp_caller = RealMcpCaller::new(mcp_url, access_token.clone());
    let lexiang_fs = LexiangFs::new(space, Box::new(mcp_caller));
    let base_fs = InMemoryFs::new().with_dir("/tmp");
    let fs = MountableFs::new(Box::new(base_fs)).mount("/kb", Box::new(lexiang_fs));

    let mut bash = Bash::new(Box::new(fs)).with_cwd("/kb");

    // 注入桥接命令 (MCP 模式仅注入 search / mcp)
    register_bridge_commands(&mut bash, config, access_token);

    Ok(bash)
}

// ═══════════════════════════════════════════════════════════
//  单次执行 (CLI: lx sh --exec)
// ═══════════════════════════════════════════════════════════

/// 构建 shell 实例 (供 main.rs 调用)
pub async fn exec_command(
    config: &Config,
    space: Option<&str>,
    path: Option<&str>,
) -> Result<Bash> {
    build_shell(config, space, path).await
}

// ═══════════════════════════════════════════════════════════
//  REPL 交互模式 (CLI: lx sh)
// ═══════════════════════════════════════════════════════════

/// 启动 REPL 交互模式
pub async fn start_repl(config: &Config, space: Option<&str>, path: Option<&str>) -> Result<()> {
    let mut bash = build_shell(config, space, path).await?;

    eprintln!("🐚 lx sh — Virtual Shell for Lexiang Knowledge Base");
    eprintln!("   Type 'help' for commands, 'exit' or Ctrl+D to quit\n");

    let history_path = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("lx")
        .join("sh_history");

    // 确保历史文件目录存在
    if let Some(parent) = history_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let mut rl = rustyline::DefaultEditor::new()?;
    let _ = rl.load_history(&history_path);

    loop {
        let cwd = bash.cwd().to_string();
        let prompt = format!("\x1b[1;36mlx:{cwd}\x1b[0m$ ");

        match rl.readline(&prompt) {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(trimmed);

                // REPL 内置命令
                match trimmed {
                    "exit" | "quit" => break,
                    "help" => {
                        print_help(&bash);
                        continue;
                    }
                    _ => {}
                }

                // 执行
                match bash.exec(trimmed).await {
                    Ok(output) => {
                        if !output.stdout.is_empty() {
                            print!("{}", output.stdout);
                        }
                        if !output.stderr.is_empty() {
                            eprint!("{}", output.stderr);
                        }
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                // Ctrl+C — 不退出，只取消当前行
                eprintln!("^C");
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                // Ctrl+D — 退出
                eprintln!("exit");
                break;
            }
            Err(e) => {
                eprintln!("readline error: {e}");
                break;
            }
        }
    }

    let _ = rl.save_history(&history_path);
    Ok(())
}

// ═══════════════════════════════════════════════════════════
//  桥接命令注入
// ═══════════════════════════════════════════════════════════

/// 注入 search / mcp 等桥接命令到 shell
fn register_bridge_commands(bash: &mut Bash, config: &Config, resolved_token: Option<String>) {
    let url = config.mcp.url.clone();
    let token = resolved_token;

    // search — 知识库关键词搜索
    {
        let url = url.clone();
        let token = token.clone();
        let handler: BridgeFn = Arc::new(move |args: Vec<String>| {
            let url = url.clone();
            let token = token.clone();
            Box::pin(async move {
                let keyword = args.join(" ");
                if keyword.is_empty() {
                    return Ok((String::new(), "search: missing keyword".to_string(), 1));
                }
                let caller = RealMcpCaller::new(&url, token);
                use crate::shell::fs::lexiang::McpCaller;
                match caller
                    .call_tool(
                        "search_kb_search",
                        serde_json::json!({ "keyword": keyword, "type": "kb_doc" }),
                    )
                    .await
                {
                    Ok(result) => Ok((format_search_results(&result), String::new(), 0)),
                    Err(e) => Ok((String::new(), format!("search error: {e}"), 1)),
                }
            })
        });
        bash.register_bridge("search", "Search knowledge base", vec![], handler);
    }

    // mcp — 透传调用任意 MCP tool
    {
        let url = url.clone();
        let token = token.clone();
        let handler: BridgeFn = Arc::new(move |args: Vec<String>| {
            let url = url.clone();
            let token = token.clone();
            Box::pin(async move {
                if args.is_empty() {
                    return Ok((
                        String::new(),
                        "usage: mcp <tool_name> [json_args]".to_string(),
                        1,
                    ));
                }
                let tool_name = &args[0];
                let json_args: serde_json::Value = if args.len() > 1 {
                    serde_json::from_str(&args[1..].join(" ")).unwrap_or(serde_json::json!({}))
                } else {
                    serde_json::json!({})
                };

                let caller = RealMcpCaller::new(&url, token);
                use crate::shell::fs::lexiang::McpCaller;
                match caller.call_tool(tool_name, json_args).await {
                    Ok(result) => {
                        let output = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| format!("{result:?}"));
                        Ok((output, String::new(), 0))
                    }
                    Err(e) => Ok((String::new(), format!("mcp error: {e}"), 1)),
                }
            })
        });
        bash.register_bridge(
            "mcp",
            "Call MCP tool directly (mcp <tool> [json_args])",
            vec![],
            handler,
        );
    }
}

/// 注入 git 桥接命令 (仅 worktree 模式)
fn register_git_bridge(bash: &mut Bash, worktree_path: PathBuf) {
    let handler: BridgeFn = Arc::new(move |args: Vec<String>| {
        let wt_path = worktree_path.clone();
        Box::pin(async move {
            if args.is_empty() {
                return Ok((
                    String::new(),
                    "usage: git <subcommand>\n\
                     \n\
                     Available subcommands:\n\
                     \x20 status    Show working tree status\n\
                     \x20 log       Show commit history\n\
                     \x20 diff      Show local changes\n\
                     \x20 pull      Fetch and merge remote changes\n\
                     \x20 push      Push local changes to remote\n\
                     \x20 add       Stage changes\n\
                     \x20 commit    Record changes\n\
                     \x20 remote    Show remote information\n"
                        .to_string(),
                    1,
                ));
            }

            let subcmd = &args[0];
            match subcmd.as_str() {
                "status" => git_bridge_status(&wt_path),
                "log" => {
                    let max_count = args
                        .iter()
                        .position(|a| a == "-n" || a == "--max-count")
                        .and_then(|i| args.get(i + 1))
                        .and_then(|n| n.parse().ok())
                        .unwrap_or(10);
                    git_bridge_log(&wt_path, max_count)
                }
                "diff" => git_bridge_diff(&wt_path),
                "remote" => git_bridge_remote(&wt_path, args.contains(&"-v".to_string())),
                "pull" | "push" | "add" | "commit" => Ok((
                    String::new(),
                    format!(
                        "git {subcmd}: this command modifies state.\n\
                         Please use 'lx git {subcmd}' from your terminal instead."
                    ),
                    1,
                )),
                _ => Ok((
                    String::new(),
                    format!("git: unknown subcommand '{subcmd}'"),
                    1,
                )),
            }
        })
    });

    bash.register_bridge(
        "git",
        "Git-style knowledge base operations",
        vec![
            "status", "log", "diff", "pull", "push", "add", "commit", "remote",
        ],
        handler,
    );
}

/// git status (只读，在虚拟 shell 中安全)
fn git_bridge_status(wt_path: &std::path::Path) -> Result<(String, String, i32), anyhow::Error> {
    use crate::worktree::Repository;

    let wt_config = WorktreeConfig::load(wt_path)?;
    let repo = Repository::open(wt_path)?;

    let mut out = String::new();
    out.push_str("On branch master\n");
    out.push_str(&format!(
        "Remote: {} ({})\n\n",
        wt_config.space_name, wt_config.space_id
    ));

    let status = repo.status()?;

    if status.staged.is_empty()
        && status.modified.is_empty()
        && status.deleted.is_empty()
        && status.untracked.is_empty()
    {
        out.push_str("nothing to commit, working tree clean\n");
    } else {
        if !status.modified.is_empty() {
            out.push_str("Changes not staged for commit:\n");
            for f in &status.modified {
                out.push_str(&format!("        modified:   {f}\n"));
            }
            out.push('\n');
        }
        if !status.deleted.is_empty() {
            out.push_str("Deleted files:\n");
            for f in &status.deleted {
                out.push_str(&format!("        deleted:    {f}\n"));
            }
            out.push('\n');
        }
        if !status.untracked.is_empty() {
            out.push_str("Untracked files:\n");
            for f in &status.untracked {
                out.push_str(&format!("        {f}\n"));
            }
        }
    }

    Ok((out, String::new(), 0))
}

/// git log (只读)
fn git_bridge_log(
    wt_path: &std::path::Path,
    max_count: usize,
) -> Result<(String, String, i32), anyhow::Error> {
    use crate::worktree::Repository;

    let repo = Repository::open(wt_path)?;
    let commits = repo.log(Some(max_count))?;

    let mut out = String::new();
    for commit in commits {
        out.push_str(&format!("commit {}\n", commit.hash));
        out.push_str(&format!("Author: {}\n", commit.author));
        out.push_str(&format!("Date:   {}\n\n", commit.date));
        out.push_str(&format!("    {}\n\n", commit.message.trim()));
    }

    Ok((out, String::new(), 0))
}

/// git diff (只读)
fn git_bridge_diff(wt_path: &std::path::Path) -> Result<(String, String, i32), anyhow::Error> {
    use crate::worktree::Repository;

    let repo = Repository::open(wt_path)?;
    let status = repo.status()?;

    let mut out = String::new();
    if status.modified.is_empty() && status.untracked.is_empty() && status.deleted.is_empty() {
        out.push_str("No changes.\n");
    } else {
        for f in &status.modified {
            out.push_str(&format!("M  {f}\n"));
        }
        for f in &status.untracked {
            out.push_str(&format!("?? {f}\n"));
        }
        for f in &status.deleted {
            out.push_str(&format!("D  {f}\n"));
        }
    }

    Ok((out, String::new(), 0))
}

/// git remote (只读)
fn git_bridge_remote(
    wt_path: &std::path::Path,
    verbose: bool,
) -> Result<(String, String, i32), anyhow::Error> {
    let wt_config = WorktreeConfig::load(wt_path)?;

    let mut out = String::new();
    if verbose {
        out.push_str(&format!(
            "origin\thttps://lexiangla.com/spaces/{} (fetch)\n",
            wt_config.space_id
        ));
        out.push_str(&format!(
            "origin\thttps://lexiangla.com/spaces/{} (push)\n",
            wt_config.space_id
        ));
    } else {
        out.push_str("origin\n");
    }

    Ok((out, String::new(), 0))
}

// ═══════════════════════════════════════════════════════════
//  帮助信息
// ═══════════════════════════════════════════════════════════

fn print_help(bash: &Bash) {
    eprintln!("📖 Available commands:\n");

    let commands = bash.list_commands();
    let aliases = bash.list_aliases();

    let core = ["ls", "cat", "grep", "find", "tree"];
    let util = [
        "head", "tail", "wc", "sort", "uniq", "echo", "pwd", "cd", "stat", "xargs",
    ];
    let guards = ["rm", "mv", "cp", "mkdir", "touch", "chmod"];

    eprintln!("  Core:     {}", core.join(", "));
    eprintln!("  Utility:  {}", util.join(", "));
    eprintln!("  Guards:   {} (read-only filesystem)", guards.join(", "));

    let builtins: std::collections::HashSet<&str> = core
        .iter()
        .chain(util.iter())
        .chain(guards.iter())
        .chain(["fzf", "sort", "uniq"].iter())
        .copied()
        .collect();

    let bridge_cmds: Vec<&&str> = commands
        .iter()
        .filter(|c| !builtins.contains(**c) && !aliases.iter().any(|(a, _)| *a == **c))
        .collect();

    if !bridge_cmds.is_empty() {
        eprintln!(
            "  Bridge:   {}",
            bridge_cmds
                .iter()
                .map(|c| **c)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    if !aliases.is_empty() {
        eprintln!("\n  Aliases:");
        for (alias, target) in &aliases {
            eprintln!("    {alias:<12} → {target}");
        }
    }

    eprintln!("\n  Special:  help, exit, quit");
    eprintln!("\n  Filesystem:");
    eprintln!("    /kb     — Knowledge base (local worktree or remote MCP)");
    eprintln!("    /tmp    — Temporary storage (read-write)");
}

/// 格式化搜索结果
fn format_search_results(result: &serde_json::Value) -> String {
    let mut output = String::new();

    let items = result
        .get("items")
        .or_else(|| result.get("results"))
        .or_else(|| result.get("entries"));

    if let Some(items) = items.and_then(|v| v.as_array()) {
        for (i, item) in items.iter().enumerate() {
            let name = item
                .get("name")
                .or_else(|| item.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or("(untitled)");
            let entry_id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let entry_type = item
                .get("entry_type")
                .or_else(|| item.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("page");
            let snippet = item
                .get("snippet")
                .or_else(|| item.get("summary"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            output.push_str(&format!(
                "{}. [{}] {} ({})\n",
                i + 1,
                entry_type,
                name,
                entry_id
            ));
            if !snippet.is_empty() {
                output.push_str(&format!("   {}\n", snippet));
            }
        }
        if items.is_empty() {
            output.push_str("No results found.\n");
        }
    } else {
        output = serde_json::to_string_pretty(result).unwrap_or_else(|_| format!("{result:?}"));
    }

    output
}
