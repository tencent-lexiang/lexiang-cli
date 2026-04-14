//! Bash 主入口: 组装 Shell 引擎所有组件
//!
//! 对标 just-bash 的 Bash 类，提供统一的命令执行接口。
//! 支持:
//! - 别名系统 (rg→grep, eza→ls, fd→find, bat→cat, fzf)
//! - CLI 命令桥接 (git, search, worktree 等外部命令注入)
//! - 运行时动态注册命令和别名

use crate::shell::commands::alias::AliasTable;
use crate::shell::commands::bridge::{BridgeCommand, BridgeDef, BridgeFn, BridgeRegistry};
use crate::shell::commands::{self, Command, CommandRegistry};
use crate::shell::fs::IFileSystem;
use crate::shell::interpreter::{Environment, Executor};
use crate::shell::parser;
use anyhow::Result;

/// Shell 执行结果
#[derive(Debug, Clone)]
pub struct ShellOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// 虚拟 Bash 引擎
///
/// 组装 parser + executor + commands + fs + aliases，提供 `exec()` 方法。
///
/// # 用法
/// ```no_run
/// use lexiang_cli::shell::bash::Bash;
/// use lexiang_cli::shell::fs::InMemoryFs;
///
/// let fs = InMemoryFs::new().with_file("/hello.txt", "world");
/// let mut bash = Bash::new(Box::new(fs));
/// let output = bash.exec("cat /hello.txt").await.unwrap();
/// assert_eq!(output.stdout, "world");
///
/// // 现代工具别名自动生效:
/// let output = bash.exec("bat /hello.txt").await.unwrap();     // → cat
/// let output = bash.exec("rg world /").await.unwrap();          // → grep -rn
/// let output = bash.exec("eza -la /").await.unwrap();           // → ls -la
/// ```
pub struct Bash {
    fs: Box<dyn IFileSystem>,
    env: Environment,
    executor: Executor,
}

impl Bash {
    /// 创建新的 Bash 实例 (内置命令 + 默认别名)
    pub fn new(fs: Box<dyn IFileSystem>) -> Self {
        let registry = commands::create_default_registry();
        Self {
            fs,
            env: Environment::default(),
            executor: Executor::new(registry),
        }
    }

    /// 创建带自定义命令注册表的 Bash 实例
    pub fn with_registry(fs: Box<dyn IFileSystem>, registry: CommandRegistry) -> Self {
        Self {
            fs,
            env: Environment::default(),
            executor: Executor::new(registry),
        }
    }

    /// 创建带自定义别名表的 Bash 实例
    pub fn with_aliases(
        fs: Box<dyn IFileSystem>,
        registry: CommandRegistry,
        aliases: AliasTable,
    ) -> Self {
        Self {
            fs,
            env: Environment::default(),
            executor: Executor::with_aliases(registry, aliases),
        }
    }

    /// 设置初始工作目录
    pub fn with_cwd(mut self, cwd: &str) -> Self {
        self.env.set_cwd(cwd);
        self
    }

    /// 设置环境变量
    pub fn with_env(mut self, key: &str, value: &str) -> Self {
        self.env.set(key, value);
        self
    }

    // ── 动态注册 API ──

    /// 注册一个桥接命令 (将外部 CLI 命令注入虚拟 shell)
    ///
    /// # Example
    /// ```ignore
    /// bash.register_bridge(
    ///     "git",
    ///     "Git-style knowledge base operations",
    ///     vec!["status", "log", "diff"],
    ///     Arc::new(|args| Box::pin(async move {
    ///         Ok(("output".to_string(), String::new(), 0))
    ///     })),
    /// );
    /// ```
    pub fn register_bridge(
        &mut self,
        name: &str,
        description: &str,
        subcommands: Vec<&str>,
        handler: BridgeFn,
    ) {
        let cmd = BridgeCommand::new(BridgeDef {
            name: name.to_string(),
            description: description.to_string(),
            subcommands: subcommands.iter().map(|s| (*s).to_string()).collect(),
            handler,
        });
        self.executor.registry_mut().register(Box::new(cmd));
    }

    /// 批量注册桥接命令
    pub fn register_bridges(&mut self, bridge_registry: BridgeRegistry) {
        for cmd in bridge_registry.into_commands() {
            self.executor.registry_mut().register(cmd);
        }
    }

    /// 注册一个自定义命令 (实现 Command trait)
    pub fn register_command(&mut self, cmd: Box<dyn Command>) {
        self.executor.registry_mut().register(cmd);
    }

    /// 添加一个简单别名
    ///
    /// # Example
    /// ```ignore
    /// bash.add_alias("g", "git", vec![]); // g → git
    /// bash.add_alias("gs", "git", vec!["status".to_string()]); // gs → git status
    /// ```
    pub fn add_alias(&mut self, alias: &str, target: &str, extra_args: Vec<String>) {
        self.executor
            .aliases_mut()
            .add_simple(alias, target, extra_args);
    }

    // ── 查询 API ──

    /// 获取当前工作目录
    pub fn cwd(&self) -> &str {
        self.env.cwd()
    }

    /// 获取环境变量
    pub fn get_env(&self, key: &str) -> Option<&str> {
        self.env.get(key)
    }

    /// 设置环境变量
    pub fn set_env(&mut self, key: &str, value: &str) {
        self.env.set(key, value);
    }

    /// 列出所有已注册的命令
    pub fn list_commands(&self) -> Vec<&str> {
        self.executor.registry_mut_ref().list_commands()
    }

    /// 列出所有别名
    pub fn list_aliases(&self) -> Vec<(&str, &str)> {
        self.executor.aliases_ref().list_aliases()
    }

    // ── 执行 API ──

    /// 执行一条 bash 命令
    ///
    /// 支持管道、重定向、变量替换、命令列表（&&, ||, ;）、别名等。
    pub async fn exec(&mut self, input: &str) -> Result<ShellOutput> {
        let input = input.trim();
        if input.is_empty() {
            return Ok(ShellOutput {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 0,
            });
        }

        // 1. Parse: input → AST
        let script = match parser::parse(input) {
            Ok(script) => script,
            Err(e) => {
                return Ok(ShellOutput {
                    stdout: String::new(),
                    stderr: format!("bash: {e}\n"),
                    exit_code: 2,
                });
            }
        };

        // 2. Execute: AST → output (alias 解析在 executor 内部)
        match self
            .executor
            .execute_script(&script, self.fs.as_ref(), &mut self.env)
            .await
        {
            Ok(output) => Ok(ShellOutput {
                stdout: output.stdout,
                stderr: output.stderr,
                exit_code: output.exit_code,
            }),
            Err(e) => Ok(ShellOutput {
                stdout: String::new(),
                stderr: format!("bash: {e}\n"),
                exit_code: 1,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::fs::InMemoryFs;
    use std::sync::Arc;

    fn test_fs() -> Box<InMemoryFs> {
        Box::new(
            InMemoryFs::new()
                .with_dir("/docs")
                .with_dir("/docs/api")
                .with_file("/docs/readme.md", "# Welcome\n\nThis is a guide.\n")
                .with_file(
                    "/docs/api/auth.md",
                    "# Auth\n\nOAuth 2.0 guide.\nToken setup.\n",
                )
                .with_file("/docs/api/users.md", "# Users\n\nUser management API.\n")
                .with_file("/docs/deploy.md", "# Deploy\n\nDeployment guide.\n"),
        )
    }

    // ── 基础命令测试 (保留原有) ──

    #[tokio::test]
    async fn test_ls() {
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash.exec("ls /docs").await.unwrap();
        assert!(output.stdout.contains("api/"));
        assert!(output.stdout.contains("readme.md"));
    }

    #[tokio::test]
    async fn test_cat() {
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash.exec("cat /docs/readme.md").await.unwrap();
        assert!(output.stdout.contains("# Welcome"));
    }

    #[tokio::test]
    async fn test_grep() {
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash.exec("grep OAuth /docs/api/auth.md").await.unwrap();
        assert!(output.stdout.contains("OAuth"));
    }

    #[tokio::test]
    async fn test_grep_recursive() {
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash.exec("grep -r guide /docs").await.unwrap();
        assert!(output.stdout.contains("readme.md"));
    }

    #[tokio::test]
    async fn test_pipe() {
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash
            .exec("cat /docs/readme.md | grep Welcome")
            .await
            .unwrap();
        assert!(output.stdout.contains("Welcome"));
    }

    #[tokio::test]
    async fn test_pipe_chain() {
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash
            .exec("cat /docs/api/auth.md | grep -i oauth | head -1")
            .await
            .unwrap();
        assert!(output.stdout.contains("OAuth"));
    }

    #[tokio::test]
    async fn test_tree() {
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash.exec("tree /docs").await.unwrap();
        assert!(output.stdout.contains("├──") || output.stdout.contains("└──"));
        assert!(output.stdout.contains("readme.md"));
    }

    #[tokio::test]
    async fn test_find() {
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash.exec("find /docs -name '*.md' -type f").await.unwrap();
        assert!(output.stdout.contains("readme.md"));
        assert!(output.stdout.contains("auth.md"));
    }

    #[tokio::test]
    async fn test_wc() {
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash.exec("cat /docs/readme.md | wc -l").await.unwrap();
        assert!(output.stdout.trim().parse::<usize>().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_head() {
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash.exec("head -1 /docs/readme.md").await.unwrap();
        assert_eq!(output.stdout.trim(), "# Welcome");
    }

    #[tokio::test]
    async fn test_cd_and_pwd() {
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        bash.exec("cd /docs").await.unwrap();
        let output = bash.exec("pwd").await.unwrap();
        assert_eq!(output.stdout.trim(), "/docs");
    }

    #[tokio::test]
    async fn test_echo() {
        let mut bash = Bash::new(test_fs());
        let output = bash.exec("echo hello world").await.unwrap();
        assert_eq!(output.stdout, "hello world\n");
    }

    #[tokio::test]
    async fn test_variable_expansion() {
        let mut bash = Bash::new(test_fs()).with_cwd("/docs");
        let output = bash.exec("echo $PWD").await.unwrap();
        assert_eq!(output.stdout.trim(), "/docs");
    }

    #[tokio::test]
    async fn test_semicolon_commands() {
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash.exec("cd /docs; pwd").await.unwrap();
        assert_eq!(output.stdout.trim(), "/docs");
    }

    #[tokio::test]
    async fn test_readonly_guard() {
        let fs = Box::new(InMemoryFs::new_read_only());
        let mut bash = Bash::new(fs);
        let output = bash.exec("rm /test.txt").await.unwrap();
        assert!(output.stderr.contains("read-only") || output.stderr.contains("EROFS"));
    }

    #[tokio::test]
    async fn test_command_not_found() {
        let mut bash = Bash::new(test_fs());
        let output = bash.exec("nonexistent_command").await.unwrap();
        assert!(output.stderr.contains("command not found"));
        assert_ne!(output.exit_code, 0);
    }

    #[tokio::test]
    async fn test_sort_pipe() {
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash.exec("echo 'c\na\nb' | sort").await.unwrap();
        let lines: Vec<&str> = output.stdout.trim().lines().collect();
        assert_eq!(lines, vec!["a", "b", "c"]);
    }

    // ═══════════════════════════════════════════════════════════
    //  新增: 别名系统测试
    // ═══════════════════════════════════════════════════════════

    #[tokio::test]
    async fn test_alias_rg_basic() {
        // rg OAuth /docs → grep -rn OAuth /docs
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash.exec("rg OAuth /docs").await.unwrap();
        assert!(output.stdout.contains("OAuth"));
        assert!(output.stdout.contains("auth.md"));
    }

    #[tokio::test]
    async fn test_alias_rg_with_type() {
        // rg -t md guide /docs → grep -rn --include=*.md guide /docs
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash.exec("rg -t md guide /docs").await.unwrap();
        assert!(output.stdout.contains("guide"));
    }

    #[tokio::test]
    async fn test_alias_rg_ignore_case() {
        // rg -i oauth /docs → grep -rni oauth /docs
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash.exec("rg -i oauth /docs").await.unwrap();
        assert!(output.stdout.contains("OAuth"));
    }

    #[tokio::test]
    async fn test_alias_eza_basic() {
        // eza -la /docs → ls -la /docs
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash.exec("eza -la /docs").await.unwrap();
        assert!(output.stdout.contains("api/"));
        assert!(output.stdout.contains("readme.md"));
    }

    #[tokio::test]
    async fn test_alias_eza_tree() {
        // eza --tree /docs → tree /docs
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash.exec("eza --tree /docs").await.unwrap();
        assert!(output.stdout.contains("├──") || output.stdout.contains("└──"));
    }

    #[tokio::test]
    async fn test_alias_exa_compat() {
        // exa 和 eza 应该等价
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash.exec("exa /docs").await.unwrap();
        assert!(output.stdout.contains("readme.md"));
    }

    #[tokio::test]
    async fn test_alias_fd_basic() {
        // fd readme /docs → find /docs -name '*readme*'
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash.exec("fd readme /docs").await.unwrap();
        assert!(output.stdout.contains("readme"));
    }

    #[tokio::test]
    async fn test_alias_fd_extension() {
        // fd -e md /docs → find /docs -name '*.md'
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash.exec("fd -e md /docs").await.unwrap();
        assert!(output.stdout.contains(".md"));
    }

    #[tokio::test]
    async fn test_alias_bat() {
        // bat /docs/readme.md → cat /docs/readme.md
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash.exec("bat /docs/readme.md").await.unwrap();
        assert!(output.stdout.contains("# Welcome"));
    }

    #[tokio::test]
    async fn test_alias_bat_with_style() {
        // bat --plain --color=never /docs/readme.md → cat /docs/readme.md
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash
            .exec("bat --plain --color=never /docs/readme.md")
            .await
            .unwrap();
        assert!(output.stdout.contains("# Welcome"));
    }

    #[tokio::test]
    async fn test_alias_ll() {
        // ll → ls -la
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash.exec("ll /docs").await.unwrap();
        assert!(output.stdout.contains("readme.md"));
    }

    #[tokio::test]
    async fn test_alias_fzf_in_pipe() {
        // find /docs -name '*.md' | fzf -q auth
        let mut bash = Bash::new(test_fs()).with_cwd("/");
        let output = bash
            .exec("find /docs -name '*.md' -type f | fzf -q auth")
            .await
            .unwrap();
        assert!(output.stdout.contains("auth"));
    }

    // ═══════════════════════════════════════════════════════════
    //  新增: 桥接命令测试
    // ═══════════════════════════════════════════════════════════

    #[tokio::test]
    async fn test_bridge_command() {
        let mut bash = Bash::new(test_fs()).with_cwd("/");

        // 注册一个 mock git 命令
        let handler: BridgeFn = Arc::new(|args: Vec<String>| {
            Box::pin(async move {
                if args.first().map(String::as_str) == Some("status") {
                    Ok((
                        "On branch master\nnothing to commit".to_string(),
                        String::new(),
                        0,
                    ))
                } else if args.first().map(String::as_str) == Some("log") {
                    Ok((
                        "commit abc123\n  initial commit".to_string(),
                        String::new(),
                        0,
                    ))
                } else {
                    Ok((
                        String::new(),
                        format!("git: unknown subcommand {:?}", args),
                        1,
                    ))
                }
            })
        });

        bash.register_bridge(
            "git",
            "Git operations",
            vec!["status", "log", "diff"],
            handler,
        );

        // 测试 git status
        let output = bash.exec("git status").await.unwrap();
        assert_eq!(output.exit_code, 0);
        assert!(output.stdout.contains("On branch master"));

        // 测试 git log
        let output = bash.exec("git log").await.unwrap();
        assert!(output.stdout.contains("commit abc123"));

        // 测试管道: git log | grep commit
        let output = bash.exec("git log | grep commit").await.unwrap();
        assert!(output.stdout.contains("commit"));
    }

    #[tokio::test]
    async fn test_bridge_search_command() {
        let mut bash = Bash::new(test_fs()).with_cwd("/");

        // 注册 search 命令
        let handler: BridgeFn = Arc::new(|args: Vec<String>| {
            Box::pin(async move {
                let keyword = args.first().cloned().unwrap_or_default();
                Ok((
                    format!("Search results for '{keyword}':\n  1. doc1.md\n  2. doc2.md\n"),
                    String::new(),
                    0,
                ))
            })
        });

        bash.register_bridge("search", "Search knowledge base", vec![], handler);

        let output = bash.exec("search OAuth").await.unwrap();
        assert!(output.stdout.contains("OAuth"));
        assert!(output.stdout.contains("doc1.md"));
    }

    #[tokio::test]
    async fn test_bridge_with_pipe() {
        let mut bash = Bash::new(test_fs()).with_cwd("/");

        let handler: BridgeFn = Arc::new(|_args: Vec<String>| {
            Box::pin(async move {
                Ok((
                    "file1.md\nfile2.rs\nfile3.md\nfile4.toml\n".to_string(),
                    String::new(),
                    0,
                ))
            })
        });

        bash.register_bridge("worktree", "Worktree management", vec!["list"], handler);

        // worktree list | grep md
        let output = bash.exec("worktree list | grep md").await.unwrap();
        assert!(output.stdout.contains("file1.md"));
        assert!(output.stdout.contains("file3.md"));
        assert!(!output.stdout.contains("file4.toml"));
    }

    #[tokio::test]
    async fn test_custom_alias() {
        let mut bash = Bash::new(test_fs()).with_cwd("/");

        // 添加自定义别名: gs → git status
        let handler: BridgeFn = Arc::new(|args: Vec<String>| {
            Box::pin(async move { Ok((format!("git: {:?}", args), String::new(), 0)) })
        });
        bash.register_bridge("git", "Git", vec![], handler);
        bash.add_alias("gs", "git", vec!["status".to_string()]);

        let output = bash.exec("gs").await.unwrap();
        assert!(output.stdout.contains("status"));
    }

    #[tokio::test]
    async fn test_bridge_registry_bulk() {
        let mut bash = Bash::new(test_fs()).with_cwd("/");

        let mut bridge_reg = BridgeRegistry::new();

        let git_handler: BridgeFn = Arc::new(|args: Vec<String>| {
            Box::pin(async move { Ok((format!("git: {:?}", args), String::new(), 0)) })
        });
        let search_handler: BridgeFn = Arc::new(|args: Vec<String>| {
            Box::pin(async move { Ok((format!("search: {:?}", args), String::new(), 0)) })
        });

        bridge_reg.register("git", "Git ops", vec!["status", "log"], git_handler);
        bridge_reg.register("search", "Search KB", vec![], search_handler);

        bash.register_bridges(bridge_reg);

        let output = bash.exec("git status").await.unwrap();
        assert!(output.stdout.contains("git:"));

        let output = bash.exec("search hello").await.unwrap();
        assert!(output.stdout.contains("search:"));
    }
}
