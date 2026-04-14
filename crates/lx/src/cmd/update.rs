//! Update 命令处理

use anyhow::Result;
use std::time::Duration;

use crate::update::{CheckResult, UpdateChecker, UpdateConfig};

/// 检查更新
pub async fn handle_check(prerelease: bool) -> Result<()> {
    let config = UpdateConfig {
        include_prerelease: prerelease,
        ..Default::default()
    };
    let checker = UpdateChecker::with_config(config);

    println!("正在检查更新...");

    let result = checker.check().await?;
    checker.save_check_time()?;

    print_check_result(&result);

    Ok(())
}

/// 列出最近的发布版本
pub async fn handle_list(limit: usize) -> Result<()> {
    let checker = UpdateChecker::new();

    println!("正在获取版本列表...\n");

    let releases = checker.list_releases(limit).await?;

    if releases.is_empty() {
        println!("未找到任何发布版本");
        return Ok(());
    }

    let current = UpdateChecker::current_version();
    println!("当前版本: v{}\n", current);

    for (i, release) in releases.iter().enumerate() {
        let marker = if release.latest_version == current {
            " (当前)"
        } else if release.has_update {
            " (新版本)"
        } else {
            ""
        };

        println!("{}. v{}{}", i + 1, release.latest_version, marker);
        println!("   {}", release.release_url);

        if let Some(ref notes) = release.release_notes {
            // 只显示第一行摘要
            if let Some(first_line) = notes.lines().next() {
                let summary = if first_line.len() > 60 {
                    format!("{}...", &first_line[..60])
                } else {
                    first_line.to_string()
                };
                if !summary.trim().is_empty() {
                    println!("   {}", summary);
                }
            }
        }
        println!();
    }

    Ok(())
}

/// 自动检查更新（静默模式，仅在有更新时输出）
pub async fn auto_check_update() {
    let checker = UpdateChecker::new();

    // 检查是否应该进行更新检查
    if !checker.should_check() {
        return;
    }

    // 使用较短的超时时间，避免影响用户体验
    let config = UpdateConfig {
        check_interval: Duration::from_secs(24 * 60 * 60),
        ..Default::default()
    };
    let checker = UpdateChecker::with_config(config);

    // 静默检查，忽略错误
    if let Ok(result) = checker.check().await {
        let _ = checker.save_check_time();

        if result.has_update {
            eprintln!();
            eprintln!(
                "\x1b[33m提示: 发现新版本 v{} (当前 v{})\x1b[0m",
                result.latest_version, result.current_version
            );
            eprintln!("运行 'lx update check' 查看详情");
            eprintln!();
        }
    }
}

/// 打印检查结果
fn print_check_result(result: &CheckResult) {
    println!();
    println!("当前版本: v{}", result.current_version);
    println!("最新版本: v{}", result.latest_version);
    println!();

    if result.has_update {
        println!("\x1b[32m✓ 发现新版本!\x1b[0m");
        println!();
        println!("下载地址: {}", result.release_url);

        if let Some(ref url) = result.download_url {
            println!("直接下载: {}", url);
        }

        if let Some(ref notes) = result.release_notes {
            println!();
            println!("更新说明:");
            println!("─────────────────────────────────────────");
            // 限制输出长度
            let lines: Vec<&str> = notes.lines().take(20).collect();
            for line in lines {
                println!("{}", line);
            }
            if notes.lines().count() > 20 {
                println!("...(更多内容请访问 release 页面)");
            }
            println!("─────────────────────────────────────────");
        }

        println!();
        println!("安装方式:");
        println!("  cargo install --git https://github.com/nicholasniu/lexiang-cli");
        if result.download_url.is_some() {
            println!("  或下载预编译二进制文件并替换现有程序");
        }
    } else {
        println!("\x1b[32m✓ 已是最新版本\x1b[0m");
    }
}
