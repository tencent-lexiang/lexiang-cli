use clap::{CommandFactory, Parser};
use clap_complete::{generate, shells::Shell};
use std::io;

#[derive(Parser)]
#[command(name = "lx")]
#[command(about = "Lexiang CLI - A command-line tool for Lexiang MCP", long_about = None)]
pub struct Cli {
    /// Access token for authentication (alternative to OAuth login)
    #[arg(long = "token", global = true, env = "LX_ACCESS_TOKEN")]
    pub token: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(clap::Subcommand)]
pub enum Commands {
    /// MCP operations
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
    },
    /// Tools schema management
    Tools {
        #[command(subcommand)]
        command: ToolsCommands,
    },
    /// Manage AI agent skill files (generate, install, uninstall)
    Skill {
        #[command(subcommand)]
        command: Option<SkillCommands>,
    },
    /// Worktree management (manage multiple local workspaces)
    Worktree {
        #[command(subcommand)]
        command: WorktreeCommands,
    },
    /// Git-style commands for local workspace
    Git {
        #[command(subcommand)]
        command: GitCommands,
    },
    /// Generate shell completion script
    Completion {
        /// Shell type (bash, zsh, fish, powershell, elvish)
        shell: Shell,
    },
    /// Login via OAuth (or directly set token with --token)
    Login {
        /// Directly set access token (skip OAuth flow)
        ///
        /// Example: lx login --token "`your_access_token`"
        #[arg(long)]
        token: Option<String>,
    },
    /// Logout and remove credentials
    Logout,
    /// Start daemon with virtual filesystem
    Start {
        /// Mount point path
        #[arg(short, long)]
        mount: Option<String>,
        /// Size in MB
        #[arg(short, long, default_value = "256")]
        size: u64,
    },
    /// Stop daemon
    Stop,
    /// Show daemon status
    Status,
    /// Print version
    Version,
    /// Check for updates from GitHub releases
    Update {
        #[command(subcommand)]
        command: Option<UpdateCommands>,
    },
    /// Virtual shell for knowledge base exploration
    ///
    /// Without arguments, detects worktree from current directory (like git).
    /// Use --space to connect to a remote knowledge base via MCP (no local worktree needed).
    Sh {
        /// Knowledge base space ID or URL (remote MCP mode, no worktree required)
        #[arg(short, long)]
        space: Option<String>,
        /// Worktree directory path (default: detect from cwd)
        #[arg(short, long)]
        path: Option<String>,
        /// Execute a single command and exit (non-interactive mode)
        #[arg(short, long)]
        exec: Option<String>,
    },
    /// Start JSON-RPC server on stdio (for editor integrations)
    ///
    /// Communicates via stdin/stdout using JSON-RPC 2.0 protocol.
    /// Designed for programmatic access from VS Code, Neovim, `JetBrains`, etc.
    Serve {
        /// Enable verbose logging to stderr
        #[arg(short, long)]
        verbose: bool,
    },
}

#[derive(clap::Subcommand)]
pub enum McpCommands {
    /// List all available tools
    List,
    /// Call a tool
    Call {
        /// Tool name
        name: String,
        /// Parameters as JSON
        #[arg(short, long)]
        params: Option<String>,
    },
}

#[derive(clap::Subcommand)]
pub enum ToolsCommands {
    /// Sync tool schema from MCP Server
    Sync,
    /// List all tool categories
    Categories,
    /// Show schema version info
    Version,
    /// List tools in a category
    List {
        /// Category name (e.g., "team", "space", "entry")
        #[arg(short, long)]
        category: Option<String>,
        /// Output format (table, json)
        #[arg(short, long, default_value = "table")]
        format: String,
    },
    /// Output full schema JSON (for `OpenClaw` and other integrations)
    Schema,
    /// Sync schema from MCP Server and write to schemas/lexiang.json (development self-bootstrap)
    SyncEmbedded,
    /// Fetch unlisted tool schemas from MCP Server based on `tool_names` in schemas/unlisted.json
    SyncUnlisted,
}

#[derive(clap::Subcommand)]
pub enum SkillCommands {
    /// Generate skill files to ~/.lexiang/skills/ (default action)
    Generate {
        /// Output directory (default: ~/.lexiang/skills)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Install skills to AI agent config directories
    Install {
        /// Target agents: claude, codebuddy, gemini, codex, or "all" (default: all)
        #[arg(short, long, default_value = "all")]
        agent: String,
        /// Install scope: user (global) or project (current dir)
        #[arg(short, long, default_value = "user")]
        scope: String,
        /// Project directory (for project scope, default: current dir)
        #[arg(short, long)]
        project_dir: Option<String>,
    },
    /// Update skills: regenerate from latest schema and reinstall
    Update {
        /// Target agents: claude, codebuddy, gemini, codex, or "all" (default: all)
        #[arg(short, long, default_value = "all")]
        agent: String,
        /// Install scope: user (global) or project (current dir)
        #[arg(short, long, default_value = "user")]
        scope: String,
        /// Project directory (for project scope, default: current dir)
        #[arg(short, long)]
        project_dir: Option<String>,
    },
    /// Uninstall skills from AI agent config directories
    Uninstall {
        /// Target agents: claude, codebuddy, gemini, codex, or "all" (default: all)
        #[arg(short, long, default_value = "all")]
        agent: String,
        /// Uninstall scope: user or project (default: user)
        #[arg(short, long, default_value = "user")]
        scope: String,
        /// Project directory (for project scope, default: current dir)
        #[arg(short, long)]
        project_dir: Option<String>,
    },
    /// Show installation status across all agents
    Status {
        /// Project directory to check project-level installs
        #[arg(short, long)]
        project_dir: Option<String>,
    },
}

#[derive(clap::Subcommand)]
pub enum WorktreeCommands {
    /// Create a new worktree from a knowledge base
    Add {
        /// Target directory path
        path: String,
        /// Knowledge base ID or URL
        #[arg(short, long)]
        space_id: String,
        /// Only fetch specific entries (comma-separated IDs)
        #[arg(short, long)]
        entry_ids: Option<String>,
    },
    /// List all worktrees
    List {
        /// Output format (table, json)
        #[arg(short, long, default_value = "table")]
        format: String,
    },
    /// Remove a worktree
    Remove {
        /// Worktree path
        path: String,
        /// Skip confirmation
        #[arg(short, long)]
        yes: bool,
    },
    /// Show worktree status
    Status,
    /// Show diff between local and remote
    Diff {
        /// Output format (diff, json, json-pretty, markdown)
        #[arg(short, long, default_value = "diff")]
        format: String,
        /// Compare against remote snapshot
        #[arg(short, long)]
        remote: bool,
    },
    /// Commit local changes
    Commit {
        /// Commit message
        #[arg(short, long)]
        message: String,
        /// Stage all changes before commit
        #[arg(short, long)]
        all: bool,
    },
    /// Show commit history
    Log {
        /// Maximum number of commits to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Reset to a previous commit
    Reset {
        /// Commit-ish (e.g., HEAD~1, commit hash)
        commitish: String,
        /// Hard reset (discard working directory changes)
        #[arg(short, long)]
        hard: bool,
    },
    /// Pull latest changes from remote
    Pull,
    /// Push local changes to remote
    Push {
        /// Dry-run (show what would be done without making changes)
        #[arg(short, long)]
        dry_run: bool,
        /// Force push (overwrite remote conflicts)
        #[arg(short, long)]
        force: bool,
    },
    /// Revert remote to a previous commit (creates reverse changes)
    Revert {
        /// Commit hash to revert to
        commitish: String,
        /// Dry-run (show what would be done without making changes)
        #[arg(short, long)]
        dry_run: bool,
    },
}

#[derive(clap::Subcommand)]
pub enum GitCommands {
    /// Clone a knowledge base to local workspace
    Clone {
        /// Knowledge base ID or URL (e.g., `space_id` or `https://lexiangla.com/spaces/...`)
        space_id: String,
        /// Target directory path
        path: String,
    },
    /// Stage file contents for commit
    Add {
        /// Files to add (use "." for all)
        #[arg(default_value = ".")]
        pathspec: String,
    },
    /// Record changes to the repository
    Commit {
        /// Commit message
        #[arg(short, long)]
        message: String,
        /// Automatically stage modified files
        #[arg(short, long)]
        all: bool,
    },
    /// Show the working tree status
    Status,
    /// Show changes between commits, commit and working tree, etc
    Diff {
        /// Compare against remote
        #[arg(long)]
        remote: bool,
    },
    /// Show commit logs
    Log {
        /// Limit number of commits
        #[arg(short = 'n', long, default_value = "10")]
        max_count: usize,
    },
    /// Fetch from and integrate with remote
    Pull,
    /// Update remote refs along with associated objects
    Push {
        /// Dry-run (show what would be done)
        #[arg(short = 'n', long)]
        dry_run: bool,
        /// Force push
        #[arg(short, long)]
        force: bool,
    },
    /// Reset current HEAD to the specified state
    Reset {
        /// Commit to reset to
        commit: String,
        /// Hard reset (also reset working tree)
        #[arg(long)]
        hard: bool,
    },
    /// Revert remote to a previous commit
    Revert {
        /// Commit to revert to
        commit: String,
        /// Dry-run
        #[arg(short = 'n', long)]
        dry_run: bool,
    },
    /// Manage remote repositories
    Remote {
        /// Show verbose output (-v)
        #[arg(short, long)]
        verbose: bool,
    },
}

#[derive(clap::Subcommand)]
pub enum UpdateCommands {
    /// Check if a new version is available
    Check {
        /// Include prerelease versions
        #[arg(long)]
        prerelease: bool,
    },
    /// List recent releases
    List {
        /// Number of releases to show
        #[arg(short, long, default_value = "5")]
        limit: usize,
    },
}

impl Cli {
    pub fn generate_completion(shell: Shell) {
        generate(shell, &mut Self::command(), "lx", &mut io::stdout());
    }
}
