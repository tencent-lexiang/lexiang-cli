use crate::config::Config;
use crate::mcp::McpClient;
use crate::worktree::{self, Repository, WorktreeConfig, WorktreeRecord, WorktreeRegistry};
use anyhow::Result;
use std::collections::HashSet;
use std::path::PathBuf;

mod workspace;

pub use workspace::handle_workspace_command;

use super::cli::GitCommands;

pub async fn handle_git_command(command: GitCommands, config: &Config) -> Result<()> {
    match command {
        GitCommands::Clone { space_id, path } => {
            let space_id = crate::cmd::utils::parse_space_id(&space_id);
            handle_clone(config, space_id, path).await
        }
        GitCommands::Add { pathspec } => handle_add(&pathspec),
        GitCommands::Commit { message, all } => handle_commit(&message, all),
        GitCommands::Status => handle_status(),
        GitCommands::Diff { remote } => handle_diff(remote),
        GitCommands::Log { max_count } => handle_log(max_count),
        GitCommands::Pull => handle_pull(config).await,
        GitCommands::Push { dry_run, force } => handle_push(config, dry_run, force).await,
        GitCommands::Reset { commit, hard } => handle_reset(&commit, hard),
        GitCommands::Revert { commit, dry_run } => handle_revert(config, &commit, dry_run).await,
        GitCommands::Remote { verbose } => handle_remote(verbose),
    }
}

async fn handle_clone(config: &Config, space_id: String, path: String) -> Result<()> {
    let worktree_path = PathBuf::from(&path);

    if worktree_path.exists() {
        anyhow::bail!("Directory already exists: {}", path);
    }

    println!("Cloning into '{}'...", path);

    std::fs::create_dir_all(&worktree_path)?;

    let access_token = crate::auth::get_access_token(config).await?;
    let client = McpClient::new(&config.mcp.url, Some(access_token))?;

    let space_info: serde_json::Value = client
        .call_raw(
            "space_describe_space",
            serde_json::json!({ "space_id": space_id }),
        )
        .await?;

    let root_entry_id = extract_root_entry_id(&space_info)?;
    let space_name = extract_space_name(&space_info, &space_id);

    println!("Space: {} ({})", space_name, space_id);

    let mut repo = Repository::init(&worktree_path)?;

    let lxworktree_dir = worktree_path.join(".lxworktree");
    std::fs::create_dir_all(&lxworktree_dir)?;

    let wt_config = WorktreeConfig::new(space_id.clone(), space_name.clone());
    wt_config.save(&worktree_path)?;

    let entries_map = std::collections::HashMap::new();
    worktree::EntriesManager::save(&worktree_path, &entries_map)?;

    println!("Fetching content...");
    let mut entries_map = entries_map;
    let mut stats = worktree::PullStats::default();

    pull_entries_recursive(
        &client,
        &root_entry_id,
        &worktree_path,
        "",
        &mut entries_map,
        &mut stats,
    )
    .await?;

    worktree::EntriesManager::save(&worktree_path, &entries_map)?;

    let commit_message = format!(
        "Initial clone from remote\n\nSpace: {} ({})\nRoot entry: {}\nFetched: {} folders, {} pages, {} files",
        space_name,
        space_id,
        root_entry_id,
        stats.folders_created,
        stats.pages_pulled,
        stats.files_pulled
    );
    repo.add_and_commit(&commit_message)?;

    let mut registry = WorktreeRegistry::load()?;
    registry.register(WorktreeRecord {
        path: worktree_path.canonicalize()?.to_string_lossy().to_string(),
        space_id,
        space_name,
        created_at: chrono::Utc::now().to_rfc3339(),
    })?;

    println!();
    println!(
        "Cloned: {} folders, {} pages, {} files",
        stats.folders_created, stats.pages_pulled, stats.files_pulled
    );

    Ok(())
}

fn handle_add(pathspec: &str) -> Result<()> {
    let worktree_path = find_worktree_path()?;
    let repo = Repository::open(&worktree_path)?;

    if pathspec == "." {
        println!("Changes staged for commit.");
        println!("(use \"lx git commit -m <message>\" to commit)");
    } else {
        println!("Staged: {}", pathspec);
    }

    let status = repo.status()?;
    if !status.untracked.is_empty() {
        println!("\nNew files:");
        for f in &status.untracked {
            println!("  {}", f);
        }
    }
    if !status.modified.is_empty() {
        println!("\nModified:");
        for f in &status.modified {
            println!("  {}", f);
        }
    }

    Ok(())
}

fn handle_commit(message: &str, _all: bool) -> Result<()> {
    let worktree_path = find_worktree_path()?;
    let mut repo = Repository::open(&worktree_path)?;

    let commit_id = repo.add_and_commit(message)?;
    println!("[master {}] {}", &commit_id[..7], message);

    Ok(())
}

fn handle_status() -> Result<()> {
    let worktree_path = find_worktree_path()?;
    let wt_config = WorktreeConfig::load(&worktree_path)?;
    let repo = Repository::open(&worktree_path)?;

    println!("On branch master");
    println!("Remote: {} ({})", wt_config.space_name, wt_config.space_id);
    println!();

    let status = repo.status()?;

    if status.staged.is_empty()
        && status.modified.is_empty()
        && status.deleted.is_empty()
        && status.untracked.is_empty()
    {
        println!("nothing to commit, working tree clean");
    } else {
        if !status.modified.is_empty() {
            println!("Changes not staged for commit:");
            println!("  (use \"lx git add <file>...\" to update what will be committed)");
            println!();
            for f in &status.modified {
                println!("        modified:   {}", f);
            }
            println!();
        }
        if !status.deleted.is_empty() {
            println!("Deleted files:");
            for f in &status.deleted {
                println!("        deleted:    {}", f);
            }
            println!();
        }
        if !status.untracked.is_empty() {
            println!("Untracked files:");
            println!("  (use \"lx git add <file>...\" to include in what will be committed)");
            println!();
            for f in &status.untracked {
                println!("        {}", f);
            }
        }
    }

    Ok(())
}

fn handle_diff(remote: bool) -> Result<()> {
    let worktree_path = find_worktree_path()?;
    let repo = Repository::open(&worktree_path)?;

    if remote {
        println!("Comparing with remote is not yet implemented.");
        println!("Use 'lx git pull' to fetch latest changes first.");
    } else {
        let status = repo.status()?;
        if status.modified.is_empty() && status.untracked.is_empty() && status.deleted.is_empty() {
            println!("No changes.");
        } else {
            for f in &status.modified {
                println!("M  {}", f);
            }
            for f in &status.untracked {
                println!("?? {}", f);
            }
            for f in &status.deleted {
                println!("D  {}", f);
            }
        }
    }

    Ok(())
}

fn handle_log(max_count: usize) -> Result<()> {
    let worktree_path = find_worktree_path()?;
    let repo = Repository::open(&worktree_path)?;

    let commits = repo.log(Some(max_count))?;

    for commit in commits {
        println!("commit {}", commit.hash);
        println!("Author: {}", commit.author);
        println!("Date:   {}", commit.date);
        println!();
        println!("    {}", commit.message.trim());
        println!();
    }

    Ok(())
}

async fn handle_pull(config: &Config) -> Result<()> {
    let worktree_path = find_worktree_path()?;
    let wt_config = WorktreeConfig::load(&worktree_path)?;
    let mut repo = Repository::open(&worktree_path)?;

    println!("Pulling from remote...");
    println!("Space: {} ({})", wt_config.space_name, wt_config.space_id);

    let access_token = crate::auth::get_access_token(config).await?;
    let client = McpClient::new(&config.mcp.url, Some(access_token))?;

    let space_info: serde_json::Value = client
        .call_raw(
            "space_describe_space",
            serde_json::json!({ "space_id": wt_config.space_id }),
        )
        .await?;
    let root_entry_id = extract_root_entry_id(&space_info)?;

    let mut entries_map = worktree::EntriesManager::load(&worktree_path)?;

    let mut stats = worktree::PullStats::default();
    pull_entries_recursive(
        &client,
        &root_entry_id,
        &worktree_path,
        "",
        &mut entries_map,
        &mut stats,
    )
    .await?;

    worktree::EntriesManager::save(&worktree_path, &entries_map)?;

    let commit_message = format!(
        "Pull from remote\n\nSpace: {} ({})\nPulled: {} folders, {} pages, {} files",
        wt_config.space_name,
        wt_config.space_id,
        stats.folders_created,
        stats.pages_pulled,
        stats.files_pulled
    );
    let commit_id = repo.add_and_commit(&commit_message)?;
    println!("Committed: {}", &commit_id[..8]);

    println!();
    println!(
        "Pull complete: {} folders, {} pages, {} files",
        stats.folders_created, stats.pages_pulled, stats.files_pulled
    );

    Ok(())
}

async fn handle_push(config: &Config, dry_run: bool, force: bool) -> Result<()> {
    let worktree_path = find_worktree_path()?;
    let wt_config = WorktreeConfig::load(&worktree_path)?;
    let repo = Repository::open(&worktree_path)?;

    if !force && repo.has_uncommitted_changes()? {
        anyhow::bail!(
            "You have uncommitted changes. Please commit first with 'lx git commit -a -m <message>'"
        );
    }

    println!("Pushing to remote...");
    println!("Space: {} ({})", wt_config.space_name, wt_config.space_id);

    let access_token = crate::auth::get_access_token(config).await?;
    let client = McpClient::new(&config.mcp.url, Some(access_token))?;

    let space_info: serde_json::Value = client
        .call_raw(
            "space_describe_space",
            serde_json::json!({ "space_id": wt_config.space_id }),
        )
        .await?;
    let root_entry_id = extract_root_entry_id(&space_info)?;

    let mut entries_map = worktree::EntriesManager::load(&worktree_path)?;

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
            .unwrap_or(full_path)
            .to_string_lossy()
            .to_string();
        current_files.insert(relative_path, full_path.to_path_buf());
    }

    let mut to_update: Vec<(String, String)> = Vec::new();
    let mut to_create: Vec<String> = Vec::new();

    for (path, entry_info) in &entries_map {
        if entry_info.entry_type == worktree::EntryType::Folder {
            continue;
        }
        if current_files.contains_key(path) {
            to_update.push((entry_info.entry_id.clone(), path.clone()));
        }
    }

    for path in current_files.keys() {
        if !entries_map.contains_key(path) && !path.starts_with(".lxworktree") {
            to_create.push(path.clone());
        }
    }

    let mut stats = worktree::PushStats::default();

    for (entry_id, path) in &to_update {
        let full_path = worktree_path.join(path);
        if !full_path.exists() {
            continue;
        }

        if path.ends_with(".md") {
            let content = std::fs::read_to_string(&full_path)?;
            if dry_run {
                println!("  [UPDATE] {}", path);
            } else {
                match push_page_content(&client, entry_id, &content).await {
                    Ok(_) => {
                        println!("  Updated: {}", path);
                        stats.entries_updated += 1;
                    }
                    Err(e) => stats.errors.push(format!("{}: {}", path, e)),
                }
            }
        } else if dry_run {
            println!("  [UPDATE FILE] {}", path);
        } else {
            match update_file_content(&client, entry_id, &full_path).await {
                Ok(_) => {
                    println!("  Updated: {}", path);
                    stats.entries_updated += 1;
                }
                Err(e) => stats.errors.push(format!("{}: {}", path, e)),
            }
        }
    }

    for path in &to_create {
        let full_path = worktree_path.join(path);
        let file_name = full_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let parent_dir = full_path
            .parent()
            .and_then(|p| p.strip_prefix(&worktree_path).ok())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        // 确保父文件夹在远程存在
        let parent_entry_id = if dry_run {
            if parent_dir.is_empty() {
                root_entry_id.clone()
            } else {
                entries_map
                    .get(&parent_dir)
                    .map(|info| info.entry_id.clone())
                    .unwrap_or_else(|| root_entry_id.clone())
            }
        } else {
            match ensure_parent_folders(&client, &root_entry_id, &parent_dir, &mut entries_map)
                .await
            {
                Ok(id) => id,
                Err(e) => {
                    stats
                        .errors
                        .push(format!("{}: failed to create parent folder: {}", path, e));
                    continue;
                }
            }
        };

        if path.ends_with(".md") {
            let page_name = full_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let content = std::fs::read_to_string(&full_path)?;

            if dry_run {
                println!("  [CREATE] {}", path);
            } else {
                match create_page_with_content(&client, &parent_entry_id, &page_name, &content)
                    .await
                {
                    Ok(new_id) => {
                        println!("  Created: {}", path);
                        stats.entries_created += 1;
                        worktree::EntriesManager::add(
                            &mut entries_map,
                            path.clone(),
                            new_id,
                            worktree::EntryType::Page,
                            None,
                        );
                    }
                    Err(e) => stats.errors.push(format!("{}: {}", path, e)),
                }
            }
        } else if dry_run {
            println!("  [CREATE FILE] {}", path);
        } else {
            match upload_new_file(&client, &parent_entry_id, &full_path, Some(&file_name)).await {
                Ok(new_id) => {
                    println!("  Uploaded: {}", path);
                    stats.entries_created += 1;
                    worktree::EntriesManager::add(
                        &mut entries_map,
                        path.clone(),
                        new_id,
                        worktree::EntryType::File,
                        None,
                    );
                }
                Err(e) => stats.errors.push(format!("{}: {}", path, e)),
            }
        }
    }

    if !dry_run {
        worktree::EntriesManager::save(&worktree_path, &entries_map)?;
    }

    println!();
    if dry_run {
        println!("Dry run complete. No changes made.");
    } else {
        println!(
            "Push complete: {} created, {} updated",
            stats.entries_created, stats.entries_updated
        );
        if !stats.errors.is_empty() {
            println!("Errors: {}", stats.errors.len());
            for e in &stats.errors {
                eprintln!("  - {}", e);
            }
        }
    }

    Ok(())
}

fn handle_reset(commit: &str, hard: bool) -> Result<()> {
    let worktree_path = find_worktree_path()?;
    let mut repo = Repository::open(&worktree_path)?;

    repo.reset(commit, hard)?;

    if hard {
        println!("HEAD is now at {}", commit);
    } else {
        println!("Unstaged changes after reset:");
        let status = repo.status()?;
        for f in status.modified {
            println!("M  {}", f);
        }
    }

    Ok(())
}

async fn handle_revert(config: &Config, commit: &str, dry_run: bool) -> Result<()> {
    let worktree_path = find_worktree_path()?;
    let wt_config = WorktreeConfig::load(&worktree_path)?;
    let repo = Repository::open(&worktree_path)?;

    println!("Reverting remote to: {}", commit);
    println!("Space: {} ({})", wt_config.space_name, wt_config.space_id);
    println!();

    let target_files = repo.get_commit_files(commit)?;
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

    for (path, content) in &target_files {
        if path.starts_with(".lxworktree") {
            continue;
        }

        if let Some(entry_info) = entries_map.get(path) {
            if path.ends_with(".md") {
                let content_str = String::from_utf8_lossy(content);
                if dry_run {
                    println!("  [REVERT] {}", path);
                } else {
                    match push_page_content(&client, &entry_info.entry_id, &content_str).await {
                        Ok(_) => {
                            println!("  Reverted: {}", path);
                            stats.entries_updated += 1;
                        }
                        Err(e) => stats.errors.push(format!("{}: {}", path, e)),
                    }
                }
            } else if dry_run {
                println!("  [REVERT FILE] {}", path);
            } else {
                let temp_path = worktree_path.join(path);
                if let Some(parent) = temp_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&temp_path, content)?;

                match update_file_content(&client, &entry_info.entry_id, &temp_path).await {
                    Ok(_) => {
                        println!("  Reverted: {}", path);
                        stats.entries_updated += 1;
                    }
                    Err(e) => stats.errors.push(format!("{}: {}", path, e)),
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
                println!("  [RECREATE] {}", path);
            } else if path.ends_with(".md") {
                let page_name = file_path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let content_str = String::from_utf8_lossy(content);

                match create_page_with_content(&client, &parent_entry_id, &page_name, &content_str)
                    .await
                {
                    Ok(new_id) => {
                        println!("  Recreated: {}", path);
                        stats.entries_created += 1;
                        worktree::EntriesManager::add(
                            &mut entries_map,
                            path.clone(),
                            new_id,
                            worktree::EntryType::Page,
                            None,
                        );
                    }
                    Err(e) => stats.errors.push(format!("{}: {}", path, e)),
                }
            } else {
                let temp_path = worktree_path.join(path);
                if let Some(parent) = temp_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&temp_path, content)?;

                match upload_new_file(&client, &parent_entry_id, &temp_path, Some(&file_name)).await
                {
                    Ok(new_id) => {
                        println!("  Recreated: {}", path);
                        stats.entries_created += 1;
                        worktree::EntriesManager::add(
                            &mut entries_map,
                            path.clone(),
                            new_id,
                            worktree::EntryType::File,
                            None,
                        );
                    }
                    Err(e) => stats.errors.push(format!("{}: {}", path, e)),
                }
            }
        }
    }

    for path in &current_paths {
        if !target_paths.contains(path) && !path.starts_with(".lxworktree") {
            if let Some(entry_info) = entries_map.get(path) {
                if dry_run {
                    println!("  [DELETE] {}", path);
                } else {
                    match delete_entry(&client, &entry_info.entry_id).await {
                        Ok(_) => {
                            println!("  Deleted: {}", path);
                            stats.entries_deleted += 1;
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

    println!();
    if dry_run {
        println!("Dry run complete.");
    } else {
        println!(
            "Revert complete: {} created, {} updated",
            stats.entries_created, stats.entries_updated
        );
        if !stats.errors.is_empty() {
            for e in &stats.errors {
                eprintln!("  - {}", e);
            }
        }
    }

    Ok(())
}

fn handle_remote(verbose: bool) -> Result<()> {
    let worktree_path = find_worktree_path()?;
    let wt_config = WorktreeConfig::load(&worktree_path)?;

    if verbose {
        println!(
            "origin\thttps://lexiangla.com/spaces/{} (fetch)",
            wt_config.space_id
        );
        println!(
            "origin\thttps://lexiangla.com/spaces/{} (push)",
            wt_config.space_id
        );
    } else {
        println!("origin");
    }

    Ok(())
}

// Helper functions

pub fn find_worktree_path() -> Result<PathBuf> {
    let mut current = std::env::current_dir()?;

    loop {
        let lxworktree = current.join(".lxworktree");
        if lxworktree.exists() {
            return Ok(current);
        }

        if !current.pop() {
            break;
        }
    }

    anyhow::bail!("Not in a worktree. Please run this command inside a worktree directory.")
}

fn extract_root_entry_id(response: &serde_json::Value) -> Result<String> {
    response
        .get("data")
        .and_then(|d| {
            d.get("root_entry_id")
                .or_else(|| d.get("space").and_then(|s| s.get("root_entry_id")))
        })
        .or_else(|| response.get("space").and_then(|s| s.get("root_entry_id")))
        .or_else(|| response.get("root_entry_id"))
        .and_then(|r| r.as_str())
        .map(std::string::ToString::to_string)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Failed to get root_entry_id from response: {}",
                serde_json::to_string_pretty(response).unwrap_or_default()
            )
        })
}

fn extract_space_name(response: &serde_json::Value, default: &str) -> String {
    response
        .get("data")
        .and_then(|d| {
            d.get("name")
                .or_else(|| d.get("space").and_then(|s| s.get("name")))
        })
        .or_else(|| response.get("space").and_then(|s| s.get("name")))
        .or_else(|| response.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or(default)
        .to_string()
}

#[async_recursion::async_recursion]
async fn pull_entries_recursive(
    client: &McpClient,
    parent_id: &str,
    worktree_path: &std::path::Path,
    relative_dir: &str,
    entries_map: &mut worktree::EntriesMap,
    stats: &mut worktree::PullStats,
) -> Result<()> {
    let result: serde_json::Value = client
        .call_raw(
            "entry_list_children",
            serde_json::json!({ "parent_id": parent_id }),
        )
        .await?;

    let entries = result
        .get("data")
        .and_then(|d| d.get("entries"))
        .and_then(|e| e.as_array())
        .map(|a| a.to_vec())
        .unwrap_or_default();

    for entry in entries {
        let entry_id = entry.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        let entry_name = entry
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let entry_type_str = entry
            .get("entry_type")
            .and_then(|v| v.as_str())
            .unwrap_or("page");
        let has_children = entry
            .get("has_children")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        let entry_type = worktree::parse_entry_type(entry_type_str);
        let filename = worktree::entry_to_filename(entry_name, &entry_type);

        let local_relative_path = if relative_dir.is_empty() {
            filename.clone()
        } else {
            format!("{}/{}", relative_dir, filename)
        };

        let local_path = worktree_path.join(&local_relative_path);

        match entry_type {
            worktree::EntryType::Folder => {
                if !local_path.exists() {
                    std::fs::create_dir_all(&local_path)?;
                    stats.folders_created += 1;
                }

                worktree::EntriesManager::add(
                    entries_map,
                    local_relative_path.clone(),
                    entry_id.to_string(),
                    entry_type,
                    None,
                );

                if has_children {
                    pull_entries_recursive(
                        client,
                        entry_id,
                        worktree_path,
                        &local_relative_path,
                        entries_map,
                        stats,
                    )
                    .await?;
                }
            }
            worktree::EntryType::Page => match pull_page_content(client, entry_id).await {
                Ok(content) => {
                    if let Some(parent) = local_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&local_path, content)?;
                    stats.pages_pulled += 1;

                    worktree::EntriesManager::add(
                        entries_map,
                        local_relative_path,
                        entry_id.to_string(),
                        entry_type,
                        None,
                    );
                }
                Err(e) => {
                    stats.errors.push(format!("{}: {}", local_relative_path, e));
                }
            },
            worktree::EntryType::File | worktree::EntryType::Smartsheet => {
                match download_file(client, entry_id, &local_path).await {
                    Ok(_) => {
                        worktree::EntriesManager::add(
                            entries_map,
                            local_relative_path.clone(),
                            entry_id.to_string(),
                            entry_type,
                            None,
                        );
                        stats.files_pulled += 1;
                    }
                    Err(e) => {
                        stats.errors.push(format!("{}: {}", local_relative_path, e));
                    }
                }
            }
        }
    }

    Ok(())
}

async fn pull_page_content(client: &McpClient, entry_id: &str) -> Result<String> {
    let result: serde_json::Value = client
        .call_raw(
            "entry_describe_ai_parse_content",
            serde_json::json!({ "entry_id": entry_id }),
        )
        .await?;

    if let Some(data) = result.get("data") {
        if let Some(content) = data
            .get("markdown")
            .and_then(|m| m.as_str())
            .or_else(|| data.get("html").and_then(|h| h.as_str()))
            .or_else(|| data.get("content").and_then(|c| c.as_str()))
            .or_else(|| data.get("text").and_then(|t| t.as_str()))
        {
            if !content.is_empty() {
                return Ok(content.to_string());
            }
        }
    }

    let entry_result: serde_json::Value = client
        .call_raw(
            "entry_describe_entry",
            serde_json::json!({ "entry_id": entry_id }),
        )
        .await?;

    let target_id = entry_result
        .get("data")
        .and_then(|d| d.get("entry"))
        .and_then(|e| e.get("target_id"))
        .and_then(|t| t.as_str());

    if let Some(block_id) = target_id {
        let blocks_result: serde_json::Value = client
            .call_raw(
                "block_list_block_children",
                serde_json::json!({
                    "block_id": block_id,
                    "recursive": true
                }),
            )
            .await?;

        if let Some(blocks) = blocks_result
            .get("data")
            .and_then(|d| d.get("blocks"))
            .and_then(|b| b.as_array())
        {
            let content = blocks_to_markdown(blocks);
            if !content.is_empty() {
                return Ok(content);
            }
        }
    }

    Ok(String::new())
}

fn blocks_to_markdown(blocks: &[serde_json::Value]) -> String {
    let mut lines = Vec::new();

    for block in blocks {
        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let content = block
            .get("content")
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .or_else(|| block.get("text").and_then(|t| t.as_str()))
            .unwrap_or("");

        let line = match block_type {
            "h1" => format!("# {}", content),
            "h2" => format!("## {}", content),
            "h3" => format!("### {}", content),
            "h4" => format!("#### {}", content),
            "h5" => format!("##### {}", content),
            "code" => {
                let lang = block
                    .get("content")
                    .and_then(|c| c.get("language"))
                    .and_then(|l| l.as_str())
                    .unwrap_or("");
                format!("```{}\n{}\n```", lang, content)
            }
            "bullet_list" | "list_item" => format!("- {}", content),
            "numbered_list" => format!("1. {}", content),
            "quote" | "blockquote" => format!("> {}", content),
            "divider" => "---".to_string(),
            // "paragraph", "text", "" and others
            _ => content.to_string(),
        };

        if !line.is_empty() {
            lines.push(line);
        }
    }

    lines.join("\n\n")
}

async fn push_page_content(client: &McpClient, entry_id: &str, content: &str) -> Result<()> {
    let _result: serde_json::Value = client
        .call_raw(
            "entry_import_content_to_entry",
            serde_json::json!({
                "entry_id": entry_id,
                "content": content,
                "content_type": "markdown",
                "force_write": true
            }),
        )
        .await?;

    Ok(())
}

async fn create_page_with_content(
    client: &McpClient,
    parent_entry_id: &str,
    name: &str,
    content: &str,
) -> Result<String> {
    let result: serde_json::Value = client
        .call_raw(
            "entry_create_entry",
            serde_json::json!({
                "entry_type": "page",
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
        .ok_or_else(|| anyhow::anyhow!("Failed to get entry_id from create response"))?;

    if !content.is_empty() {
        push_page_content(client, entry_id, content).await?;
    }

    Ok(entry_id.to_string())
}

async fn upload_new_file(
    client: &McpClient,
    parent_entry_id: &str,
    file_path: &std::path::Path,
    file_name: Option<&str>,
) -> Result<String> {
    let config = crate::mcp::UploadConfig {
        file_id: None,
        parent_entry_id: parent_entry_id.to_string(),
        file_name: file_name.map(std::string::ToString::to_string),
        content_type: None,
    };

    client.upload_file(&config, file_path).await
}

async fn update_file_content(
    client: &McpClient,
    entry_id: &str,
    file_path: &std::path::Path,
) -> Result<()> {
    let entry_info: serde_json::Value = client
        .call_raw(
            "entry_describe_entry",
            serde_json::json!({ "entry_id": entry_id }),
        )
        .await?;

    let file_id = entry_info
        .pointer("/data/entry/target_id")
        .or_else(|| entry_info.pointer("/data/target_id"))
        .or_else(|| entry_info.get("target_id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!("Failed to get file_id (target_id) for entry {}", entry_id)
        })?;

    let config = crate::mcp::UploadConfig {
        file_id: Some(file_id.to_string()),
        parent_entry_id: entry_id.to_string(),
        file_name: None,
        content_type: None,
    };

    client.upload_file(&config, file_path).await?;
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

async fn download_file(
    client: &McpClient,
    entry_id: &str,
    local_path: &std::path::Path,
) -> Result<()> {
    // 先获取 entry 详情拿到 file_id (target_id)
    let entry_info: serde_json::Value = client
        .call_raw(
            "entry_describe_entry",
            serde_json::json!({ "entry_id": entry_id }),
        )
        .await?;

    let file_id = entry_info
        .pointer("/data/entry/target_id")
        .or_else(|| entry_info.pointer("/data/target_id"))
        .or_else(|| entry_info.get("target_id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!("Failed to get file_id (target_id) for entry {}", entry_id)
        })?;

    // 调用 file_download_file 获取下载 URL
    let download_result: serde_json::Value = client
        .call_raw(
            "file_download_file",
            serde_json::json!({
                "file_id": file_id,
                "expire_seconds": 3600
            }),
        )
        .await?;

    let download_url = download_result
        .pointer("/data/url")
        .or_else(|| download_result.get("url"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Failed to get download URL for file {}", file_id))?;

    // 下载文件内容
    let response = reqwest::get(download_url).await?;
    if !response.status().is_success() {
        anyhow::bail!(
            "Failed to download file: HTTP {}",
            response.status().as_u16()
        );
    }

    let bytes = response.bytes().await?;

    // 确保父目录存在
    if let Some(parent) = local_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(local_path, &bytes)?;

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
