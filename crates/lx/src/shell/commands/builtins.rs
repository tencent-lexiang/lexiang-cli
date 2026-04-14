//! echo, pwd, cd, sort, uniq, stat 等基础命令

use super::registry::{Command, CommandContext, CommandOutput};
use crate::shell::fs;
use crate::submit_command;
use anyhow::Result;
use async_trait::async_trait;

// ============ echo ============

#[derive(Default)]
pub struct EchoCommand;

#[async_trait]
impl Command for EchoCommand {
    fn name(&self) -> &str {
        "echo"
    }

    async fn execute(
        &self,
        args: &[String],
        _ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        let mut no_newline = false;
        let mut start = 0;

        if args.first().is_some_and(|a| a == "-n") {
            no_newline = true;
            start = 1;
        }

        let mut output = args[start..].join(" ");
        if !no_newline {
            output.push('\n');
        }

        Ok(CommandOutput::success(output))
    }
}

// ============ pwd ============

#[derive(Default)]
pub struct PwdCommand;

#[async_trait]
impl Command for PwdCommand {
    fn name(&self) -> &str {
        "pwd"
    }

    async fn execute(
        &self,
        _args: &[String],
        ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        Ok(CommandOutput::success(format!("{}\n", ctx.cwd)))
    }
}

// ============ cd ============

#[derive(Default)]
pub struct CdCommand;

#[async_trait]
impl Command for CdCommand {
    fn name(&self) -> &str {
        "cd"
    }

    async fn execute(
        &self,
        args: &[String],
        ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        let target = if let Some(dir) = args.first() {
            if dir == "-" {
                // cd - : 回到上一个目录
                ctx.env.get("OLDPWD").unwrap_or("/").to_string()
            } else if dir == "~" || dir.starts_with("~/") {
                let home = ctx.env.get("HOME").unwrap_or("/").to_string();
                if dir == "~" {
                    home
                } else {
                    format!("{}{}", home, &dir[1..])
                }
            } else {
                fs::join_path(ctx.cwd, dir)
            }
        } else {
            ctx.env.get("HOME").unwrap_or("/").to_string()
        };

        // 验证目标目录存在
        match ctx.fs.exists(&target).await {
            Ok(true) => {
                // 验证是目录
                match ctx.fs.stat(&target).await {
                    Ok(stat) if stat.file_type.is_dir() => {
                        let old_cwd = ctx.cwd.to_string();
                        ctx.env.set("OLDPWD", &old_cwd);
                        ctx.env.set_cwd(&target);
                        Ok(CommandOutput::success(String::new()))
                    }
                    Ok(_) => Ok(CommandOutput::error(format!(
                        "cd: not a directory: {target}"
                    ))),
                    Err(e) => Ok(CommandOutput::error(format!("cd: {target}: {e}"))),
                }
            }
            Ok(false) => Ok(CommandOutput::error(format!(
                "cd: no such file or directory: {target}"
            ))),
            Err(e) => Ok(CommandOutput::error(format!("cd: {target}: {e}"))),
        }
    }
}

// ============ sort ============

#[derive(Default)]
pub struct SortCommand;

#[async_trait]
impl Command for SortCommand {
    fn name(&self) -> &str {
        "sort"
    }

    async fn execute(
        &self,
        args: &[String],
        ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        let mut reverse = false;
        let mut numeric = false;
        let mut unique = false;
        let mut files: Vec<&str> = Vec::new();

        for arg in args {
            if arg.starts_with('-') && !arg.starts_with("--") {
                for ch in arg[1..].chars() {
                    match ch {
                        'r' => reverse = true,
                        'n' => numeric = true,
                        'u' => unique = true,
                        _ => {}
                    }
                }
            } else {
                files.push(arg);
            }
        }

        // 获取输入内容
        let content = if files.is_empty() {
            ctx.stdin.clone().unwrap_or_default()
        } else {
            let mut combined = String::new();
            for file in files {
                let path = fs::join_path(ctx.cwd, file);
                match ctx.fs.read_file(&path).await {
                    Ok(c) => combined.push_str(&c),
                    Err(e) => return Ok(CommandOutput::error(format!("sort: {file}: {e}"))),
                }
            }
            combined
        };

        let mut lines: Vec<&str> = content.lines().collect();

        if numeric {
            lines.sort_by(|a, b| {
                let na: f64 = a.trim().parse().unwrap_or(0.0);
                let nb: f64 = b.trim().parse().unwrap_or(0.0);
                na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal)
            });
        } else {
            lines.sort();
        }

        if reverse {
            lines.reverse();
        }

        if unique {
            lines.dedup();
        }

        let mut output = lines.join("\n");
        if !output.is_empty() {
            output.push('\n');
        }
        Ok(CommandOutput::success(output))
    }
}

// ============ uniq ============

#[derive(Default)]
pub struct UniqCommand;

#[async_trait]
impl Command for UniqCommand {
    fn name(&self) -> &str {
        "uniq"
    }

    async fn execute(
        &self,
        args: &[String],
        ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        let mut count = false;
        let mut files: Vec<&str> = Vec::new();

        for arg in args {
            if arg == "-c" || arg == "--count" {
                count = true;
            } else if !arg.starts_with('-') {
                files.push(arg);
            }
        }

        let content = if files.is_empty() {
            ctx.stdin.clone().unwrap_or_default()
        } else {
            let path = fs::join_path(ctx.cwd, files[0]);
            ctx.fs.read_file(&path).await.unwrap_or_default()
        };

        let lines: Vec<&str> = content.lines().collect();
        let mut output = String::new();

        if count {
            let mut i = 0;
            while i < lines.len() {
                let mut cnt = 1;
                while i + cnt < lines.len() && lines[i + cnt] == lines[i] {
                    cnt += 1;
                }
                output.push_str(&format!("{cnt:>7} {}\n", lines[i]));
                i += cnt;
            }
        } else {
            let mut prev: Option<&str> = None;
            for line in &lines {
                if prev != Some(line) {
                    output.push_str(line);
                    output.push('\n');
                    prev = Some(line);
                }
            }
        }

        Ok(CommandOutput::success(output))
    }
}

// ============ stat ============

#[derive(Default)]
pub struct StatCommand;

#[async_trait]
impl Command for StatCommand {
    fn name(&self) -> &str {
        "stat"
    }

    async fn execute(
        &self,
        args: &[String],
        ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        if args.is_empty() {
            return Ok(CommandOutput::error("stat: missing operand".to_string()));
        }

        let mut output = String::new();
        for arg in args {
            if arg.starts_with('-') {
                continue;
            }
            let path = fs::join_path(ctx.cwd, arg);
            match ctx.fs.stat(&path).await {
                Ok(stat) => {
                    output.push_str(&format!("  File: {arg}\n"));
                    output.push_str(&format!("  Type: {}\n", stat.file_type));
                    output.push_str(&format!("  Size: {} bytes\n", stat.size));
                    output.push_str(&format!(
                        "  Read-only: {}\n",
                        if stat.readonly { "yes" } else { "no" }
                    ));
                    if let Some(ref meta) = stat.metadata {
                        if let Some(ref id) = meta.entry_id {
                            output.push_str(&format!("  Entry ID: {id}\n"));
                        }
                        if let Some(ref t) = meta.entry_type {
                            output.push_str(&format!("  Entry Type: {t}\n"));
                        }
                    }
                }
                Err(e) => {
                    output.push_str(&format!("stat: {arg}: {e}\n"));
                }
            }
        }

        Ok(CommandOutput::success(output))
    }
}

// ============ 只读保护命令 ============

pub struct ReadOnlyGuardCommand {
    cmd_name: &'static str,
}

impl ReadOnlyGuardCommand {
    /// 通用构造: 指定要拦截的命令名
    pub fn new(cmd_name: &'static str) -> Self {
        Self { cmd_name }
    }

    pub fn rm() -> Self {
        Self::new("rm")
    }
    pub fn mv() -> Self {
        Self::new("mv")
    }
    pub fn cp() -> Self {
        Self::new("cp")
    }
    pub fn mkdir_cmd() -> Self {
        Self::new("mkdir")
    }
    pub fn touch() -> Self {
        Self::new("touch")
    }
    pub fn chmod() -> Self {
        Self::new("chmod")
    }
}

#[async_trait]
impl Command for ReadOnlyGuardCommand {
    fn name(&self) -> &str {
        self.cmd_name
    }

    async fn execute(
        &self,
        _args: &[String],
        ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        if ctx.fs.is_read_only() {
            Ok(CommandOutput::error(format!(
                "{}: read-only file system (EROFS): 知识库为只读模式，不支持写操作",
                self.cmd_name
            )))
        } else {
            Ok(CommandOutput::error(format!(
                "{}: operation not supported in virtual shell",
                self.cmd_name
            )))
        }
    }
}

// ============ xargs (简化版) ============

#[derive(Default)]
pub struct XargsCommand;

#[async_trait]
impl Command for XargsCommand {
    fn name(&self) -> &str {
        "xargs"
    }

    async fn execute(
        &self,
        args: &[String],
        ctx: &mut CommandContext<'_>,
    ) -> Result<CommandOutput> {
        // 简化版: 将 stdin 每行作为最后一个参数添加到命令中
        // 真正的 xargs 需要再次调用 executor，这里只做文本拼接
        let input = ctx.stdin.clone().unwrap_or_default();
        let items: Vec<&str> = input.lines().filter(|l| !l.is_empty()).collect();

        if args.is_empty() {
            // 默认 echo
            let output = items.join(" ");
            return Ok(CommandOutput::success(format!("{output}\n")));
        }

        // 拼接: 将 stdin 行作为参数追加
        let cmd_and_args = args.join(" ");
        let items_str = items.join(" ");
        let output = format!("{cmd_and_args} {items_str}\n");
        Ok(CommandOutput::success(output))
    }
}

// ─── 命令注册 ────────────────────────────────────────────
submit_command!(EchoCommand);
submit_command!(PwdCommand);
submit_command!(CdCommand);
submit_command!(SortCommand);
submit_command!(UniqCommand);
submit_command!(StatCommand);
submit_command!(XargsCommand);

// 只读保护命令 — 使用自定义工厂
submit_command!(|| Box::new(ReadOnlyGuardCommand::new("rm")));
submit_command!(|| Box::new(ReadOnlyGuardCommand::new("mv")));
submit_command!(|| Box::new(ReadOnlyGuardCommand::new("cp")));
submit_command!(|| Box::new(ReadOnlyGuardCommand::new("mkdir")));
submit_command!(|| Box::new(ReadOnlyGuardCommand::new("touch")));
submit_command!(|| Box::new(ReadOnlyGuardCommand::new("chmod")));
