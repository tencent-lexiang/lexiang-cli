use crate::datadir;
use crate::skill::{AgentKind, InstallScope, SkillGenerator, SkillInstaller};
use anyhow::Result;
use std::path::PathBuf;

/// 生成 skill 文件（lx skill generate）
pub fn handle_generate(output: Option<&str>) -> Result<()> {
    let schema = super::load_schema()
        .ok_or_else(|| anyhow::anyhow!("No schema found. Run 'lx tools sync' first."))?;

    let output_dir = match output {
        Some(path) => PathBuf::from(path),
        None => datadir::skills_dir(),
    };

    let generator = SkillGenerator::new(&schema, output_dir.clone());
    let files = generator.generate_all()?;

    println!("Generated {} skill files to {:?}:", files.len(), output_dir);
    for file in files {
        // 显示相对于 output_dir 的路径
        let display = file.strip_prefix(&output_dir).unwrap_or(&file).display();
        println!("  - {}", display);
    }

    Ok(())
}

/// 解析 agent 参数
fn parse_agents(agent_str: &str) -> Result<Vec<AgentKind>> {
    if agent_str == "all" {
        return Ok(AgentKind::all().to_vec());
    }

    let mut agents = Vec::new();
    for name in agent_str.split(',') {
        let name = name.trim();
        match AgentKind::from_name(name) {
            Some(agent) => agents.push(agent),
            None => {
                let valid: Vec<_> = AgentKind::all()
                    .iter()
                    .map(super::super::skill::installer::AgentKind::name)
                    .collect();
                anyhow::bail!("未知 agent: '{}'\n有效值: {}, all", name, valid.join(", "));
            }
        }
    }
    Ok(agents)
}

/// 解析 scope 参数
fn parse_scope(scope_str: &str) -> Result<InstallScope> {
    match scope_str.to_lowercase().as_str() {
        "user" | "global" => Ok(InstallScope::User),
        "project" | "local" => Ok(InstallScope::Project),
        _ => anyhow::bail!("未知 scope: '{}'\n有效值: user, project", scope_str),
    }
}

/// 安装 skill 到 Agent 配置目录（lx skill install）
pub fn handle_install(agent_str: &str, scope_str: &str, project_dir: Option<&str>) -> Result<()> {
    let agents = parse_agents(agent_str)?;
    let scope = parse_scope(scope_str)?;

    let project_path = match scope {
        InstallScope::Project => Some(
            project_dir
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
        ),
        InstallScope::User => project_dir.map(PathBuf::from),
    };

    let installer = SkillInstaller::new(None, project_path);
    let results = installer.install(&agents, scope)?;

    println!("✅ Skill 安装完成：\n");
    for result in &results {
        println!("{}", result);
    }

    println!("\n💡 提示：AI agent 将自动发现安装的 skill 文件。");

    Ok(())
}

/// 更新 skill：重新生成 + 重新安装（lx skill update）
pub fn handle_update(agent_str: &str, scope_str: &str, project_dir: Option<&str>) -> Result<()> {
    println!("🔄 更新 skill 文件...\n");

    // 1. 重新生成
    handle_generate(None)?;
    println!();

    // 2. 重新安装（install_to_dir 会先清理旧文件）
    handle_install(agent_str, scope_str, project_dir)?;

    Ok(())
}

/// 卸载 skill（lx skill uninstall）
pub fn handle_uninstall(agent_str: &str, scope_str: &str, project_dir: Option<&str>) -> Result<()> {
    let agents = parse_agents(agent_str)?;
    let scope = parse_scope(scope_str)?;

    let project_path = match scope {
        InstallScope::Project => Some(
            project_dir
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
        ),
        InstallScope::User => project_dir.map(PathBuf::from),
    };

    let installer = SkillInstaller::new(None, project_path);
    let removed = installer.uninstall(&agents, scope)?;

    if removed.is_empty() {
        println!("没有找到已安装的 skill 文件。");
    } else {
        println!("✅ Skill 卸载完成：\n");
        for (agent, path) in &removed {
            println!("  ✓ {} → {:?}", agent.display_name(), path);
        }
    }

    Ok(())
}

/// 显示安装状态（lx skill status）
pub fn handle_status(project_dir: Option<&str>) -> Result<()> {
    let project_path = project_dir
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok());

    let installer = SkillInstaller::new(None, project_path);
    let status = installer.status();

    println!("lx CLI Skill 安装状态：\n");
    println!("  {:<15} {:<10} {:<10} Path", "Agent", "Scope", "Status");
    println!("  {}", "-".repeat(70));

    for agent in AgentKind::all() {
        if let Some(entries) = status.get(agent) {
            for (scope, path, installed) in entries {
                let status_icon = if *installed { "✅" } else { "—" };
                println!(
                    "  {:<15} {:<10} {:<10} {}",
                    agent.display_name(),
                    format!("{}", scope),
                    status_icon,
                    path.display()
                );
            }
        }
    }

    println!("\n💡 运行 'lx skill install' 安装到所有 agent");

    Ok(())
}
