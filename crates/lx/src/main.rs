mod auth;
mod cmd;
mod config;
mod daemon;
mod datadir;
pub mod serve;
pub mod shell;
mod update;
mod vfs;
mod worktree;

mod json_rpc;

mod mcp;
mod service;
mod skill;
mod version;

use clap::Parser;
use cmd::{Cli, Commands};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志：如果是 serve 模式，日志写入文件；否则输出到 stderr
    init_logging();

    let args: Vec<String> = std::env::args().collect();

    // 加载 schema（用于动态命令）
    let schema = cmd::load_schema();

    // 检查是否是 --help 或 -h（需要显示包含动态命令的帮助）
    let is_help = args.iter().any(|a| a == "--help" || a == "-h");
    let is_root_help = is_help && args.len() == 2;

    if is_root_help {
        // 显示包含动态命令的帮助
        cmd::print_help_with_dynamic_commands(schema.as_ref());
        return Ok(());
    }

    // 检查是否是高级 block 命令（静态优先，动态回退）
    if args.len() >= 3 && args[1] == "block" && cmd::try_handle_block_command(&args).await? {
        return Ok(());
    }
    // 不是高级命令，继续 fallthrough 到动态命令

    // 检查是否是动态命令
    if args.len() >= 2 {
        if let Some(ref schema) = schema {
            let potential_namespace = &args[1];
            if schema
                .get_namespaces()
                .contains(&potential_namespace.to_string())
            {
                // 这是一个动态命令，构建并执行
                return cmd::handle_dynamic_command(&args, schema).await;
            }

            // 不是已知 namespace，尝试当作子命令在所有 namespace 中查找唯一匹配
            // 例如 `lx whoami` -> `lx contact whoami`
            if let Some(resolved) = schema.resolve_unique_subcommand(potential_namespace) {
                let mut new_args = Vec::with_capacity(args.len() + 1);
                new_args.push(args[0].clone()); // "lx"
                new_args.push(resolved.namespace.clone()); // 自动插入 namespace
                new_args.extend_from_slice(&args[1..]); // 原始子命令及后续参数
                eprintln!(
                    "hint: `lx {}` resolved to `lx {} {}`",
                    potential_namespace, resolved.namespace, potential_namespace
                );
                return cmd::handle_dynamic_command(&new_args, schema).await;
            }
        }
    }

    // 常规命令处理
    let cli = Cli::parse();
    let mut config = config::Config::load()?;

    // 将 --token / LX_ACCESS_TOKEN 注入 Config，零侵入传递
    if let Some(ref token) = cli.token {
        config.mcp.access_token = Some(token.clone());
    }

    match cli.command {
        Some(Commands::Version) => {
            println!(
                "{} v{}",
                env!("CARGO_PKG_NAME"),
                crate::version::current_version()
            );
            // 版本命令后自动检查更新
            cmd::auto_check_update().await;
        }
        Some(Commands::Login { token, client }) => {
            if let Some(t) = token {
                if client {
                    anyhow::bail!("--token and --client cannot be used together");
                }
                auth::save_token_direct(&t)?;
                println!("✓ Token 已保存，登录成功");
            } else if client {
                // 清理上次残留的回调文件
                let _ = auth::clear_callback_url();

                // 尝试注册 URL scheme（如果尚未注册）
                if !auth::is_url_scheme_registered() {
                    if let Err(e) = auth::register_url_scheme() {
                        tracing::debug!("URL scheme 注册失败，将回退到手动粘贴: {e}");
                    } else {
                        println!("✓ 已注册 lexiang:// URL scheme，浏览器回调将自动完成登录");
                    }
                }

                let scheme_registered = auth::is_url_scheme_registered();
                println!(
                    "\n请在浏览器中打开以下链接完成登录：\n{}\n",
                    auth::client_login_url(None)
                );

                if scheme_registered {
                    // 写 pending 标记，等待文件 IPC 回调
                    auth::write_pending_login()?;
                    println!("浏览器登录完成后将自动回调，无需手动操作...");
                    match auth::wait_for_callback_url().await {
                        Ok(callback_url) => {
                            let _ = auth::clear_pending_login();
                            let _token = auth::login_with_client_callback(&callback_url).await?;
                            println!("✓ 客户端登录成功，Cookie 与 MCP Token 已保存");
                        }
                        Err(e) => {
                            let _ = auth::clear_pending_login();
                            println!("\n自动回调超时: {e}");
                            println!("请手动粘贴回调链接：");
                            let mut callback = String::new();
                            std::io::stdin().read_line(&mut callback)?;
                            let _token = auth::login_with_client_callback(callback.trim()).await?;
                            println!("✓ 客户端登录成功，Cookie 与 MCP Token 已保存");
                        }
                    }
                } else {
                    println!("登录完成后，请粘贴 lexiang://auth-callback?... 回调链接：");
                    let mut callback = String::new();
                    std::io::stdin().read_line(&mut callback)?;
                    let _token = auth::login_with_client_callback(callback.trim()).await?;
                    println!("✓ 客户端登录成功，Cookie 与 MCP Token 已保存");
                }
            } else {
                let _token = auth::login().await?;
                println!("✓ 登录成功");
            }
        }
        Some(Commands::Logout) => {
            auth::logout()?;
            println!("已登出");
        }
        Some(Commands::HandleUrl { url }) => {
            if url.trim().is_empty() {
                eprintln!("错误：回调 URL 为空");
                std::process::exit(1);
            }
            auth::write_callback_url(&url)?;
            if auth::has_pending_login() {
                println!("✓ 已接收登录回调，正在完成登录...");
            } else {
                println!("✓ 已接收登录回调（无等待中的登录流程，回调 URL 已缓存）");
            }
        }
        Some(Commands::Start { mount, size }) => {
            use std::path::PathBuf;
            let mount_point = mount.map(PathBuf::from);
            let daemon = daemon::DaemonManager::new(mount_point, Some(size));
            daemon.start()?;
        }
        Some(Commands::Stop) => {
            let daemon = daemon::DaemonManager::new(None, None);
            daemon.stop()?;
        }
        Some(Commands::Status) => {
            let daemon = daemon::DaemonManager::new(None, None);
            let status = daemon.status()?;
            if status.running {
                println!("守护进程运行中 (PID: {:?})", status.pid);
                if let Some(vfs) = status.vfs_status {
                    println!("虚拟文件系统: {:?}", vfs.mount_point);
                    println!("大小: {}MB", vfs.size_mb);
                }
            } else {
                println!("守护进程未运行");
            }
        }
        Some(Commands::Mcp { command }) => match command {
            cmd::McpCommands::List => cmd::list_tools(&config).await?,
            cmd::McpCommands::Call { name, params } => {
                let params: serde_json::Value = params
                    .map(|p| serde_json::from_str(&p))
                    .transpose()?
                    .unwrap_or(serde_json::json!({}));
                cmd::call_tool(&config, &name, params).await?;
            }
        },
        Some(Commands::Tools { command }) => match command {
            cmd::ToolsCommands::Sync => cmd::handle_sync(&config).await?,
            cmd::ToolsCommands::Categories => cmd::handle_categories()?,
            cmd::ToolsCommands::Version => cmd::handle_version()?,
            cmd::ToolsCommands::List { category, format } => {
                cmd::handle_list(category.as_deref(), &format)?;
            }
            cmd::ToolsCommands::Schema => cmd::handle_schema()?,
            cmd::ToolsCommands::SyncEmbedded => cmd::handle_sync_embedded(&config).await?,
            cmd::ToolsCommands::SyncUnlisted => cmd::handle_sync_unlisted(&config).await?,
        },
        Some(Commands::Skill { command: subcmd }) => match subcmd {
            Some(cmd::SkillCommands::Generate { output }) => {
                cmd::handle_generate(output.as_deref())?;
            }
            Some(cmd::SkillCommands::Install {
                agent,
                scope,
                project_dir,
            }) => {
                cmd::handle_install(&agent, &scope, project_dir.as_deref())?;
            }
            Some(cmd::SkillCommands::Update {
                agent,
                scope,
                project_dir,
            }) => {
                cmd::handle_update(&agent, &scope, project_dir.as_deref())?;
            }
            Some(cmd::SkillCommands::Uninstall {
                agent,
                scope,
                project_dir,
            }) => {
                cmd::handle_uninstall(&agent, &scope, project_dir.as_deref())?;
            }
            Some(cmd::SkillCommands::Status { project_dir }) => {
                cmd::handle_status(project_dir.as_deref())?;
            }
            None => {
                // 默认行为：生成 + 安装到所有 agent（用户级）
                cmd::handle_generate(None)?;
                println!();
                cmd::handle_install("all", "user", None)?;
            }
        },
        Some(Commands::Update { command }) => match command {
            Some(cmd::UpdateCommands::Check { prerelease }) => {
                cmd::handle_update_check(prerelease).await?;
            }
            Some(cmd::UpdateCommands::List { limit }) => {
                cmd::handle_update_list(limit).await?;
            }
            None => {
                // 默认行为：检查更新
                cmd::handle_update_check(false).await?;
            }
        },
        Some(Commands::Worktree { command }) => {
            cmd::git::handle_workspace_command(command, &config).await?;
        }
        Some(Commands::Git { command }) => {
            cmd::handle_git_command(command, &config).await?;
        }
        Some(Commands::Completion { shell }) => {
            Cli::generate_completion(shell);
        }
        Some(Commands::Sh { space, path, exec }) => {
            if let Some(command) = exec {
                // 单次执行模式
                let mut bash =
                    cmd::exec_command(&config, space.as_deref(), path.as_deref()).await?;
                let output = bash.exec(&command).await?;
                if !output.stdout.is_empty() {
                    print!("{}", output.stdout);
                }
                if !output.stderr.is_empty() {
                    eprint!("{}", output.stderr);
                }
                std::process::exit(output.exit_code);
            } else {
                // REPL 交互模式
                cmd::start_repl(&config, space.as_deref(), path.as_deref()).await?;
            }
        }
        Some(Commands::Serve { verbose }) => {
            serve::run_serve(config, verbose).await?;
        }
        None => {
            // 默认显示帮助（包含动态命令）
            cmd::print_help_with_dynamic_commands(schema.as_ref());
            // 无命令时也检查更新
            cmd::auto_check_update().await;
        }
    }

    Ok(())
}

/// 初始化日志系统
/// - serve 模式：日志同时写入文件（JSON）和输出到 stderr（文本，供 `VSCode` 捕获）
/// - 其他模式：日志输出到 stderr
fn init_logging() {
    use tracing_subscriber::prelude::*;

    let args: Vec<String> = std::env::args().collect();
    let is_serve = args.len() >= 2 && args[1] == "serve";

    if is_serve {
        // serve 模式：双输出 - 文件（JSON）+ stderr（紧凑文本，供 VSCode 实时查看）
        let log_dir = dirs::home_dir()
            .map(|h| h.join(".lexiang").join("logs"))
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp/lexiang-logs"));

        let _ = std::fs::create_dir_all(&log_dir);

        // 文件 appender（JSON 格式，用于持久化和排查）
        let file_appender = tracing_appender::rolling::daily(log_dir, "lx");

        // stderr 过滤器：只打印 lx crate 的 info+ 日志，屏蔽依赖库噪音
        let stderr_filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("lx=info"));

        // stderr layer（紧凑文本格式，用于 VSCode 实时展示）
        let stderr_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_ansi(false)
            .compact()
            .with_filter(stderr_filter);

        // 文件 layer（JSON 格式，全量日志）
        let file_layer = tracing_subscriber::fmt::layer()
            .json()
            .with_writer(file_appender)
            .with_ansi(false);

        tracing_subscriber::registry()
            .with(stderr_layer)
            .with(file_layer)
            .init();
    } else {
        // 其他模式：日志输出到 stderr
        tracing_subscriber::fmt::init();
    }
}
