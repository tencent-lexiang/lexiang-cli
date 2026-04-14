use crate::cmd::ui;
use crate::config::Config;
use crate::mcp::McpClient;
use crate::worktree::{self, Repository, WorktreeConfig, WorktreeRecord, WorktreeRegistry};
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::PathBuf;

use super::{
    create_page_with_content, extract_root_entry_id, extract_space_name, find_worktree_path,
    pull_entries_recursive, pull_page_content, push_page_content, truncate_path,
    update_file_content, upload_new_file,
};
use crate::cmd::cli::WorktreeCommands;

pub async fn handle_workspace_command(command: WorktreeCommands, config: &Config) -> Result<()> {
    match command {
        WorktreeCommands::Add {
            path,
            space_id,
            entry_ids,
        } => {
            let space_id = crate::cmd::utils::parse_space_id(&space_id);
            handle_add(config, path, space_id, entry_ids).await
        }
        WorktreeCommands::List { format } => handle_list(&format),
        WorktreeCommands::Remove { path, yes } => handle_remove(&path, yes),
        WorktreeCommands::Status => handle_status(),
        WorktreeCommands::Diff { format, remote } => handle_diff(&format, remote, config).await,
        WorktreeCommands::Commit { message, all } => handle_commit(&message, all),
        WorktreeCommands::Log { limit } => handle_log(limit),
        WorktreeCommands::Reset { commitish, hard } => handle_reset(&commitish, hard),
        WorktreeCommands::Pull => handle_pull(config).await,
        WorktreeCommands::Push { dry_run, force } => handle_push(config, dry_run, force).await,
        WorktreeCommands::Revert { commitish, dry_run } => {
            handle_revert(config, &commitish, dry_run).await
        }
    }
}

async fn handle_add(
    config: &Config,
    path: String,
    space_id: String,
    entry_ids: Option<String>,
) -> Result<()> {
    let worktree_path = PathBuf::from(&path);

    if worktree_path.exists() {
        anyhow::bail!("Directory already exists: {}", path);
    }

    let sp = ui::spinner("正在连接远端...");

    std::fs::create_dir_all(&worktree_path)?;

    let access_token = crate::auth::get_access_token(config).await?;
    let client = McpClient::new(&config.mcp.url, Some(access_token))?;

    let space_result: serde_json::Value = client
        .call_raw(
            "space_describe_space",
            serde_json::json!({ "space_id": space_id }),
        )
        .await?;

    let space_name = extract_space_name(&space_result, &space_id);
    let root_entry_id = extract_root_entry_id(&space_result)?;

    sp.set_message(format!("正在检出 {} ...", space_name));

    let mut wt_config = WorktreeConfig::new(space_id.clone(), space_name.clone());

    let config_dir = worktree_path.join(".lxworktree");
    std::fs::create_dir_all(&config_dir)?;

    sp.set_message("初始化 git 仓库...");
    let mut repo = Repository::init(&worktree_path)?;

    sp.set_message("正在拉取远端文件...");

    let entries_to_fetch: Vec<String> = if let Some(ids) = entry_ids {
        ids.split(',').map(|s| s.trim().to_string()).collect()
    } else {
        vec![]
    };

    let mut stats = worktree::PullStats::default();
    let mut entries_map = worktree::EntriesMap::new();

    if entries_to_fetch.is_empty() {
        pull_entries_recursive(
            &client,
            &root_entry_id,
            &worktree_path,
            "",
            &mut entries_map,
            &mut stats,
            None,
        )
        .await?;
    } else {
        for entry_id in &entries_to_fetch {
            let entry_result: serde_json::Value = client
                .call_raw(
                    "entry_describe_entry",
                    serde_json::json!({ "entry_id": entry_id }),
                )
                .await?;

            if let Some(entry_data) = entry_result.get("data").and_then(|d| d.get("entry")) {
                let entry_name = entry_data
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("untitled");
                let entry_type_str = entry_data
                    .get("entry_type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("page");
                let has_children = entry_data
                    .get("has_children")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);

                let entry_type = worktree::parse_entry_type(entry_type_str);
                let filename = worktree::entry_to_filename(entry_name, &entry_type);

                match entry_type {
                    worktree::EntryType::Folder => {
                        let local_path = worktree_path.join(&filename);
                        std::fs::create_dir_all(&local_path)?;
                        stats.folders_created += 1;

                        worktree::EntriesManager::add(
                            &mut entries_map,
                            filename.clone(),
                            entry_id.clone(),
                            entry_type,
                            None,
                        );

                        if has_children {
                            pull_entries_recursive(
                                &client,
                                entry_id,
                                &worktree_path,
                                &filename,
                                &mut entries_map,
                                &mut stats,
                                None,
                            )
                            .await?;
                        }
                    }
                    worktree::EntryType::Page => match pull_page_content(&client, entry_id).await {
                        Ok(content) => {
                            let local_path = worktree_path.join(&filename);
                            std::fs::write(&local_path, content)?;
                            stats.pages_pulled += 1;

                            worktree::EntriesManager::add(
                                &mut entries_map,
                                filename,
                                entry_id.clone(),
                                entry_type,
                                None,
                            );
                        }
                        Err(e) => {
                            stats.errors.push(format!("{}: {}", entry_id, e));
                        }
                    },
                    _ => {
                        stats.files_pulled += 1;
                    }
                }
            }
        }
    }

    worktree::EntriesManager::save(&worktree_path, &entries_map)?;

    let commit_message = format!(
        "Initial checkout from remote\n\nSpace: {} ({})\nRoot entry: {}\nFetched: {} folders, {} pages, {} files",
        space_name,
        space_id,
        root_entry_id,
        stats.folders_created,
        stats.pages_pulled,
        stats.files_pulled
    );
    let commit_id = repo.add_and_commit(&commit_message)?;
    wt_config.set_remote_snapshot(commit_id);

    wt_config.save(&worktree_path)?;

    sp.finish_and_clear();

    ui::print_header("Checked out", &format!("'{}'", path), &space_name);

    let record = WorktreeRecord {
        path: worktree_path.canonicalize()?.to_string_lossy().to_string(),
        space_id,
        space_name,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    let mut registry = WorktreeRegistry::load()?;
    registry.register(record)?;

    let canonical_path = worktree_path.canonicalize()?;
    ui::print_worktree_add_complete(
        stats.folders_created,
        stats.pages_pulled,
        stats.files_pulled,
        &stats.errors,
        &canonical_path.display().to_string(),
    );

    Ok(())
}

fn handle_list(format: &str) -> Result<()> {
    let registry = WorktreeRegistry::load()?;
    let worktrees = registry.list();

    match format {
        "json" => {
            ui::line(&serde_json::to_string_pretty(&worktrees)?);
        }
        _ => {
            let items: Vec<ui::WorktreeItem> = worktrees
                .iter()
                .map(|wt| ui::WorktreeItem {
                    path: &wt.path,
                    space_name: &wt.space_name,
                    space_id: &wt.space_id,
                    created_at: &wt.created_at,
                })
                .collect();
            ui::print_worktree_list(&items);
        }
    }

    Ok(())
}

fn handle_remove(path: &str, yes: bool) -> Result<()> {
    let worktree_path = PathBuf::from(&path);
    let canonical_path = worktree_path
        .canonicalize()
        .with_context(|| format!("Worktree not found: {}", path))?;
    let path_str = canonical_path.to_string_lossy().to_string();

    let mut registry = WorktreeRegistry::load()?;

    if registry.find_by_path(&path_str).is_none() {
        anyhow::bail!("Worktree not registered: {}", path);
    }

    if let Ok(repo) = Repository::open(&canonical_path) {
        if repo.has_uncommitted_changes()? && !yes {
            ui::warn("Warning: Worktree has uncommitted changes!");
        }
    }

    if !yes {
        ui::prompt(&format!("Remove worktree {}? [y/N] ", path));
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            ui::dim("Aborted.");
            return Ok(());
        }
    }

    std::fs::remove_dir_all(&canonical_path)
        .with_context(|| format!("Failed to remove directory: {}", path))?;

    registry.unregister(&path_str)?;

    ui::success(&format!("Worktree removed: {}", path));

    Ok(())
}

fn handle_status() -> Result<()> {
    let worktree_path = find_worktree_path()?;
    let repo = Repository::open(&worktree_path)?;
    let status = repo.status()?;

    ui::kv("Worktree", &worktree_path.display().to_string());
    ui::print_status(&ui::StatusOutput {
        staged: status.staged,
        modified: status.modified,
        deleted: status.deleted,
        untracked: status.untracked,
    });

    Ok(())
}

async fn handle_diff(format: &str, remote: bool, config: &Config) -> Result<()> {
    let worktree_path = find_worktree_path()?;
    let wt_config = WorktreeConfig::load(&worktree_path)?;

    if remote {
        ui::info("Comparing local with remote...");
        ui::print_header("Space", &wt_config.space_name, &wt_config.space_id);

        let access_token = crate::auth::get_access_token(config).await?;
        let client = McpClient::new(&config.mcp.url, Some(access_token))?;

        let entries_map = worktree::EntriesManager::load(&worktree_path)?;

        let mut added_local: Vec<String> = Vec::new();
        let mut modified: Vec<String> = Vec::new();
        let mut deleted_local: Vec<String> = Vec::new();

        for (local_path, entry_info) in &entries_map {
            let full_path = worktree_path.join(local_path);

            if entry_info.entry_type == worktree::EntryType::Folder {
                continue;
            }

            if !full_path.exists() {
                deleted_local.push(local_path.clone());
                continue;
            }

            if entry_info.entry_type != worktree::EntryType::Page {
                continue;
            }

            let local_content = std::fs::read_to_string(&full_path)?;

            match pull_page_content(&client, &entry_info.entry_id).await {
                Ok(remote_content) => {
                    if local_content.trim() != remote_content.trim() {
                        modified.push(local_path.clone());

                        if format == "full" {
                            ui::print_diff_header("remote", "local", local_path);
                            let remote_lines: Vec<&str> = remote_content.lines().collect();
                            let local_lines: Vec<&str> = local_content.lines().collect();
                            print_simple_diff(&remote_lines, &local_lines);
                        }
                    }
                }
                Err(e) => {
                    ui::warn(&format!("  Failed to fetch {}: {}", local_path, e));
                }
            }
        }

        for entry in walkdir::WalkDir::new(&worktree_path)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let full_path = entry.path();

            if full_path
                .components()
                .any(|c| c.as_os_str() == ".git" || c.as_os_str() == ".lxworktree")
            {
                continue;
            }

            let relative_path = full_path
                .strip_prefix(&worktree_path)?
                .to_string_lossy()
                .to_string();

            if !entries_map.contains_key(&relative_path) && relative_path.ends_with(".md") {
                added_local.push(relative_path);
            }
        }

        ui::print_remote_diff(&added_local, &modified, &deleted_local);
    } else {
        let repo = Repository::open(&worktree_path)?;
        let status = repo.status()?;
        ui::print_status(&ui::StatusOutput {
            staged: status.staged,
            modified: status.modified,
            deleted: status.deleted,
            untracked: status.untracked,
        });
    }

    Ok(())
}

fn handle_commit(message: &str, all: bool) -> Result<()> {
    let worktree_path = find_worktree_path()?;
    let mut repo = Repository::open(&worktree_path)?;

    let commit_id = if all {
        repo.add_and_commit(message)?
    } else {
        let status = repo.status()?;
        if status.staged.is_empty() {
            ui::dim("Nothing to commit. Use --all to stage all changes.");
            return Ok(());
        }
        repo.add_and_commit(message)?
    };

    ui::print_committed(&commit_id[..8]);

    Ok(())
}

fn handle_log(limit: usize) -> Result<()> {
    let worktree_path = find_worktree_path()?;
    let repo = Repository::open(&worktree_path)?;
    let commits = repo.log(Some(limit))?;

    for commit in commits {
        ui::print_log_entry(
            &commit.hash[..8],
            &commit.message,
            &commit.author,
            &commit.date,
        );
    }

    Ok(())
}

fn handle_reset(commitish: &str, hard: bool) -> Result<()> {
    let worktree_path = find_worktree_path()?;
    let mut repo = Repository::open(&worktree_path)?;

    if hard {
        ui::prompt("Hard reset will discard all working directory changes. Continue? [y/N] ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            ui::dim("Aborted.");
            return Ok(());
        }
    }

    repo.reset(commitish, hard)?;
    ui::print_reset_result(commitish, hard);

    Ok(())
}

async fn handle_pull(config: &Config) -> Result<()> {
    let worktree_path = find_worktree_path()?;
    let wt_config = WorktreeConfig::load(&worktree_path)?;

    let sp = ui::spinner("正在拉取...");
    ui::print_header("Pulling from", &wt_config.space_name, &wt_config.space_id);

    let access_token = crate::auth::get_access_token(config).await?;
    let client = McpClient::new(&config.mcp.url, Some(access_token))?;

    let space_result: serde_json::Value = client
        .call_raw(
            "space_describe_space",
            serde_json::json!({ "space_id": wt_config.space_id }),
        )
        .await?;

    let root_entry_id = extract_root_entry_id(&space_result)?;

    let mut entries_map = worktree::EntriesManager::load(&worktree_path)?;

    let mut stats = worktree::PullStats::default();
    pull_entries_recursive(
        &client,
        &root_entry_id,
        &worktree_path,
        "",
        &mut entries_map,
        &mut stats,
        Some(&sp),
    )
    .await?;

    worktree::EntriesManager::save(&worktree_path, &entries_map)?;

    let mut repo = Repository::open(&worktree_path)?;
    if repo.has_uncommitted_changes()? {
        let commit_message = format!(
            "Pull from remote\n\nSpace: {} ({})\nPulled: {} folders, {} pages, {} files",
            wt_config.space_name,
            wt_config.space_id,
            stats.folders_created,
            stats.pages_pulled,
            stats.files_pulled
        );
        let commit_id = repo.add_and_commit(&commit_message)?;
        sp.finish_and_clear();
        ui::print_committed(&commit_id[..8]);
    } else {
        sp.finish_and_clear();
    }

    ui::print_pull_stats(
        stats.folders_created,
        stats.pages_pulled,
        stats.files_pulled,
        &stats.errors,
    );

    Ok(())
}

async fn handle_push(config: &Config, dry_run: bool, force: bool) -> Result<()> {
    let worktree_path = find_worktree_path()?;
    let wt_config = WorktreeConfig::load(&worktree_path)?;

    ui::print_header("Pushing to", &wt_config.space_name, &wt_config.space_id);

    let repo = Repository::open(&worktree_path)?;
    if repo.has_uncommitted_changes()? && !force {
        anyhow::bail!(
            "You have uncommitted changes. Please commit first or use --force to skip this check."
        );
    }

    let access_token = crate::auth::get_access_token(config).await?;
    let client = McpClient::new(&config.mcp.url, Some(access_token))?;

    let space_result: serde_json::Value = client
        .call_raw(
            "space_describe_space",
            serde_json::json!({ "space_id": wt_config.space_id }),
        )
        .await?;

    let root_entry_id = extract_root_entry_id(&space_result)?;

    let mut entries_map = worktree::EntriesManager::load(&worktree_path)?;
    let mut stats = worktree::PushStats::default();

    let mut entry_id_to_old_path: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for (path, info) in &entries_map {
        entry_id_to_old_path.insert(info.entry_id.clone(), path.clone());
    }

    let mut current_files: std::collections::HashMap<String, PathBuf> =
        std::collections::HashMap::new();

    for entry in walkdir::WalkDir::new(&worktree_path)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let full_path = entry.path();
        if full_path
            .components()
            .any(|c| c.as_os_str() == ".git" || c.as_os_str() == ".lxworktree")
        {
            continue;
        }
        let relative_path = full_path
            .strip_prefix(&worktree_path)
            .unwrap()
            .to_string_lossy()
            .to_string();
        current_files.insert(relative_path, full_path.to_path_buf());
    }

    let mut to_rename: Vec<(String, String, String)> = Vec::new();
    let mut to_move: Vec<(String, String, String)> = Vec::new();
    let mut to_update: Vec<(String, String)> = Vec::new();
    let mut to_delete: Vec<(String, String)> = Vec::new();
    let mut to_create: Vec<String> = Vec::new();

    for (old_path, entry_info) in &entries_map {
        if entry_info.entry_type == worktree::EntryType::Folder {
            continue;
        }

        if current_files.contains_key(old_path) {
            to_update.push((entry_info.entry_id.clone(), old_path.clone()));
        } else {
            let old_filename = std::path::Path::new(old_path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            let mut found_new_path: Option<String> = None;

            for new_path in current_files.keys() {
                if entries_map.contains_key(new_path) {
                    continue;
                }

                let new_filename = std::path::Path::new(new_path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();

                let old_dir = std::path::Path::new(old_path)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let new_dir = std::path::Path::new(new_path)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();

                if old_filename == new_filename || old_dir == new_dir {
                    found_new_path = Some(new_path.clone());
                    break;
                }
            }

            if let Some(new_path) = found_new_path {
                let old_filename = std::path::Path::new(old_path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let new_filename = std::path::Path::new(&new_path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();

                let old_dir = std::path::Path::new(old_path)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let new_dir = std::path::Path::new(&new_path)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();

                if old_dir == new_dir && old_filename != new_filename {
                    to_rename.push((entry_info.entry_id.clone(), old_filename, new_filename));
                } else if old_dir != new_dir {
                    if old_filename != new_filename {
                        to_rename.push((
                            entry_info.entry_id.clone(),
                            old_filename,
                            new_filename.clone(),
                        ));
                    }
                    let new_parent_id = if new_dir.is_empty() {
                        root_entry_id.clone()
                    } else {
                        entries_map
                            .get(&new_dir)
                            .map(|info| info.entry_id.clone())
                            .unwrap_or_else(|| root_entry_id.clone())
                    };
                    to_move.push((entry_info.entry_id.clone(), old_path.clone(), new_parent_id));
                }

                to_update.push((entry_info.entry_id.clone(), new_path));
            } else {
                to_delete.push((entry_info.entry_id.clone(), old_path.clone()));
            }
        }
    }

    for path in current_files.keys() {
        if !entries_map.contains_key(path) {
            if path.starts_with(".lxworktree") {
                continue;
            }
            let is_move_target = to_update.iter().any(|(_, p)| p == path);
            if !is_move_target {
                to_create.push(path.clone());
            }
        }
    }

    let total_ops =
        to_rename.len() + to_move.len() + to_update.len() + to_delete.len() + to_create.len();
    let mut created_paths: Vec<String> = Vec::new();
    let mut updated_paths: Vec<String> = Vec::new();
    let mut deleted_paths: Vec<String> = Vec::new();

    // dry-run 模式
    if dry_run {
        ui::print_dry_run_header(total_ops, "");
        for (_id, old_name, new_name) in &to_rename {
            ui::print_dry_run_item("RENAME", &format!("{} -> {}", old_name, new_name));
        }
        for (_id, old_path, new_parent_id) in &to_move {
            ui::print_dry_run_item("MOVE", &format!("{} -> parent:{}", old_path, new_parent_id));
        }
        for (_id, path) in &to_update {
            ui::print_dry_run_item("UPDATE", path);
        }
        for (_id, path) in &to_delete {
            ui::print_dry_run_item("DELETE", path);
        }
        for path in &to_create {
            if path.ends_with(".md") {
                ui::print_dry_run_item("CREATE PAGE", path);
            } else {
                ui::print_dry_run_item("CREATE FILE", path);
            }
        }
        ui::print_dry_run_complete();
        return Ok(());
    }

    // 实际推送
    let pb = if total_ops > 0 {
        Some(ui::progress_bar(total_ops as u64, "推送中..."))
    } else {
        None
    };

    for (entry_id, old_name, new_name) in &to_rename {
        if let Some(ref pb) = pb {
            pb.set_message(format!("重命名 {}", truncate_path(old_name, 30)));
        }
        match rename_entry(&client, entry_id, new_name).await {
            Ok(_) => {
                stats.entries_updated += 1;
                updated_paths.push(format!("{} → {}", old_name, new_name));
            }
            Err(e) => {
                stats.errors.push(format!("rename {}: {}", old_name, e));
            }
        }
        if let Some(ref pb) = pb {
            pb.inc(1);
        }
    }

    for (entry_id, old_path, new_parent_id) in &to_move {
        if let Some(ref pb) = pb {
            pb.set_message(format!("移动 {}", truncate_path(old_path, 30)));
        }
        match move_entry(&client, entry_id, new_parent_id).await {
            Ok(_) => {
                stats.entries_updated += 1;
                updated_paths.push(format!("{} (moved)", old_path));
            }
            Err(e) => {
                stats.errors.push(format!("move {}: {}", old_path, e));
            }
        }
        if let Some(ref pb) = pb {
            pb.inc(1);
        }
    }

    for (entry_id, path) in &to_update {
        let full_path = worktree_path.join(path);
        if !full_path.exists() {
            if let Some(ref pb) = pb {
                pb.inc(1);
            }
            continue;
        }

        if let Some(ref pb) = pb {
            pb.set_message(truncate_path(path, 40));
        }

        if path.ends_with(".md") {
            let content = std::fs::read_to_string(&full_path)?;
            match push_page_content(&client, entry_id, &content).await {
                Ok(_) => {
                    stats.entries_updated += 1;
                    updated_paths.push(path.clone());
                }
                Err(e) => {
                    stats.errors.push(format!("{}: {}", path, e));
                }
            }
        } else {
            match update_file_content(&client, entry_id, &full_path).await {
                Ok(_) => {
                    stats.entries_updated += 1;
                    updated_paths.push(path.clone());
                }
                Err(e) => {
                    stats.errors.push(format!("{}: {}", path, e));
                }
            }
        }
        if let Some(ref pb) = pb {
            pb.inc(1);
        }
    }

    for (entry_id, path) in &to_delete {
        if let Some(ref pb) = pb {
            pb.set_message(format!("删除 {}", truncate_path(path, 30)));
        }
        match delete_entry(&client, entry_id).await {
            Ok(_) => {
                stats.entries_deleted += 1;
                deleted_paths.push(path.clone());
            }
            Err(e) => {
                stats.errors.push(format!("delete {}: {}", path, e));
            }
        }
        if let Some(ref pb) = pb {
            pb.inc(1);
        }
    }

    for path in &to_create {
        let full_path = worktree_path.join(path);
        let entry_name = full_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let parent_dir = full_path
            .parent()
            .and_then(|p| p.strip_prefix(&worktree_path).ok())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        if let Some(ref pb) = pb {
            pb.set_message(truncate_path(path, 40));
        }

        let parent_entry_id =
            match ensure_parent_folders(&client, &root_entry_id, &parent_dir, &mut entries_map)
                .await
            {
                Ok(id) => id,
                Err(e) => {
                    stats
                        .errors
                        .push(format!("{}: failed to create parent folder: {}", path, e));
                    if let Some(ref pb) = pb {
                        pb.inc(1);
                    }
                    continue;
                }
            };

        if path.ends_with(".md") {
            let page_name = full_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let content = std::fs::read_to_string(&full_path)?;

            match create_page_with_content(&client, &parent_entry_id, &page_name, &content).await {
                Ok(new_entry_id) => {
                    stats.entries_created += 1;
                    created_paths.push(path.clone());

                    worktree::EntriesManager::add(
                        &mut entries_map,
                        path.clone(),
                        new_entry_id,
                        worktree::EntryType::Page,
                        None,
                    );
                }
                Err(e) => {
                    stats.errors.push(format!("{}: {}", path, e));
                }
            }
        } else {
            match upload_new_file(&client, &parent_entry_id, &full_path, Some(&entry_name)).await {
                Ok(new_entry_id) => {
                    stats.entries_created += 1;
                    created_paths.push(path.clone());

                    worktree::EntriesManager::add(
                        &mut entries_map,
                        path.clone(),
                        new_entry_id,
                        worktree::EntryType::File,
                        None,
                    );
                }
                Err(e) => {
                    stats.errors.push(format!("{}: {}", path, e));
                }
            }
        }
        if let Some(ref pb) = pb {
            pb.inc(1);
        }
    }

    if let Some(pb) = pb {
        pb.finish_and_clear();
    }

    worktree::EntriesManager::save(&worktree_path, &entries_map)?;

    ui::print_push_stats(
        stats.entries_created,
        stats.entries_updated,
        stats.entries_deleted,
        &created_paths,
        &updated_paths,
        &deleted_paths,
        &stats.errors,
    );

    Ok(())
}

async fn handle_revert(config: &Config, commitish: &str, dry_run: bool) -> Result<()> {
    let worktree_path = find_worktree_path()?;
    let wt_config = WorktreeConfig::load(&worktree_path)?;
    let repo = worktree::Repository::open(&worktree_path)?;

    ui::print_header(
        "Reverting to",
        commitish,
        &format!("{} ({})", wt_config.space_name, wt_config.space_id),
    );

    let target_files = repo.get_commit_files(commitish)?;
    let target_paths: HashSet<String> = target_files.iter().map(|(p, _)| p.clone()).collect();

    let current_paths: HashSet<String> = repo.get_commit_file_paths("HEAD")?.into_iter().collect();

    let mut entries_map = worktree::EntriesManager::load(&worktree_path)?;

    let access_token = crate::auth::get_access_token(config).await?;
    let client = McpClient::new(&config.mcp.url, Some(access_token))?;

    let space_info: serde_json::Value = client
        .call_raw(
            "space_describe_space",
            serde_json::json!({ "space_id": wt_config.space_id }),
        )
        .await?;
    let root_entry_id = extract_root_entry_id(&space_info)?;

    let mut stats = worktree::PushStats::default();
    let mut updated_paths: Vec<String> = Vec::new();
    let mut created_paths: Vec<String> = Vec::new();
    let mut deleted_paths: Vec<String> = Vec::new();

    for (path, content) in &target_files {
        if path.starts_with(".lxworktree") {
            continue;
        }

        if let Some(entry_info) = entries_map.get(path) {
            if path.ends_with(".md") {
                let content_str = String::from_utf8_lossy(content);
                if dry_run {
                    ui::print_dry_run_item("REVERT", path);
                } else {
                    match push_page_content(&client, &entry_info.entry_id, &content_str).await {
                        Ok(_) => {
                            stats.entries_updated += 1;
                            updated_paths.push(path.clone());
                        }
                        Err(e) => {
                            stats.errors.push(format!("{}: {}", path, e));
                        }
                    }
                }
            } else if dry_run {
                ui::print_dry_run_item("REVERT FILE", path);
            } else {
                let temp_path = worktree_path.join(path);
                if let Some(parent) = temp_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&temp_path, content)?;

                match update_file_content(&client, &entry_info.entry_id, &temp_path).await {
                    Ok(_) => {
                        stats.entries_updated += 1;
                        updated_paths.push(path.clone());
                    }
                    Err(e) => {
                        stats.errors.push(format!("{}: {}", path, e));
                    }
                }
            }
        } else {
            let file_path = std::path::Path::new(path);
            let file_name = file_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            let parent_dir = file_path
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            let parent_entry_id = if parent_dir.is_empty() {
                root_entry_id.clone()
            } else {
                entries_map
                    .get(&parent_dir)
                    .map(|info| info.entry_id.clone())
                    .unwrap_or_else(|| root_entry_id.clone())
            };

            if dry_run {
                ui::print_dry_run_item("RECREATE", path);
            } else if path.ends_with(".md") {
                let page_name = file_path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let content_str = String::from_utf8_lossy(content);

                match create_page_with_content(&client, &parent_entry_id, &page_name, &content_str)
                    .await
                {
                    Ok(new_entry_id) => {
                        stats.entries_created += 1;
                        created_paths.push(path.clone());
                        worktree::EntriesManager::add(
                            &mut entries_map,
                            path.clone(),
                            new_entry_id,
                            worktree::EntryType::Page,
                            None,
                        );
                    }
                    Err(e) => {
                        stats.errors.push(format!("{}: {}", path, e));
                    }
                }
            } else {
                let temp_path = worktree_path.join(path);
                if let Some(parent) = temp_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&temp_path, content)?;

                match upload_new_file(&client, &parent_entry_id, &temp_path, Some(&file_name)).await
                {
                    Ok(new_entry_id) => {
                        stats.entries_created += 1;
                        created_paths.push(path.clone());
                        worktree::EntriesManager::add(
                            &mut entries_map,
                            path.clone(),
                            new_entry_id,
                            worktree::EntryType::File,
                            None,
                        );
                    }
                    Err(e) => {
                        stats.errors.push(format!("{}: {}", path, e));
                    }
                }
            }
        }
    }

    for path in &current_paths {
        if !target_paths.contains(path) && !path.starts_with(".lxworktree") {
            if let Some(entry_info) = entries_map.get(path) {
                if dry_run {
                    ui::print_dry_run_item("DELETE", path);
                } else {
                    match delete_entry(&client, &entry_info.entry_id).await {
                        Ok(_) => {
                            stats.entries_deleted += 1;
                            deleted_paths.push(path.clone());
                        }
                        Err(e) => {
                            stats.errors.push(format!("delete {}: {}", path, e));
                        }
                    }
                }
            }
        }
    }

    if !dry_run {
        worktree::EntriesManager::save(&worktree_path, &entries_map)?;
    }

    if dry_run {
        ui::print_dry_run_complete();
    } else {
        ui::print_push_stats(
            stats.entries_created,
            stats.entries_updated,
            stats.entries_deleted,
            &created_paths,
            &updated_paths,
            &deleted_paths,
            &stats.errors,
        );
    }

    Ok(())
}

fn print_simple_diff(old_lines: &[&str], new_lines: &[&str]) {
    use std::collections::HashSet;

    let old_set: HashSet<&str> = old_lines.iter().copied().collect();
    let new_set: HashSet<&str> = new_lines.iter().copied().collect();

    for line in old_lines {
        if !new_set.contains(line) && !line.trim().is_empty() {
            ui::print_diff_line("-", line);
        }
    }

    for line in new_lines {
        if !old_set.contains(line) && !line.trim().is_empty() {
            ui::print_diff_line("+", line);
        }
    }
}

async fn rename_entry(client: &McpClient, entry_id: &str, new_name: &str) -> Result<()> {
    let _result: serde_json::Value = client
        .call_raw(
            "entry_rename_entry",
            serde_json::json!({
                "entry_id": entry_id,
                "name": new_name
            }),
        )
        .await?;

    Ok(())
}

async fn move_entry(client: &McpClient, entry_id: &str, parent_entry_id: &str) -> Result<()> {
    let _result: serde_json::Value = client
        .call_raw(
            "entry_move_entry",
            serde_json::json!({
                "entry_id": entry_id,
                "parent_entry_id": parent_entry_id
            }),
        )
        .await?;

    Ok(())
}

async fn delete_entry(client: &McpClient, entry_id: &str) -> Result<()> {
    let _result: serde_json::Value = client
        .call_raw(
            "entry_delete_entry",
            serde_json::json!({
                "entry_id": entry_id
            }),
        )
        .await?;

    Ok(())
}

async fn create_folder(client: &McpClient, parent_entry_id: &str, name: &str) -> Result<String> {
    let result: serde_json::Value = client
        .call_raw(
            "entry_create_entry",
            serde_json::json!({
                "entry_type": "folder",
                "parent_entry_id": parent_entry_id,
                "name": name
            }),
        )
        .await?;

    let entry_id = result
        .get("data")
        .and_then(|d| d.get("entry"))
        .and_then(|e| e.get("id"))
        .and_then(|id| id.as_str())
        .ok_or_else(|| anyhow::anyhow!("Failed to get entry_id from create folder response"))?;

    Ok(entry_id.to_string())
}

/// 确保远程父文件夹存在，返回最终的 `parent_entry_id`
async fn ensure_parent_folders(
    client: &McpClient,
    root_entry_id: &str,
    parent_dir: &str,
    entries_map: &mut worktree::EntriesMap,
) -> Result<String> {
    if parent_dir.is_empty() {
        return Ok(root_entry_id.to_string());
    }

    // 如果已经存在，直接返回
    if let Some(info) = entries_map.get(parent_dir) {
        return Ok(info.entry_id.clone());
    }

    // 递归确保父文件夹存在
    let path = std::path::Path::new(parent_dir);
    let parent_parent = path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let folder_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let parent_entry_id = Box::pin(ensure_parent_folders(
        client,
        root_entry_id,
        &parent_parent,
        entries_map,
    ))
    .await?;

    // 创建当前文件夹
    let new_folder_id = create_folder(client, &parent_entry_id, &folder_name).await?;

    worktree::EntriesManager::add(
        entries_map,
        parent_dir.to_string(),
        new_folder_id.clone(),
        worktree::EntryType::Folder,
        None,
    );

    Ok(new_folder_id)
}
