//! Git 仓库封装 (gitoxide)
//!
//! 封装 gitoxide 操作，提供简化的 Git 仓库管理接口。
//!
//! 注意：所有 Git 操作必须通过 gix API 完成，禁止任何 `git` CLI 调用。
//! 原因：运行环境不一定安装了 git，必须保证零依赖开箱即用。

use anyhow::{Context, Result};
use gix::bstr::BStr;
use std::path::{Path, PathBuf};

/// Git 仓库封装
pub struct Repository {
    /// 仓库路径
    path: PathBuf,
}

impl Repository {
    /// 在指定路径初始化新仓库
    pub fn init(path: &Path) -> Result<Self> {
        gix::init(path)
            .with_context(|| format!("Failed to init git repo at {}", path.display()))?;

        // 安装 git hooks 防止 agent 误操作
        Self::install_hooks(path)?;

        // 设置无效 remote，防止直接 git push
        Self::setup_fake_remote(path)?;

        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    /// 安装 git hooks 防止误操作
    fn install_hooks(path: &Path) -> Result<()> {
        let hooks_dir = path.join(".git").join("hooks");
        std::fs::create_dir_all(&hooks_dir)?;

        // pre-push hook: 阻止直接 git push
        let pre_push_hook = hooks_dir.join("pre-push");
        let pre_push_content = r#"#!/bin/sh
# 此 hook 由 lx 自动生成，用于防止直接使用 git push
#
# 乐享知识库工作区必须使用 lx git push 命令推送变更
# 直接使用 git push 不会同步到乐享知识库

echo ""
echo "⚠️  请使用 'lx git push' 推送到乐享知识库"
echo ""
echo "    直接使用 git push 不会同步变更到知识库。"
echo "    此仓库是乐享知识库的本地工作区，请使用："
echo ""
echo "        lx git push          # 推送到乐享知识库"
echo "        lx git push --dry-run # 预览推送内容"
echo ""
exit 1
"#;
        std::fs::write(&pre_push_hook, pre_push_content)?;

        // 设置可执行权限 (Unix)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&pre_push_hook)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&pre_push_hook, perms)?;
        }

        // pre-commit hook: 提示使用 lx git commit
        let pre_commit_hook = hooks_dir.join("pre-commit");
        let pre_commit_content = r#"#!/bin/sh
# 此 hook 由 lx 自动生成
#
# 提示：建议使用 lx git commit 提交变更

# 允许提交，仅打印提示
echo ""
echo "💡 提示: 推送到乐享知识库请使用 'lx git push'"
echo ""
"#;
        std::fs::write(&pre_commit_hook, pre_commit_content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&pre_commit_hook)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&pre_commit_hook, perms)?;
        }

        Ok(())
    }

    /// 设置无效 remote，防止直接 git push 成功
    fn setup_fake_remote(path: &Path) -> Result<()> {
        let config_path = path.join(".git").join("config");

        // 追加 remote 配置
        let remote_config = r#"
[remote "origin"]
	url = lx://use-lx-git-push-to-sync-with-lexiang
	fetch = +refs/heads/*:refs/remotes/origin/*
"#;

        let mut config_content = std::fs::read_to_string(&config_path).unwrap_or_default();
        config_content.push_str(remote_config);
        std::fs::write(&config_path, config_content)?;

        Ok(())
    }

    /// 打开已存在的仓库
    pub fn open(path: &Path) -> Result<Self> {
        gix::open(path)
            .with_context(|| format!("Failed to open git repo at {}", path.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    /// 获取仓库路径
    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 获取 gix 仓库
    fn gix_repo(&self) -> Result<gix::Repository> {
        Ok(gix::open(&self.path)?)
    }

    /// 获取 HEAD commit hash
    #[allow(dead_code)]
    pub fn head_commit_id(&self) -> Result<Option<String>> {
        let repo = self.gix_repo()?;
        let result = match repo.head_commit() {
            Ok(commit) => Ok(Some(commit.id.to_hex().to_string())),
            Err(_) => Ok(None),
        };
        result
    }

    /// 暂存所有变更（通过直接操作 index）
    fn stage_all_internal(&self) -> Result<()> {
        let repo = self.gix_repo()?;
        let index_path = self.path.join(".git").join("index");

        // 创建全新的 index 状态
        let mut state = gix::index::State::new(repo.object_hash());

        // Walk worktree and add all files
        let mut entries: Vec<(String, gix::ObjectId)> = Vec::new();

        for entry in walkdir::WalkDir::new(&self.path)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let full_path = entry.path();

            // Skip .git directory
            if full_path.components().any(|c| c.as_os_str() == ".git") {
                continue;
            }

            let relative_path = full_path.strip_prefix(&self.path)?;
            let relative_str = relative_path.to_string_lossy().to_string();

            // Read file content and create blob
            let content = std::fs::read(full_path)?;
            let blob_id: gix::ObjectId = repo.write_blob(&content)?.into();

            entries.push((relative_str, blob_id));
        }

        // Sort entries by path (required by git)
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        // Add all entries to index
        for (path, blob_id) in entries {
            let path_bstr: &BStr = path.as_bytes().into();
            state.dangerously_push_entry(
                gix::index::entry::Stat::default(),
                blob_id,
                gix::index::entry::Flags::empty(),
                gix::index::entry::Mode::FILE,
                path_bstr,
            );
        }

        // Create index file and write
        let mut index_file = gix::index::File::from_state(state, index_path);
        index_file.write(gix::index::write::Options::default())?;

        Ok(())
    }

    /// 添加文件到暂存区并提交
    pub fn add_and_commit(&mut self, message: &str) -> Result<String> {
        // Stage all changes
        self.stage_all_internal()?;

        // Now create commit
        self.commit_internal(message)
    }

    /// 提交（gix 实现）
    #[allow(dead_code)]
    pub fn commit(&self, message: &str, _staged_only: bool) -> Result<String> {
        // For now, same as add_and_commit - always stage all then commit
        self.stage_all_internal()?;
        self.commit_internal(message)
    }

    /// 内部提交逻辑
    fn commit_internal(&self, message: &str) -> Result<String> {
        let repo = self.gix_repo()?;

        // Build tree from index entries
        // We need to create a tree object from the index and write it
        let index = repo.index_or_empty()?;
        let tree = index_to_tree(&index, &repo)?;
        let tree_id: gix::ObjectId = repo.write_object(tree)?.into();

        // Get parent commit
        let parent = repo.head_commit().ok();

        // Create signatures
        let author = create_signature("Worktree User", "worktree@localhost");
        let committer = create_signature("Worktree User", "worktree@localhost");

        // Create commit
        let parents: Vec<gix::ObjectId> = if let Some(p) = parent {
            vec![p.id]
        } else {
            vec![]
        };

        let commit_id = repo
            .commit_as(
                committer,
                author,
                "HEAD",
                message,
                tree_id,
                parents.iter().copied(),
            )
            .context("Failed to create commit")?;

        Ok(commit_id.to_hex().to_string())
    }

    /// 获取工作区状态
    pub fn status(&self) -> Result<WorktreeStatus> {
        let mut status = WorktreeStatus::default();
        let repo = self.gix_repo()?;

        // Check for untracked and modified files by walking worktree
        let index = repo.index_or_empty()?;

        for entry in walkdir::WalkDir::new(&self.path)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let full_path = entry.path();

            // Skip .git directory
            if full_path.components().any(|c| c.as_os_str() == ".git") {
                continue;
            }

            let relative_path = full_path.strip_prefix(&self.path)?;
            let path_bytes = relative_path.as_os_str().as_encoded_bytes();
            let path_str = relative_path.to_string_lossy().to_string();

            // Check if in index
            let path_bstr: &BStr = path_bytes.into();
            if let Ok(pos) = index.entry_index_by_path(path_bstr) {
                let idx_entry = &index.entries()[pos];

                // Check if content changed by comparing blob ids
                let content = std::fs::read(full_path)?;
                let current_blob_id: gix::ObjectId = repo.write_blob(&content)?.into();

                if idx_entry.id != current_blob_id {
                    status.modified.push(path_str);
                }
            } else {
                // Not in index = untracked
                status.untracked.push(path_str);
            }
        }

        // Check for deleted files (in index but not in worktree)
        for entry in index.entries() {
            let entry_path = entry.path_in(index.path_backing());
            let relative_path = Path::new(std::str::from_utf8(entry_path)?);
            let full_path = self.path.join(relative_path);

            if !full_path.exists() {
                status
                    .deleted
                    .push(relative_path.to_string_lossy().to_string());
            }
        }

        // For staged files: compare index vs HEAD tree
        if let Ok(head_commit) = repo.head_commit() {
            let head_tree = head_commit.tree()?;

            for entry in index.entries() {
                let entry_path = entry.path_in(index.path_backing());
                let path_str = std::str::from_utf8(entry_path)?.to_string();

                // Check if entry exists in HEAD tree with same oid
                let in_head = head_tree.iter().any(|e| {
                    e.as_ref()
                        .map(|tree_entry| {
                            tree_entry.filename() == entry_path
                                && tree_entry.oid() == entry.id.as_ref()
                        })
                        .unwrap_or(false)
                });

                if !in_head
                    && !status.untracked.contains(&path_str)
                    && !status.deleted.contains(&path_str)
                {
                    // In index but different from HEAD = staged
                    if !status.staged.contains(&path_str) {
                        status.staged.push(path_str);
                    }
                }
            }
        } else {
            // No HEAD commit, everything in index is staged
            for entry in index.entries() {
                let entry_path = entry.path_in(index.path_backing());
                let path_str = std::str::from_utf8(entry_path)?.to_string();
                if !status.untracked.contains(&path_str)
                    && !status.deleted.contains(&path_str)
                    && !status.staged.contains(&path_str)
                {
                    status.staged.push(path_str);
                }
            }
        }

        Ok(status)
    }

    /// 重置到指定提交
    pub fn reset(&mut self, commitish: &str, hard: bool) -> Result<()> {
        let repo = self.gix_repo()?;

        // Parse commitish to commit id
        let commit_id = repo.rev_parse_single(commitish)?;
        let commit = repo
            .find_object(commit_id)?
            .try_into_commit()
            .context("Not a valid commit")?;

        // Get tree
        let tree_id = commit.tree_id()?;

        // Update HEAD reference
        repo.edit_reference(gix::refs::transaction::RefEdit {
            change: gix::refs::transaction::Change::Update {
                log: gix::refs::transaction::LogChange {
                    message: format!("reset: moving to {}", commitish).into(),
                    ..Default::default()
                },
                expected: gix::refs::transaction::PreviousValue::Any,
                new: gix::refs::Target::Object(commit_id.into()),
            },
            name: "HEAD".try_into()?,
            deref: true,
        })?;

        // Update index from tree
        let index = repo.index_or_empty()?;
        let mut index_file = (*index).clone();

        // Clear existing entries
        index_file.remove_entries(|_, _, _| true);

        // Populate index from tree
        let tree = repo.find_object(tree_id)?.try_into_tree()?;
        for entry in tree.iter() {
            let entry = entry?;
            let filename = entry.filename();
            let mode = entry.mode();
            let oid: gix::ObjectId = entry.oid().into();

            // Create index entry
            let stat = gix::index::entry::Stat::default();
            let filename_bstr: &BStr = filename;

            // Use correct API: (stat, id, flags, mode, path)
            index_file.dangerously_push_entry(
                stat,
                oid,
                gix::index::entry::Flags::empty(),
                mode.into(),
                filename_bstr,
            );
        }

        // Write index
        index_file.write(gix::index::write::Options::default())?;

        // Hard reset: update worktree files
        if hard {
            // Write files from tree to worktree
            let tree = repo.find_object(tree_id)?.try_into_tree()?;
            for entry in tree.iter() {
                let entry = entry?;
                let filename = std::str::from_utf8(entry.filename())?;
                let blob = repo.find_object(entry.oid())?.try_into_blob()?;

                let file_path = self.path.join(filename);
                if let Some(parent) = file_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&file_path, &blob.data)?;
            }

            // Remove files not in tree
            for entry in walkdir::WalkDir::new(&self.path)
                .into_iter()
                .filter_map(std::result::Result::ok)
                .filter(|e| e.file_type().is_file())
            {
                let full_path = entry.path();
                if full_path.components().any(|c| c.as_os_str() == ".git") {
                    continue;
                }

                let relative_path = full_path.strip_prefix(&self.path)?;
                let path_bytes = relative_path.as_os_str().as_encoded_bytes();

                // Check if in tree
                let tree = repo.find_object(tree_id)?.try_into_tree()?;
                let in_tree = tree.iter().any(|e| {
                    e.as_ref()
                        .map(|tree_entry| tree_entry.filename() == path_bytes)
                        .unwrap_or(false)
                });

                if !in_tree {
                    std::fs::remove_file(full_path)?;
                }
            }
        }

        Ok(())
    }

    /// 获取提交日志
    pub fn log(&self, limit: Option<usize>) -> Result<Vec<CommitInfo>> {
        let limit = limit.unwrap_or(10);
        let mut commits = Vec::new();

        let repo = self.gix_repo()?;
        let head = repo.head_commit().context("No HEAD commit found")?;

        let mut current = Some(head);
        let mut count = 0;

        while let Some(commit) = current {
            if count >= limit {
                break;
            }

            let message = commit
                .decode()
                .ok()
                .map(|c| c.message.to_string())
                .unwrap_or_default();

            let author = commit
                .author()
                .map(|a| format!("{} <{}>", a.name, a.email))
                .unwrap_or_else(|_| "Unknown".to_string());

            let time = commit
                .time()
                .map(|t| format!("{}", t))
                .unwrap_or_else(|_| "Unknown".to_string());

            commits.push(CommitInfo {
                hash: commit.id.to_hex().to_string(),
                message,
                author,
                date: time,
            });

            current = commit
                .parent_ids()
                .next()
                .and_then(|id| repo.find_object(id).ok())
                .and_then(|obj| obj.try_into_commit().ok());

            count += 1;
        }

        Ok(commits)
    }

    /// 检查是否有未提交的变更
    pub fn has_uncommitted_changes(&self) -> Result<bool> {
        let status = self.status()?;
        Ok(!status.staged.is_empty()
            || !status.modified.is_empty()
            || !status.deleted.is_empty()
            || !status.untracked.is_empty())
    }

    /// 获取两个提交之间的差异
    #[allow(dead_code)]
    pub fn diff(
        &self,
        _old_commit: Option<&str>,
        _new_commit: Option<&str>,
    ) -> Result<Vec<FileDiff>> {
        // TODO: 实现完整的 diff 逻辑
        Ok(Vec::new())
    }

    /// 获取指定 commit 的文件列表及内容
    /// 返回 Vec<(path, content)>
    pub fn get_commit_files(&self, commitish: &str) -> Result<Vec<(String, Vec<u8>)>> {
        let repo = self.gix_repo()?;
        let commit_id = repo.rev_parse_single(commitish)?;
        let commit = repo
            .find_object(commit_id)?
            .try_into_commit()
            .context("Not a valid commit")?;

        let tree = commit.tree()?;
        let mut files = Vec::new();

        for entry in tree.iter() {
            let entry = entry?;
            let filename = std::str::from_utf8(entry.filename())?.to_string();

            // 只处理文件（blob）
            if entry.mode().is_blob() {
                let blob = repo.find_object(entry.oid())?.try_into_blob()?;
                files.push((filename, blob.data.to_vec()));
            }
        }

        Ok(files)
    }

    /// 获取指定 commit 的文件路径列表
    pub fn get_commit_file_paths(&self, commitish: &str) -> Result<Vec<String>> {
        let repo = self.gix_repo()?;
        let commit_id = repo.rev_parse_single(commitish)?;
        let commit = repo
            .find_object(commit_id)?
            .try_into_commit()
            .context("Not a valid commit")?;

        let tree = commit.tree()?;
        let mut paths = Vec::new();

        for entry in tree.iter() {
            let entry = entry?;
            if entry.mode().is_blob() {
                let filename = std::str::from_utf8(entry.filename())?.to_string();
                paths.push(filename);
            }
        }

        Ok(paths)
    }
}

/// 从 index 构建 tree 对象
fn index_to_tree(index: &gix::worktree::Index, _repo: &gix::Repository) -> Result<gix::objs::Tree> {
    let mut tree = gix::objs::Tree::empty();

    // Get access to the underlying index state
    let index_file = &**index;

    for entry in index_file.entries() {
        let path = entry.path_in(index_file.path_backing());
        let filename = std::str::from_utf8(path)?;

        // Convert index entry mode to tree entry mode using EntryKind
        let kind = match entry.mode {
            gix::index::entry::Mode::DIR => gix::objs::tree::EntryKind::Tree,
            gix::index::entry::Mode::SYMLINK => gix::objs::tree::EntryKind::Link,
            gix::index::entry::Mode::COMMIT => gix::objs::tree::EntryKind::Commit,
            _ => gix::objs::tree::EntryKind::Blob,
        };

        tree.entries.push(gix::objs::tree::Entry {
            mode: kind.into(),
            filename: filename.into(),
            oid: entry.id,
        });
    }

    // Sort entries by filename (required by git format)
    tree.entries.sort_by(|a, b| a.filename.cmp(&b.filename));

    Ok(tree)
}

/// 创建签名
fn create_signature(name: &str, email: &str) -> gix::actor::SignatureRef<'static> {
    // Create a signature with 'static lifetime by using owned strings leaked to static
    // This is a workaround for the API requirement
    let name: &'static str = Box::leak(name.to_string().into_boxed_str());
    let email: &'static str = Box::leak(email.to_string().into_boxed_str());

    gix::actor::SignatureRef {
        name: name.into(),
        email: email.into(),
        time: gix::date::Time::now_local_or_utc()
            .format(gix::date::time::format::SHORT)
            .unwrap_or("now".into())
            .leak(),
    }
}

/// 提交信息
#[derive(Debug, Clone)]
pub struct CommitInfo {
    /// Commit hash
    pub hash: String,
    /// 提交信息
    pub message: String,
    /// 作者
    pub author: String,
    /// 时间
    pub date: String,
}

/// 工作区状态
#[derive(Debug, Default)]
pub struct WorktreeStatus {
    /// 已暂存的文件
    pub staged: Vec<String>,
    /// 已修改的文件
    pub modified: Vec<String>,
    /// 已删除的文件
    pub deleted: Vec<String>,
    /// 未跟踪的文件
    pub untracked: Vec<String>,
}

/// 文件差异
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct FileDiff {
    /// 文件路径
    pub path: String,
    /// 变更类型
    pub change_type: ChangeType,
    /// 旧内容
    pub old_content: Option<String>,
    /// 新内容
    pub new_content: Option<String>,
}

/// 变更类型
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChangeType {
    /// 新增
    Added,
    /// 修改
    Modified,
    /// 删除
    Deleted,
    /// 重命名
    Renamed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: 配置 git user，使 gix `commit_as` 能获取 author/committer
    fn configure_git_user(path: &std::path::Path) {
        let config_path = path.join(".git").join("config");
        let config = r#"[user]
    name = Test User
    email = test@example.com
"#;
        // append to existing config
        let mut existing = fs::read_to_string(&config_path).unwrap_or_default();
        existing.push_str(config);
        fs::write(&config_path, existing).unwrap();
    }

    // ==========================================
    // 1. Init / Open
    // ==========================================

    #[test]
    fn test_init_creates_git_dir() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let path = PathBuf::from(temp_dir.path());

        let repo = Repository::init(&path)?;
        assert!(repo.path().exists());
        assert!(
            path.join(".git").exists(),
            ".git directory should exist after init"
        );
        assert!(
            repo.head_commit_id()?.is_none(),
            "fresh repo has no HEAD commit"
        );

        Ok(())
    }

    #[test]
    fn test_open_existing_repo() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let path = PathBuf::from(temp_dir.path());

        Repository::init(&path)?;
        let repo = Repository::open(&path)?;
        assert_eq!(repo.path(), path.as_path());

        Ok(())
    }

    #[test]
    fn test_open_nonexistent_fails() {
        let path = PathBuf::from("/tmp/nonexistent_repo_xyz_12345");
        assert!(Repository::open(&path).is_err());
    }

    // ==========================================
    // 2. add_and_commit + log
    // ==========================================

    #[test]
    fn test_add_and_commit_creates_commit() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let path = PathBuf::from(temp_dir.path());

        let mut repo = Repository::init(&path)?;
        configure_git_user(&path);

        // Create a file
        fs::write(path.join("hello.txt"), "hello world")?;

        let commit_id = repo.add_and_commit("initial commit")?;
        assert!(!commit_id.is_empty(), "commit id should not be empty");

        // HEAD should now exist
        let head = repo.head_commit_id()?;
        assert!(head.is_some());
        assert_eq!(head.unwrap(), commit_id);

        Ok(())
    }

    #[test]
    fn test_log_returns_commits_in_order() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let path = PathBuf::from(temp_dir.path());

        let mut repo = Repository::init(&path)?;
        configure_git_user(&path);

        fs::write(path.join("a.txt"), "a")?;
        let id1 = repo.add_and_commit("first")?;

        fs::write(path.join("b.txt"), "b")?;
        let id2 = repo.add_and_commit("second")?;

        let log = repo.log(Some(10))?;
        assert_eq!(log.len(), 2);
        // Most recent first
        assert_eq!(log[0].hash, id2);
        assert_eq!(log[1].hash, id1);
        assert!(log[0].message.contains("second"));
        assert!(log[1].message.contains("first"));

        Ok(())
    }

    /// FIXED: commit 现在使用 index tree 而非 HEAD tree
    /// 每次 commit 都会正确反映工作区的变更
    #[test]
    fn test_commit_uses_index_tree_not_head_tree() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let path = PathBuf::from(temp_dir.path());

        let mut repo = Repository::init(&path)?;
        configure_git_user(&path);

        fs::write(path.join("a.txt"), "version1")?;
        let _id1 = repo.add_and_commit("first")?;

        // Modify file
        fs::write(path.join("a.txt"), "version2")?;
        let _id2 = repo.add_and_commit("second")?;

        // FIXED: After the second commit, the tree should contain "version2"
        let gix_repo = gix::open(&path)?;
        let head = gix_repo.head_commit()?;
        let parent = head
            .parent_ids()
            .next()
            .and_then(|id| gix_repo.find_object(id).ok())
            .and_then(|obj| obj.try_into_commit().ok())
            .expect("should have parent");

        let head_tree = head.tree_id()?;
        let parent_tree = parent.tree_id()?;

        // FIXED: trees should be different (head has version2, parent has version1)
        assert_ne!(
            head_tree.to_hex().to_string(),
            parent_tree.to_hex().to_string(),
            "trees should be different after modifying file"
        );

        Ok(())
    }

    // ==========================================
    // 3. status
    // ==========================================

    #[test]
    fn test_status_detects_untracked_files() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let path = PathBuf::from(temp_dir.path());

        let repo = Repository::init(&path)?;
        configure_git_user(&path);

        // Create file but don't commit
        fs::write(path.join("new.txt"), "new")?;

        let status = repo.status()?;
        assert!(
            status.untracked.iter().any(|f| f.contains("new.txt")),
            "new.txt should appear as untracked, got: {:?}",
            status.untracked
        );

        Ok(())
    }

    /// FIXED: 现在 status 使用 gix status API，能够正确检测修改的文件
    #[test]
    fn test_status_detects_modified_files() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let path = PathBuf::from(temp_dir.path());

        let mut repo = Repository::init(&path)?;
        configure_git_user(&path);

        fs::write(path.join("a.txt"), "original")?;
        repo.add_and_commit("initial")?;

        fs::write(path.join("a.txt"), "modified")?;

        let status = repo.status()?;
        // FIXED: 修改的文件应该出现在 modified 列表中
        assert!(
            status.modified.iter().any(|f| f.contains("a.txt")),
            "modified file should be detected, got: staged={:?} modified={:?} untracked={:?}",
            status.staged,
            status.modified,
            status.untracked
        );

        Ok(())
    }

    /// FIXED: 现在 commit 使用 index tree，commit 后 status 应该是 clean 的
    #[test]
    fn test_status_clean_after_commit() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let path = PathBuf::from(temp_dir.path());

        let mut repo = Repository::init(&path)?;
        configure_git_user(&path);

        fs::write(path.join("a.txt"), "content")?;
        repo.add_and_commit("initial")?;

        let status = repo.status()?;
        // FIXED: status should be clean after commit
        assert!(
            status.staged.is_empty() && status.modified.is_empty() && status.deleted.is_empty(),
            "status should be clean after commit; staged={:?}, modified={:?}",
            status.staged,
            status.modified
        );

        Ok(())
    }

    // ==========================================
    // 4. reset
    // ==========================================

    /// FIXED: reset --hard 现在使用 gix 实现，能够正确恢复文件内容
    #[test]
    fn test_reset_hard_restores_file_content() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let path = PathBuf::from(temp_dir.path());

        let mut repo = Repository::init(&path)?;
        configure_git_user(&path);

        fs::write(path.join("a.txt"), "original")?;
        let id1 = repo.add_and_commit("first")?;

        fs::write(path.join("a.txt"), "changed")?;
        repo.add_and_commit("second")?;

        repo.reset(&id1, true)?;

        // HEAD should point to first commit
        let head = repo.head_commit_id()?.expect("HEAD should exist");
        assert_eq!(head, id1, "HEAD should point to first commit after reset");

        // FIXED: file should be restored to original content
        let content = fs::read_to_string(path.join("a.txt"))?;
        assert_eq!(
            content, "original",
            "file content should be restored to first commit state"
        );

        Ok(())
    }

    /// Test reset --soft (不修改工作区)
    #[test]
    fn test_reset_soft_keeps_worktree() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let path = PathBuf::from(temp_dir.path());

        let mut repo = Repository::init(&path)?;
        configure_git_user(&path);

        fs::write(path.join("a.txt"), "original")?;
        let id1 = repo.add_and_commit("first")?;

        fs::write(path.join("a.txt"), "changed")?;
        repo.add_and_commit("second")?;

        repo.reset(&id1, false)?; // soft reset

        // HEAD should point to first commit
        let head = repo.head_commit_id()?.expect("HEAD should exist");
        assert_eq!(
            head, id1,
            "HEAD should point to first commit after reset --soft"
        );

        // Worktree should still have "changed"
        let content = fs::read_to_string(path.join("a.txt"))?;
        assert_eq!(
            content, "changed",
            "file content should remain after reset --soft"
        );

        Ok(())
    }

    // ==========================================
    // 5. has_uncommitted_changes
    // ==========================================

    #[test]
    fn test_has_uncommitted_changes_true_for_new_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let path = PathBuf::from(temp_dir.path());

        let repo = Repository::init(&path)?;
        fs::write(path.join("new.txt"), "content")?;

        assert!(
            repo.has_uncommitted_changes()?,
            "should detect uncommitted changes"
        );

        Ok(())
    }

    /// FIXED: `has_uncommitted_changes` 现在在 commit 后应该返回 false
    #[test]
    fn test_has_uncommitted_changes_false_after_commit() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let path = PathBuf::from(temp_dir.path());

        let mut repo = Repository::init(&path)?;
        configure_git_user(&path);

        fs::write(path.join("a.txt"), "content")?;
        repo.add_and_commit("initial")?;

        // FIXED: should be false after commit
        assert!(
            !repo.has_uncommitted_changes()?,
            "has_uncommitted_changes should be false after commit"
        );

        Ok(())
    }

    // ==========================================
    // 6. diff (placeholder)
    // ==========================================

    #[test]
    fn test_diff_returns_empty_for_now() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let path = PathBuf::from(temp_dir.path());

        let repo = Repository::init(&path)?;
        let diff = repo.diff(None, None)?;
        assert!(
            diff.is_empty(),
            "diff is not yet implemented, should return empty"
        );

        Ok(())
    }
}
