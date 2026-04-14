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
    tracing_subscriber::fmt::init();

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
        Some(Commands::Login { token }) => {
            if let Some(t) = token {
                auth::save_token_direct(&t)?;
                println!("✓ Token 已保存，登录成功");
            } else {
                let _token = auth::login().await?;
                println!("✓ 登录成功");
            }
        }
        Some(Commands::Logout) => {
            auth::logout()?;
            println!("已登出");
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
