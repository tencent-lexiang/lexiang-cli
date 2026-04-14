//! Alias 系统: 现代 CLI 工具别名 + 参数翻译
//!
//! Claude Code / Cursor 等 AI Agent 倾向使用新一代 Rust 写的工具：
//! - `rg` (ripgrep) → 映射到内置 `grep`
//! - `eza` / `exa` → 映射到内置 `ls`
//! - `fd` → 映射到内置 `find`
//! - `fzf` → 映射到内置 `fzf` (模糊搜索)
//! - `bat` → 映射到内置 `cat`
//!
//! 不是简单的名字替换，而是**参数翻译**：
//! 把现代工具的参数习惯翻译成内置命令的参数。

use std::collections::HashMap;

/// 别名解析结果
#[derive(Debug, Clone)]
pub struct AliasExpansion {
    /// 翻译后的命令名
    pub command: String,
    /// 翻译后的参数列表
    pub args: Vec<String>,
}

/// 别名注册表
pub struct AliasTable {
    /// 简单别名: name → (`target_command`, `extra_args`)
    /// 例如 "ll" → ("ls", ["-la"])
    simple_aliases: HashMap<String, (String, Vec<String>)>,

    /// 翻译器别名: name → translator function
    /// 用于需要复杂参数翻译的情况 (rg → grep 等)
    translators: HashMap<String, Box<dyn AliasTranslator>>,
}

/// 参数翻译器 trait
pub trait AliasTranslator: Send + Sync {
    /// 将原始命令名+参数翻译为目标命令名+参数
    fn translate(&self, args: &[String]) -> AliasExpansion;

    /// 该 alias 指向的底层命令名
    fn target_command(&self) -> &str;
}

impl AliasTable {
    pub fn new() -> Self {
        Self {
            simple_aliases: HashMap::new(),
            translators: HashMap::new(),
        }
    }

    /// 创建预装所有默认别名的 `AliasTable`
    pub fn with_defaults() -> Self {
        let mut table = Self::new();

        // ── 简单别名 ──
        table.add_simple("ll", "ls", vec!["-la".to_string()]);
        table.add_simple("la", "ls", vec!["-a".to_string()]);
        table.add_simple("l", "ls", vec!["-1".to_string()]);

        // ── 翻译器别名 (现代 Rust 工具) ──
        table.add_translator("rg", Box::new(RipgrepTranslator));
        table.add_translator("eza", Box::new(EzaTranslator));
        table.add_translator("exa", Box::new(EzaTranslator)); // exa 是 eza 的前身
        table.add_translator("fd", Box::new(FdTranslator));
        table.add_translator("fdfind", Box::new(FdTranslator)); // Ubuntu 上的名字
        table.add_translator("bat", Box::new(BatTranslator));
        table.add_translator("batcat", Box::new(BatTranslator)); // Ubuntu 上的名字
        table.add_translator("fzf", Box::new(FzfTranslator));

        table
    }

    /// 添加简单别名
    pub fn add_simple(&mut self, alias: &str, target: &str, extra_args: Vec<String>) {
        self.simple_aliases
            .insert(alias.to_string(), (target.to_string(), extra_args));
    }

    /// 添加翻译器别名
    pub fn add_translator(&mut self, alias: &str, translator: Box<dyn AliasTranslator>) {
        self.translators.insert(alias.to_string(), translator);
    }

    /// 解析别名。如果命令名是别名，返回翻译后的结果；否则返回 None
    pub fn resolve(&self, command: &str, args: &[String]) -> Option<AliasExpansion> {
        // 1. 先查翻译器 (优先级高)
        if let Some(translator) = self.translators.get(command) {
            return Some(translator.translate(args));
        }

        // 2. 再查简单别名
        if let Some((target, extra_args)) = self.simple_aliases.get(command) {
            let mut merged_args = extra_args.clone();
            merged_args.extend_from_slice(args);
            return Some(AliasExpansion {
                command: target.clone(),
                args: merged_args,
            });
        }

        None
    }

    /// 检查命令是否是别名
    pub fn is_alias(&self, command: &str) -> bool {
        self.simple_aliases.contains_key(command) || self.translators.contains_key(command)
    }

    /// 列出所有别名
    pub fn list_aliases(&self) -> Vec<(&str, &str)> {
        let mut result: Vec<(&str, &str)> = Vec::new();

        for (alias, (target, _)) in &self.simple_aliases {
            result.push((alias, target));
        }

        for (alias, translator) in &self.translators {
            result.push((alias, translator.target_command()));
        }

        result.sort_by_key(|(a, _)| *a);
        result
    }
}

impl Default for AliasTable {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ═══════════════════════════════════════════════════════════════
//  ripgrep (rg) → grep 翻译器
// ═══════════════════════════════════════════════════════════════

/// `rg` → `grep` 参数翻译
///
/// ripgrep 默认行为与 grep 不同:
/// - rg 默认递归搜索
/// - rg 默认显示行号
/// - rg 默认忽略 .gitignore 中的文件
/// - rg 默认高亮匹配 (我们忽略颜色)
///
/// 参数映射:
/// - `rg pattern [path]` → `grep -rn pattern [path]`
/// - `rg -i pattern` → `grep -rni pattern`
/// - `rg -l pattern` → `grep -rl pattern`
/// - `rg -c pattern` → `grep -rc pattern`
/// - `rg --type md pattern` → `grep -rn --include='*.md' pattern`
/// - `rg -t md pattern` → `grep -rn --include='*.md' pattern`
/// - `rg -g '*.md' pattern` → `grep -rn --include='*.md' pattern`
/// - `rg --fixed-strings / -F pattern` → `grep -rn pattern` (our grep is always fixed-string)
/// - `rg -C N pattern` → `grep -rn -C N pattern`
/// - `rg --context N` → `grep -rn -C N pattern`
/// - `rg -A N` / `rg -B N` → 转为 `-C N` (简化)
/// - `rg -w pattern` → 保留 pattern 不变 (word boundary 由 grep 层处理)
/// - `rg --no-heading` → 忽略 (我们的输出格式固定)
/// - `rg --hidden` → `grep -rna` (搜索隐藏文件)
struct RipgrepTranslator;

impl AliasTranslator for RipgrepTranslator {
    fn target_command(&self) -> &str {
        "grep"
    }

    fn translate(&self, args: &[String]) -> AliasExpansion {
        let mut grep_flags = String::from("-rn"); // rg 默认递归+行号
        let mut grep_args: Vec<String> = Vec::new();
        let mut pattern: Option<String> = None;
        let mut paths: Vec<String> = Vec::new();
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];

            match arg.as_str() {
                // 直接映射的 flags
                "-i" | "--ignore-case" => grep_flags.push('i'),
                "-l" | "--files-with-matches" => grep_flags.push('l'),
                "-c" | "--count" => grep_flags.push('c'),
                "-v" | "--invert-match" => grep_flags.push('v'),
                "-F" | "--fixed-strings"       // grep 默认就是 fixed-string
                | "-w" | "--word-regexp"       // 暂不支持 word boundary
                | "--no-heading" | "--no-filename"
                | "--color=never" | "--color=always" | "--color=auto" => {} // 忽略

                // 隐藏文件
                "--hidden" | "-." => {
                    if !grep_flags.contains('a') {
                        grep_flags.push('a');
                    }
                }

                // 上下文行 (全部映射为 -C)
                "-C" | "--context" | "-A" | "--after-context" | "-B" | "--before-context" => {
                    i += 1;
                    if i < args.len() {
                        grep_args.push("-C".to_string());
                        grep_args.push(args[i].clone());
                    }
                }

                // 文件类型过滤
                "-t" | "--type" => {
                    i += 1;
                    if i < args.len() {
                        let ext = rg_type_to_glob(&args[i]);
                        grep_args.push(format!("--include={ext}"));
                    }
                }
                "-g" | "--glob" => {
                    i += 1;
                    if i < args.len() {
                        let glob = &args[i];
                        // rg 的 glob 可能是 '*.md' 或 '!*.log'
                        if !glob.starts_with('!') {
                            grep_args.push(format!("--include={glob}"));
                        }
                        // 排除 glob 我们暂不支持
                    }
                }

                // 长选项合并 --type=md
                _ if arg.starts_with("--type=") => {
                    let type_name = arg.strip_prefix("--type=").unwrap();
                    let ext = rg_type_to_glob(type_name);
                    grep_args.push(format!("--include={ext}"));
                }
                _ if arg.starts_with("--glob=") => {
                    let glob = arg.strip_prefix("--glob=").unwrap();
                    if !glob.starts_with('!') {
                        grep_args.push(format!("--include={glob}"));
                    }
                }
                _ if arg.starts_with("-C") && arg.len() > 2 => {
                    // -C3 形式
                    grep_args.push("-C".to_string());
                    grep_args.push(arg[2..].to_string());
                }
                _ if arg.starts_with("-t") && arg.len() > 2 => {
                    // -tmd 形式
                    let type_name = &arg[2..];
                    let ext = rg_type_to_glob(type_name);
                    grep_args.push(format!("--include={ext}"));
                }

                // 位置参数
                _ if arg.starts_with('-') => {
                    // 其他未知 flag，尝试拆解短选项组合
                    for ch in arg[1..].chars() {
                        match ch {
                            'i' => grep_flags.push('i'),
                            'l' => grep_flags.push('l'),
                            'c' => grep_flags.push('c'),
                            'v' => grep_flags.push('v'),
                            _ => {} // -n/-r 已有, 其余忽略
                        }
                    }
                }
                _ => {
                    // 位置参数: 第一个非 flag 是 pattern，后续是 paths
                    if pattern.is_none() {
                        pattern = Some(arg.clone());
                    } else {
                        paths.push(arg.clone());
                    }
                }
            }

            i += 1;
        }

        // 组装最终参数
        let mut final_args = vec![grep_flags];
        final_args.extend(grep_args);

        if let Some(pat) = pattern {
            final_args.push(pat);
        }

        if paths.is_empty() {
            final_args.push(".".to_string()); // rg 默认在当前目录搜索
        } else {
            final_args.extend(paths);
        }

        AliasExpansion {
            command: "grep".to_string(),
            args: final_args,
        }
    }
}

/// 将 rg 的 --type 名称映射为 glob pattern
fn rg_type_to_glob(type_name: &str) -> String {
    match type_name {
        "md" | "markdown" => "*.md".to_string(),
        "rs" | "rust" => "*.rs".to_string(),
        "py" | "python" => "*.py".to_string(),
        "js" | "javascript" => "*.js".to_string(),
        "ts" | "typescript" => "*.ts".to_string(),
        "tsx" => "*.tsx".to_string(),
        "jsx" => "*.jsx".to_string(),
        "json" => "*.json".to_string(),
        "yaml" | "yml" => "*.yml".to_string(),
        "toml" => "*.toml".to_string(),
        "html" => "*.html".to_string(),
        "css" => "*.css".to_string(),
        "go" => "*.go".to_string(),
        "java" => "*.java".to_string(),
        "c" => "*.c".to_string(),
        "cpp" => "*.cpp".to_string(),
        "h" => "*.h".to_string(),
        "sh" | "shell" | "bash" => "*.sh".to_string(),
        "sql" => "*.sql".to_string(),
        "xml" => "*.xml".to_string(),
        "txt" => "*.txt".to_string(),
        _ => format!("*.{type_name}"),
    }
}

// ═══════════════════════════════════════════════════════════════
//  eza / exa → ls 翻译器
// ═══════════════════════════════════════════════════════════════

/// `eza` / `exa` → `ls` 参数翻译
///
/// eza 是 ls 的现代替代，大部分参数兼容:
/// - `eza -la` → `ls -la`
/// - `eza -T` / `eza --tree` → `tree` (切换到 tree 命令!)
/// - `eza -T -L 2` → `tree -L 2`
/// - `eza --icons` → 忽略 (无图标支持)
/// - `eza --git` → 忽略 (无 git 状态)
/// - `eza -1` → `ls -1`
/// - `eza --long` → `ls -l`
struct EzaTranslator;

impl AliasTranslator for EzaTranslator {
    fn target_command(&self) -> &str {
        "ls"
    }

    fn translate(&self, args: &[String]) -> AliasExpansion {
        let mut is_tree = false;
        let mut tree_level: Option<String> = None;
        let mut ls_flags = String::new();
        let mut paths: Vec<String> = Vec::new();
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];

            match arg.as_str() {
                "-T" | "--tree" => is_tree = true,
                "-L" | "--level" => {
                    i += 1;
                    if i < args.len() {
                        tree_level = Some(args[i].clone());
                    }
                }
                "--icons"
                | "--icons=always"
                | "--icons=auto"
                | "--icons=never"
                | "--git"
                | "--git-ignore"
                | "--no-git"
                | "--color=always"
                | "--color=auto"
                | "--color=never"
                | "--colour=always"
                | "--colour=auto"
                | "--colour=never"
                | "--no-permissions"
                | "--no-filesize"
                | "--no-time"
                | "--no-user"
                | "--group-directories-first"
                | "--sort=name"
                | "--sort=size"
                | "--sort=time"
                | "--sort=ext" => {}
                "-l" | "--long" => ls_flags.push('l'),
                "-a" | "--all" | "-A" => ls_flags.push('a'),
                "-1" | "--oneline" => ls_flags.push('1'),
                "-h" | "--header" => ls_flags.push('h'),
                _ if arg.starts_with("--level=") => {
                    tree_level = arg
                        .strip_prefix("--level=")
                        .map(std::string::ToString::to_string);
                }
                _ if arg.starts_with('-') => {
                    // 短选项组合
                    for ch in arg[1..].chars() {
                        match ch {
                            'l' => ls_flags.push('l'),
                            'a' | 'A' => ls_flags.push('a'),
                            '1' => ls_flags.push('1'),
                            'h' => ls_flags.push('h'),
                            'T' => is_tree = true,
                            _ => {} // 忽略不认识的
                        }
                    }
                }
                _ => paths.push(arg.clone()),
            }

            i += 1;
        }

        if is_tree {
            // 切换到 tree 命令
            let mut tree_args: Vec<String> = Vec::new();
            if let Some(level) = tree_level {
                tree_args.push("-L".to_string());
                tree_args.push(level);
            }
            tree_args.extend(paths);
            return AliasExpansion {
                command: "tree".to_string(),
                args: tree_args,
            };
        }

        let mut final_args: Vec<String> = Vec::new();
        if !ls_flags.is_empty() {
            final_args.push(format!("-{ls_flags}"));
        }
        final_args.extend(paths);

        AliasExpansion {
            command: "ls".to_string(),
            args: final_args,
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  fd → find 翻译器
// ═══════════════════════════════════════════════════════════════

/// `fd` → `find` 参数翻译
///
/// fd 的参数模型与 find 差异较大:
/// - `fd pattern` → `find . -name '*pattern*'` (默认模糊匹配!)
/// - `fd pattern dir` → `find dir -name '*pattern*'`
/// - `fd -e md` → `find . -name '*.md'`
/// - `fd -e md pattern` → `find . -name '*.md' | grep pattern` (特殊)
/// - `fd -t f` → `find . -type f`
/// - `fd -t d` → `find . -type d`
/// - `fd -d 2` / `fd --max-depth 2` → `find . -maxdepth 2`
/// - `fd --hidden` → 包含隐藏文件 (默认排除)
/// - `fd -x cmd` → 我们不支持 exec
struct FdTranslator;

impl AliasTranslator for FdTranslator {
    fn target_command(&self) -> &str {
        "find"
    }

    fn translate(&self, args: &[String]) -> AliasExpansion {
        let mut pattern: Option<String> = None;
        let mut paths: Vec<String> = Vec::new();
        let mut extensions: Vec<String> = Vec::new();
        let mut file_type: Option<String> = None;
        let mut max_depth: Option<String> = None;
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];

            match arg.as_str() {
                "-e" | "--extension" => {
                    i += 1;
                    if i < args.len() {
                        extensions.push(args[i].clone());
                    }
                }
                "-t" | "--type" => {
                    i += 1;
                    if i < args.len() {
                        file_type = Some(match args[i].as_str() {
                            "f" | "file" => "f".to_string(),
                            "d" | "dir" | "directory" => "d".to_string(),
                            "l" | "symlink" => "l".to_string(),
                            other => other.to_string(),
                        });
                    }
                }
                "-d" | "--max-depth" => {
                    i += 1;
                    if i < args.len() {
                        max_depth = Some(args[i].clone());
                    }
                }
                "--hidden" | "-H"    // find 默认包含隐藏文件
                | "--no-ignore" | "-I"
                | "--color=always" | "--color=auto" | "--color=never" => {}
                "-x" | "--exec" | "-X" | "--exec-batch" => break, // 不支持 exec，停止解析
                _ if arg.starts_with("--extension=") => {
                    if let Some(ext) = arg.strip_prefix("--extension=") {
                        extensions.push(ext.to_string());
                    }
                }
                _ if arg.starts_with("--max-depth=") => {
                    max_depth = arg.strip_prefix("--max-depth=").map(std::string::ToString::to_string);
                }
                _ if arg.starts_with("--type=") => {
                    if let Some(t) = arg.strip_prefix("--type=") {
                        file_type = Some(match t {
                            "f" | "file" => "f".to_string(),
                            "d" | "dir" | "directory" => "d".to_string(),
                            other => other.to_string(),
                        });
                    }
                }
                _ if arg.starts_with('-') => {} // 忽略未知选项
                _ => {
                    // 位置参数: 第一个是 pattern, 后续是 paths
                    if pattern.is_none() {
                        pattern = Some(arg.clone());
                    } else {
                        paths.push(arg.clone());
                    }
                }
            }

            i += 1;
        }

        // 组装 find 参数
        let mut find_args: Vec<String> = Vec::new();

        // 搜索起点
        if paths.is_empty() {
            find_args.push(".".to_string());
        } else {
            find_args.extend(paths);
        }

        // max-depth (find 不标准支持，但我们的 find 实现可以扩展)
        if let Some(depth) = max_depth {
            find_args.push("-maxdepth".to_string());
            find_args.push(depth);
        }

        // -name
        if !extensions.is_empty() {
            // 如果有扩展名，用 -name '*.ext'
            // 多个扩展名时只用第一个 (简化)
            let ext = &extensions[0];
            find_args.push("-name".to_string());
            find_args.push(format!("*.{ext}"));
        } else if let Some(ref pat) = pattern {
            // fd 的 pattern 是模糊匹配，翻译为 *pattern*
            find_args.push("-name".to_string());
            if pat.contains('*') || pat.contains('?') {
                find_args.push(pat.clone()); // 已经是 glob
            } else {
                find_args.push(format!("*{pat}*"));
            }
        }

        // -type
        if let Some(ft) = file_type {
            find_args.push("-type".to_string());
            find_args.push(ft);
        }

        AliasExpansion {
            command: "find".to_string(),
            args: find_args,
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  bat → cat 翻译器
// ═══════════════════════════════════════════════════════════════

/// `bat` → `cat` 参数翻译
///
/// bat 是 cat 的现代替代：
/// - `bat file.md` → `cat file.md`
/// - `bat -n file` → `cat -n file`
/// - `bat --plain` → `cat file` (去除装饰)
/// - `bat -l md file` → `cat file` (语言高亮我们不支持)
/// - `bat --range 10:20 file` → `head -20 file | tail -11` (复杂，简化为 cat)
struct BatTranslator;

impl AliasTranslator for BatTranslator {
    fn target_command(&self) -> &str {
        "cat"
    }

    fn translate(&self, args: &[String]) -> AliasExpansion {
        let mut cat_args: Vec<String> = Vec::new();
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];

            match arg.as_str() {
                // 直接传递
                "-n" | "--number" => cat_args.push("-n".to_string()),

                // 忽略 bat 特有的选项
                "--plain"
                | "-p"
                | "--paging=never"
                | "--paging=always"
                | "--paging=auto"
                | "--style=plain"
                | "--decorations=never"
                | "--color=always"
                | "--color=auto"
                | "--color=never"
                | "--theme" => {}
                "-l" | "--language" => {
                    i += 1; // 跳过语言参数
                }
                _ if arg.starts_with("--language=") => {}
                _ if arg.starts_with("--theme=") => {}
                _ if arg.starts_with("--style=") => {}
                _ if arg.starts_with("--range") => {
                    // --range=10:20 或 --range 10:20
                    if !arg.contains('=') {
                        i += 1; // 跳过范围参数
                    }
                }

                // 位置参数 (文件路径)
                _ if !arg.starts_with('-') => cat_args.push(arg.clone()),

                // 其他短选项
                _ => {} // 忽略
            }

            i += 1;
        }

        AliasExpansion {
            command: "cat".to_string(),
            args: cat_args,
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  fzf → 内置模糊搜索命令
// ═══════════════════════════════════════════════════════════════

/// `fzf` → 内置模糊搜索
///
/// fzf 通常用在管道中：
/// - `find . | fzf` → 对输入进行模糊过滤
/// - `cat file | fzf` → 对文件内容模糊搜索
///
/// 我们把 fzf 实现为一个特殊命令，对 stdin 进行模糊匹配：
/// - `fzf -q pattern` / `fzf --query pattern` → 用 pattern 过滤 stdin
/// - `fzf -f pattern` / `fzf --filter pattern` → 同上 (非交互模式)
///
/// 在虚拟 shell 中没有交互式 UI，所以 fzf 退化为模糊 grep
struct FzfTranslator;

impl AliasTranslator for FzfTranslator {
    fn target_command(&self) -> &str {
        "fzf"
    }

    fn translate(&self, args: &[String]) -> AliasExpansion {
        // fzf 映射到内置 fzf 命令，参数直接透传
        AliasExpansion {
            command: "fzf".to_string(),
            args: args.to_vec(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  测试
// ═══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn sv(v: &[&str]) -> Vec<String> {
        v.iter().copied().map(str::to_string).collect()
    }

    #[test]
    fn test_simple_alias() {
        let table = AliasTable::with_defaults();

        let result = table.resolve("ll", &[]).unwrap();
        assert_eq!(result.command, "ls");
        assert_eq!(result.args, vec!["-la"]);

        let result = table.resolve("ll", &sv(&["/docs"])).unwrap();
        assert_eq!(result.command, "ls");
        assert_eq!(result.args, vec!["-la", "/docs"]);
    }

    #[test]
    fn test_rg_basic() {
        let table = AliasTable::with_defaults();

        // rg pattern → grep -rn pattern .
        let result = table.resolve("rg", &sv(&["OAuth"])).unwrap();
        assert_eq!(result.command, "grep");
        assert!(result.args.contains(&"-rn".to_string()));
        assert!(result.args.contains(&"OAuth".to_string()));
        assert!(result.args.contains(&".".to_string()));
    }

    #[test]
    fn test_rg_with_path() {
        let table = AliasTable::with_defaults();

        // rg pattern /docs → grep -rn pattern /docs
        let result = table.resolve("rg", &sv(&["OAuth", "/docs"])).unwrap();
        assert_eq!(result.command, "grep");
        assert!(result.args.contains(&"OAuth".to_string()));
        assert!(result.args.contains(&"/docs".to_string()));
    }

    #[test]
    fn test_rg_with_type() {
        let table = AliasTable::with_defaults();

        // rg -t md pattern → grep -rn --include=*.md pattern .
        let result = table.resolve("rg", &sv(&["-t", "md", "OAuth"])).unwrap();
        assert_eq!(result.command, "grep");
        assert!(result.args.iter().any(|a| a == "--include=*.md"));
    }

    #[test]
    fn test_rg_ignore_case() {
        let table = AliasTable::with_defaults();

        // rg -i pattern → grep -rni pattern .
        let result = table.resolve("rg", &sv(&["-i", "oauth"])).unwrap();
        assert_eq!(result.command, "grep");
        assert!(result.args[0].contains('i'));
    }

    #[test]
    fn test_rg_context() {
        let table = AliasTable::with_defaults();

        // rg -C 3 pattern → grep -rn -C 3 pattern .
        let result = table.resolve("rg", &sv(&["-C", "3", "pattern"])).unwrap();
        assert_eq!(result.command, "grep");
        assert!(result.args.contains(&"-C".to_string()));
        assert!(result.args.contains(&"3".to_string()));
    }

    #[test]
    fn test_eza_basic() {
        let table = AliasTable::with_defaults();

        // eza -la /docs → ls -la /docs
        let result = table.resolve("eza", &sv(&["-la", "/docs"])).unwrap();
        assert_eq!(result.command, "ls");
        assert!(result.args[0].contains('l'));
        assert!(result.args[0].contains('a'));
    }

    #[test]
    fn test_eza_tree() {
        let table = AliasTable::with_defaults();

        // eza -T -L 2 /docs → tree -L 2 /docs
        let result = table
            .resolve("eza", &sv(&["-T", "-L", "2", "/docs"]))
            .unwrap();
        assert_eq!(result.command, "tree");
        assert!(result.args.contains(&"-L".to_string()));
        assert!(result.args.contains(&"2".to_string()));
        assert!(result.args.contains(&"/docs".to_string()));
    }

    #[test]
    fn test_eza_tree_flag_combo() {
        let table = AliasTable::with_defaults();

        // eza --tree /docs → tree /docs
        let result = table.resolve("eza", &sv(&["--tree", "/docs"])).unwrap();
        assert_eq!(result.command, "tree");
        assert!(result.args.contains(&"/docs".to_string()));
    }

    #[test]
    fn test_fd_basic() {
        let table = AliasTable::with_defaults();

        // fd readme → find . -name '*readme*'
        let result = table.resolve("fd", &sv(&["readme"])).unwrap();
        assert_eq!(result.command, "find");
        assert!(result.args.contains(&".".to_string()));
        assert!(result.args.contains(&"-name".to_string()));
        assert!(result.args.contains(&"*readme*".to_string()));
    }

    #[test]
    fn test_fd_extension() {
        let table = AliasTable::with_defaults();

        // fd -e md → find . -name '*.md'
        let result = table.resolve("fd", &sv(&["-e", "md"])).unwrap();
        assert_eq!(result.command, "find");
        assert!(result.args.contains(&"-name".to_string()));
        assert!(result.args.contains(&"*.md".to_string()));
    }

    #[test]
    fn test_fd_with_type_and_path() {
        let table = AliasTable::with_defaults();

        // fd -t f readme /docs → find /docs -name '*readme*' -type f
        let result = table
            .resolve("fd", &sv(&["-t", "f", "readme", "/docs"]))
            .unwrap();
        assert_eq!(result.command, "find");
        assert!(result.args.contains(&"/docs".to_string()));
        assert!(result.args.contains(&"-type".to_string()));
        assert!(result.args.contains(&"f".to_string()));
    }

    #[test]
    fn test_bat_basic() {
        let table = AliasTable::with_defaults();

        // bat file.md → cat file.md
        let result = table.resolve("bat", &sv(&["file.md"])).unwrap();
        assert_eq!(result.command, "cat");
        assert_eq!(result.args, vec!["file.md"]);
    }

    #[test]
    fn test_bat_with_number() {
        let table = AliasTable::with_defaults();

        // bat -n file.md → cat -n file.md
        let result = table.resolve("bat", &sv(&["-n", "file.md"])).unwrap();
        assert_eq!(result.command, "cat");
        assert!(result.args.contains(&"-n".to_string()));
        assert!(result.args.contains(&"file.md".to_string()));
    }

    #[test]
    fn test_bat_ignores_styling() {
        let table = AliasTable::with_defaults();

        // bat --plain --color=never file.md → cat file.md
        let result = table
            .resolve("bat", &sv(&["--plain", "--color=never", "file.md"]))
            .unwrap();
        assert_eq!(result.command, "cat");
        assert_eq!(result.args, vec!["file.md"]);
    }

    #[test]
    fn test_not_alias() {
        let table = AliasTable::with_defaults();
        assert!(table.resolve("cat", &[]).is_none());
        assert!(table.resolve("grep", &[]).is_none());
    }

    #[test]
    fn test_list_aliases() {
        let table = AliasTable::with_defaults();
        let list = table.list_aliases();
        assert!(list.iter().any(|(a, _)| *a == "rg"));
        assert!(list.iter().any(|(a, _)| *a == "eza"));
        assert!(list.iter().any(|(a, _)| *a == "fd"));
        assert!(list.iter().any(|(a, _)| *a == "bat"));
        assert!(list.iter().any(|(a, _)| *a == "ll"));
    }
}
