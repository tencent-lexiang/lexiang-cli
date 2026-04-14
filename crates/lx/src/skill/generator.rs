//! Skill 文件管理器
//!
//! 支持两种模式：
//! 1. **静态模式（推荐）**：从项目 `skills/` 目录读取预生成的静态 SKILL.md 文件
//! 2. **动态模式（fallback）**：根据 MCP schema 在运行时自动生成 skill 文件
//!
//! 静态文件支持 `::: params <tool_name>` slot 语法（类似 `VuePress`），
//! slot 区域内的参数表格由 MCP schema 自动刷新，slot 外的内容为手写静态部分。
//!
//! ```markdown
//! ::: params entry_create_entry
//! | 参数 | 类型 | 必填 | 描述 |
//! |------|------|------|------|
//! | `entry_type` | string | 否 | 节点类型 |
//! :::
//! ```

use crate::cmd::block::build_block_commands;
use crate::datadir;
use crate::mcp::schema::types::{
    extract_command_name, extract_namespace, to_kebab_case, McpCategory, McpSchemaCollection,
};
use anyhow::Result;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// 静态 skill 文件目录（项目根 skills/）
const STATIC_SKILLS_DIR: &str = "skills";

/// Block 高级命令的文档模板（编译期嵌入）
const BLOCK_ADVANCED_TEMPLATE: &str = include_str!("templates/block_advanced.md");

/// 安全截断字符串（处理多字节 UTF-8 字符）
fn truncate_str(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        // 多行描述只取第一行
        s.lines().next().unwrap_or(s).to_string()
    } else {
        let truncated: String = s.chars().take(max_chars - 3).collect();
        format!("{}...", truncated)
    }
}

/// 从 SKILL.md 的 YAML frontmatter 提取 description 字段
fn extract_skill_description(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    // 简单解析 YAML frontmatter（--- ... ---）
    if !content.starts_with("---") {
        return None;
    }
    let rest = &content[3..];
    let end = rest.find("---")?;
    let frontmatter = &rest[..end];

    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(desc) = line.strip_prefix("description:") {
            let desc = desc.trim().trim_matches('"').trim_matches('\'');
            if !desc.is_empty() {
                return Some(desc.to_string());
            }
        }
    }
    None
}

/// Skill 文件生成器
pub struct SkillGenerator<'a> {
    schema: &'a McpSchemaCollection,
    output_dir: PathBuf,
}

impl<'a> SkillGenerator<'a> {
    pub fn new(schema: &'a McpSchemaCollection, output_dir: PathBuf) -> Self {
        Self { schema, output_dir }
    }

    /// 生成所有 skill 文件
    ///
    /// 优先从静态 skills/ 目录复制预生成文件，不存在时回退到动态生成。
    pub fn generate_all(&self) -> Result<Vec<PathBuf>> {
        fs::create_dir_all(&self.output_dir)?;

        let mut generated_files = Vec::new();

        // 尝试从静态目录加载
        let static_dir = find_static_skills_dir();
        if let Some(ref sdir) = static_dir {
            let copied = self.copy_static_skills(sdir)?;
            if !copied.is_empty() {
                // 清理根目录下旧的动态生成 .md 文件（README.md 除外，已被静态模式覆盖）
                self.cleanup_legacy_files()?;
                return Ok(copied);
            }
        }

        // 创建 references/ 子目录（仅动态模式需要）
        let references_dir = self.output_dir.join("references");
        fs::create_dir_all(&references_dir)?;

        // Fallback: 动态生成
        // 1. 生成总览文件
        let readme_path = self.output_dir.join("README.md");
        fs::write(&readme_path, self.generate_readme())?;
        generated_files.push(readme_path);

        // 2. 为每个 namespace 生成 skill 文件到 references/ 子目录
        for category in &self.schema.categories {
            let namespace = extract_namespace(&category.name);
            let filename = format!("{}.md", namespace);
            let filepath = references_dir.join(&filename);

            let content = self.generate_namespace_skill(category);
            fs::write(&filepath, content)?;
            generated_files.push(filepath);
        }

        Ok(generated_files)
    }

    /// 清理输出根目录下旧的动态生成 .md 文件
    ///
    /// 静态模式下，所有 skill 文件都在 references/ 子目录中，
    /// 根目录只保留 README.md，其余旧文件（如 block.md、entry.md）需要删除。
    fn cleanup_legacy_files(&self) -> Result<()> {
        for entry in fs::read_dir(&self.output_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "md" {
                        let filename = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        // 保留 README.md，删除其余旧 .md 文件
                        if filename != "README.md" {
                            fs::remove_file(&path)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// 从静态 skills/ 目录复制完整 skill 子目录到输出目录
    ///
    /// 每个 skill 目录结构：
    /// ```text
    /// lx-search/
    /// ├── SKILL.md           # 主入口（决策树 + 执行规则）
    /// └── references/        # 详细参数参考文档
    ///     ├── kb-search.md
    ///     └── ...
    /// ```
    fn copy_static_skills(&self, static_dir: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        // 收集所有有效的静态 skill 目录（含 SKILL.md）
        let mut skill_dirs = Vec::new();
        for entry in fs::read_dir(static_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let skill_file = path.join("SKILL.md");
            if !skill_file.exists() {
                continue;
            }

            let dir_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            skill_dirs.push((dir_name, path));
        }

        if skill_dirs.is_empty() {
            return Ok(files);
        }

        // 清理输出目录下旧的 skill 子目录，避免混杂
        for (dir_name, _) in &skill_dirs {
            let dest_skill_dir = self.output_dir.join(dir_name);
            if dest_skill_dir.exists() {
                fs::remove_dir_all(&dest_skill_dir)?;
            }
        }
        // 同时清理旧格式的 references/ 目录（兼容迁移）
        let legacy_refs = self.output_dir.join("references");
        if legacy_refs.exists() {
            fs::remove_dir_all(&legacy_refs)?;
        }

        // 复制每个 skill 的完整目录结构
        for (dir_name, src_dir) in &skill_dirs {
            let dest_skill_dir = self.output_dir.join(dir_name);
            fs::create_dir_all(&dest_skill_dir)?;

            // 复制 SKILL.md
            let src_skill = src_dir.join("SKILL.md");
            let dest_skill = dest_skill_dir.join("SKILL.md");
            fs::copy(&src_skill, &dest_skill)?;
            files.push(dest_skill);

            // 复制 references/ 子目录（如果存在）
            let src_refs = src_dir.join("references");
            if src_refs.is_dir() {
                let dest_refs = dest_skill_dir.join("references");
                fs::create_dir_all(&dest_refs)?;
                for ref_entry in fs::read_dir(&src_refs)? {
                    let ref_entry = ref_entry?;
                    let ref_path = ref_entry.path();
                    if ref_path.extension().is_some_and(|ext| ext == "md") {
                        let dest_ref = dest_refs.join(ref_path.file_name().unwrap_or_default());
                        fs::copy(&ref_path, &dest_ref)?;
                        files.push(dest_ref);
                    }
                }
            }
        }

        // 生成 README.md 索引
        if !files.is_empty() {
            let readme_path = self.output_dir.join("README.md");
            fs::write(&readme_path, self.generate_static_readme(&skill_dirs))?;
            files.insert(0, readme_path);
        }

        Ok(files)
    }

    /// 为静态 skill 目录生成 README.md 索引
    fn generate_static_readme(&self, skill_dirs: &[(String, PathBuf)]) -> String {
        let mut content = String::new();

        content.push_str("# lx CLI Skills\n\n");
        content.push_str(
            "本目录包含 lx CLI 的 AI agent 技能文件，用于指导 agent 如何操作乐享知识库。\n\n",
        );
        content.push_str(
            "每个 skill 包含 `SKILL.md`（决策指引和执行规则）和 `references/`（详细参数参考）。\n\n",
        );

        content.push_str("## 可用 Skills\n\n");
        content.push_str("| Skill | 描述 | 入口文件 |\n");
        content.push_str("|-------|------|----------|\n");
        for (dir_name, src_dir) in skill_dirs {
            // 尝试从 SKILL.md frontmatter 提取 description
            let desc = extract_skill_description(&src_dir.join("SKILL.md"))
                .unwrap_or_else(|| format!("{} skill", dir_name));
            let short_desc = truncate_str(&desc, 60);
            content.push_str(&format!(
                "| `{}` | {} | [{}/SKILL.md]({}/SKILL.md) |\n",
                dir_name, short_desc, dir_name, dir_name
            ));
        }

        content.push_str("\n## 快速开始\n\n");
        content.push_str("```bash\n");
        content.push_str("# 登录\n");
        content.push_str("lx login\n\n");
        content.push_str("# 查看可用命令\n");
        content.push_str("lx tools categories\n");
        content.push_str("lx tools list --category entry\n");
        content.push_str("```\n");

        content
    }

    /// 生成 README.md 总览文件
    fn generate_readme(&self) -> String {
        let mut content = String::new();

        content.push_str("# lx CLI Skills\n\n");
        content.push_str(
            "本目录包含 lx CLI 的 AI agent 技能文件，用于指导 agent 如何使用各个 namespace 的命令。\n\n",
        );

        // 统计信息
        let total_tools: usize = self.schema.categories.iter().map(|c| c.tools.len()).sum();
        content.push_str(&format!(
            "## 概览\n\n- **Namespaces**: {}\n- **Total Tools**: {}\n\n",
            self.schema.categories.len(),
            total_tools
        ));

        // Namespace 列表
        content.push_str("## 可用 Namespaces\n\n");
        content.push_str("| Namespace | 描述 | 命令数 | 技能文件 |\n");
        content.push_str("|-----------|------|--------|----------|\n");

        for category in &self.schema.categories {
            let namespace = extract_namespace(&category.name);
            let desc = category.description.as_deref().unwrap_or("无描述");
            let tool_count = category.tools.len();
            content.push_str(&format!(
                "| `{}` | {} | {} | [{}.md](references/{}.md) |\n",
                namespace, desc, tool_count, namespace, namespace
            ));
        }

        // 快速开始
        content.push_str("\n## 快速开始\n\n");
        content.push_str("```bash\n");
        content.push_str("# 登录\n");
        content.push_str("lx login\n\n");
        content.push_str("# 同步最新 schema\n");
        content.push_str("lx tools sync\n\n");
        content.push_str("# 查看可用命令\n");
        content.push_str("lx tools categories\n");
        content.push_str("lx tools list --category team\n");
        content.push_str("```\n\n");

        // 使用方法
        content.push_str("## 如何使用技能文件\n\n");
        content.push_str("AI agent 可以读取这些 skill 文件来学习如何使用 lx CLI。\n\n");
        content.push_str("例如，当用户需要搜索知识库时，agent 可以：\n");
        content.push_str("1. 读取 `references/search.md` 了解搜索命令的参数和用法\n");
        content.push_str("2. 构建正确的命令执行搜索\n");
        content.push_str("3. 解析输出结果\n");

        content
    }

    /// 生成单个 namespace 的 skill 文件
    fn generate_namespace_skill(&self, category: &McpCategory) -> String {
        let namespace = extract_namespace(&category.name);
        let mut content = String::new();

        // 标题和描述
        content.push_str(&format!("# lx {} - {}\n\n", namespace, category.name));

        if let Some(desc) = &category.description {
            content.push_str(&format!("{}\n\n", desc));
        }

        // 概览表格
        content.push_str("## 命令概览\n\n");
        content.push_str("| 命令 | 描述 |\n");
        content.push_str("|------|------|\n");

        for tool in &category.tools {
            let cmd_name = extract_command_name(&tool.name, &namespace);
            let desc = tool.description.as_deref().unwrap_or("无描述");
            // 截断过长描述（安全处理多字节字符）
            let short_desc = truncate_str(desc, 80);
            content.push_str(&format!(
                "| `lx {} {}` | {} |\n",
                namespace, cmd_name, short_desc
            ));
        }

        // 详细命令说明
        content.push_str("\n## 命令详情\n\n");

        for tool in &category.tools {
            let cmd_name = extract_command_name(&tool.name, &namespace);
            content.push_str(&format!("### `lx {} {}`\n\n", namespace, cmd_name));

            if let Some(desc) = &tool.description {
                content.push_str(&format!("{}\n\n", desc));
            }

            content.push_str(&format!("**MCP Tool**: `{}`\n\n", tool.name));

            // 获取完整 schema 以显示参数
            if let Some(full_schema) = self.schema.tools.get(&tool.name) {
                if let Some(input_schema) = &full_schema.input_schema {
                    if !input_schema.properties.is_empty() {
                        content.push_str("**参数**:\n\n");
                        content.push_str("| 参数 | 类型 | 必填 | 描述 |\n");
                        content.push_str("|------|------|------|------|\n");

                        for (name, prop) in &input_schema.properties {
                            let arg_name = to_kebab_case(name);
                            let type_str = prop.type_.as_deref().unwrap_or("string");
                            let required = if input_schema.required.contains(name) {
                                "是"
                            } else {
                                "否"
                            };
                            let desc = prop.description.as_deref().unwrap_or("-");
                            // 截断过长描述（安全处理多字节字符）
                            let short_desc = truncate_str(desc, 60);
                            content.push_str(&format!(
                                "| `--{}` | {} | {} | {} |\n",
                                arg_name, type_str, required, short_desc
                            ));
                        }
                        content.push('\n');
                    }
                }
            }

            // 使用示例
            content.push_str("**示例**:\n\n");
            content.push_str("```bash\n");
            content.push_str(&format!("lx {} {}", namespace, cmd_name));

            // 添加示例参数
            if let Some(full_schema) = self.schema.tools.get(&tool.name) {
                if let Some(input_schema) = &full_schema.input_schema {
                    for name in &input_schema.required {
                        let arg_name = to_kebab_case(name);
                        content.push_str(&format!(" --{} <{}>", arg_name, name.to_uppercase()));
                    }
                }
            }
            content.push_str("\n```\n\n");

            content.push_str("---\n\n");
        }

        // 典型工作流
        content.push_str("## 典型工作流\n\n");
        content.push_str(&self.generate_workflow_examples(&namespace));

        // 高级封装命令（仅 block 命名空间）
        if namespace == "block" {
            content.push_str("\n---\n\n");
            // 自动生成的命令参考表（从 clap 定义读取，保持文档与代码一致）
            content.push_str(&generate_block_command_reference());
            content.push_str("\n\n");
            // 嵌入式模板：工作流示例和使用说明
            content.push_str(BLOCK_ADVANCED_TEMPLATE);
        }

        content
    }

    /// 生成典型工作流示例
    fn generate_workflow_examples(&self, namespace: &str) -> String {
        let mut content = String::new();

        match namespace {
            "team" => {
                content.push_str("### 列出所有团队\n\n");
                content.push_str("```bash\n");
                content.push_str("# 获取用户可访问的团队列表\n");
                content.push_str("lx team list\n\n");
                content.push_str("# 获取常用团队\n");
                content.push_str("lx team list-frequent\n");
                content.push_str("```\n");
            }
            "space" => {
                content.push_str("### 操作知识库\n\n");
                content.push_str("```bash\n");
                content.push_str("# 获取团队下的知识库列表\n");
                content.push_str("lx space list --team-id <TEAM_ID>\n\n");
                content.push_str("# 获取知识库详情\n");
                content.push_str("lx space describe --space-id <SPACE_ID>\n\n");
                content.push_str("# 获取最近访问的知识库\n");
                content.push_str("lx space list-recently\n");
                content.push_str("```\n");
            }
            "entry" => {
                content.push_str("### 操作知识条目\n\n");
                content.push_str("```bash\n");
                content.push_str("# 获取条目详情\n");
                content.push_str("lx entry describe --entry-id <ENTRY_ID>\n\n");
                content.push_str("# 创建新条目\n");
                content.push_str("lx entry create --parent-entry-id <PARENT_ID> --name \"新文档\" --entry-type page\n\n");
                content.push_str("# 获取条目的子节点\n");
                content.push_str("lx entry list-children --parent-id <ENTRY_ID>\n");
                content.push_str("```\n");
            }
            "search" => {
                content.push_str("### 搜索知识库\n\n");
                content.push_str("```bash\n");
                content.push_str("# 全局搜索\n");
                content.push_str("lx search kb --keyword \"关键词\"\n\n");
                content.push_str("# 在指定知识库中搜索\n");
                content.push_str("lx search kb --keyword \"关键词\" --space-id <SPACE_ID>\n\n");
                content.push_str("# 向量检索\n");
                content.push_str("lx search embedding --keyword \"语义查询\"\n");
                content.push_str("```\n");
            }
            "block" => {
                content.push_str("### 操作文档块（原子命令）\n\n");
                content.push_str(
                    "> **推荐优先使用高级命令**（见下方），原子命令适合精细控制单个块。\n\n",
                );
                content.push_str("原子命令由 MCP schema 动态生成，参见上方「命令详情」。\n");
                // 高级命令的完整文档通过模板嵌入，在 generate_namespace_skill() 末尾追加
            }
            "file" => {
                content.push_str("### 文件操作\n\n");
                content.push_str("```bash\n");
                content.push_str("# 获取文件详情\n");
                content.push_str("lx file describe --file-id <FILE_ID>\n\n");
                content.push_str("# 下载文件\n");
                content.push_str("lx file download --file-id <FILE_ID>\n\n");
                content.push_str("# 上传文件（需要多步骤）\n");
                content.push_str("# 1. 申请上传\n");
                content.push_str("lx file apply-upload --parent-entry-id <PARENT_ID> --file-name \"example.pdf\"\n");
                content.push_str("# 2. 使用返回的 upload_url 上传文件\n");
                content.push_str("# 3. 确认上传\n");
                content.push_str("lx file commit-upload --session-id <SESSION_ID>\n");
                content.push_str("```\n");
            }
            _ => {
                content.push_str(&format!("### 使用 {} namespace\n\n", namespace));
                content.push_str("```bash\n");
                content.push_str(&format!("# 查看 {} 命令帮助\n", namespace));
                content.push_str(&format!("lx {} --help\n", namespace));
                content.push_str("```\n");
            }
        }

        content
    }
}

/// 查找静态 skills/ 目录
///
/// 搜索顺序：
/// 1. 当前工作目录下的 skills/
/// 2. 可执行文件所在目录的同级 skills/
fn find_static_skills_dir() -> Option<PathBuf> {
    // 1. 当前工作目录
    let cwd = std::env::current_dir().ok()?;
    let cwd_skills = cwd.join(STATIC_SKILLS_DIR);
    if cwd_skills.is_dir() && has_skill_files(&cwd_skills) {
        return Some(cwd_skills);
    }

    // 2. 可执行文件所在目录的同级
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let exe_skills = exe_dir.join(STATIC_SKILLS_DIR);
            if exe_skills.is_dir() && has_skill_files(&exe_skills) {
                return Some(exe_skills);
            }
        }
    }

    None
}

/// 检查目录中是否有 SKILL.md 文件（至少有一个子目录含 SKILL.md）
fn has_skill_files(dir: &Path) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("SKILL.md").exists() {
                return true;
            }
        }
    }
    false
}

/// 从 clap 命令定义自动生成高级 block 命令参考表
///
/// 确保文档与代码始终一致：新增/修改命令只需改 `cmd::block::build_block_commands()`，
/// 文档会自动更新。
fn generate_block_command_reference() -> String {
    let commands = build_block_commands();
    let mut out = String::new();

    out.push_str("## 高级命令参考\n\n");
    out.push_str("| 命令 | 描述 | 必填参数 | 可选参数 |\n");
    out.push_str("|------|------|----------|----------|\n");

    for cmd in &commands {
        let name = cmd.get_name();
        let about = cmd
            .get_about()
            .map(std::string::ToString::to_string)
            .unwrap_or_default();

        let mut required = Vec::new();
        let mut optional = Vec::new();

        for arg in cmd.get_arguments() {
            let arg_name = arg.get_id().as_str();
            // 跳过 clap 内建参数
            if arg_name == "help" || arg_name == "version" {
                continue;
            }
            let flag = format!("`--{}`", arg_name);
            if arg.is_required_set() {
                required.push(flag);
            } else {
                optional.push(flag);
            }
        }

        let required_str = if required.is_empty() {
            "-".to_string()
        } else {
            required.join(", ")
        };
        let optional_str = if optional.is_empty() {
            "-".to_string()
        } else {
            optional.join(", ")
        };

        let _ = writeln!(
            out,
            "| `lx block {}` | {} | {} | {} |",
            name, about, required_str, optional_str
        );
    }

    out
}

/// 生成 skill 文件到默认目录
#[allow(dead_code)]
pub fn generate_skills(schema: &McpSchemaCollection) -> Result<Vec<PathBuf>> {
    let skill_dir = datadir::skills_dir();

    let generator = SkillGenerator::new(schema, skill_dir);
    generator.generate_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::schema::types::{McpCategory, McpCategoryTool};
    use std::collections::HashMap;

    fn test_schema() -> McpSchemaCollection {
        McpSchemaCollection {
            version: "test".to_string(),
            categories: vec![McpCategory {
                name: "teamspace.team".to_string(),
                description: Some("团队管理".to_string()),
                tool_count: 2,
                tools: vec![
                    McpCategoryTool {
                        name: "team_list_teams".to_string(),
                        description: Some("列出团队".to_string()),
                    },
                    McpCategoryTool {
                        name: "team_describe_team".to_string(),
                        description: Some("获取团队详情".to_string()),
                    },
                ],
            }],
            tools: HashMap::new(),
        }
    }

    #[test]
    fn test_skill_generator_readme() {
        let schema = test_schema();
        let temp_dir = std::env::temp_dir().join("lexiang-skill-test-readme");
        let generator = SkillGenerator::new(&schema, temp_dir.clone());

        let readme = generator.generate_readme();
        assert!(readme.contains("# lx CLI Skills"));
        assert!(readme.contains("team"));
        assert!(readme.contains("团队管理"));
        // 链接应指向 references/ 子目录
        assert!(readme.contains("references/team.md"));

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_skill_generator_references_dir() {
        let schema = test_schema();
        // 使用独特的目录名避免静态 skills/ 目录被意外发现
        let temp_dir = std::env::temp_dir().join("lexiang-skill-test-refs-dynamic");
        let _ = fs::remove_dir_all(&temp_dir);

        let generator = SkillGenerator::new(&schema, temp_dir.clone());

        // 直接测试动态生成（不走 generate_all 避免静态模式干扰）
        let refs_dir = temp_dir.join("references");
        fs::create_dir_all(&refs_dir).unwrap();

        let readme_path = temp_dir.join("README.md");
        fs::write(&readme_path, generator.generate_readme()).unwrap();
        assert!(readme_path.exists());

        // 验证 README 内容
        let readme = fs::read_to_string(&readme_path).unwrap();
        assert!(readme.contains("# lx CLI Skills"));
        assert!(readme.contains("references/team.md"));

        // 为 namespace 生成 skill 文件
        for category in &schema.categories {
            let namespace = extract_namespace(&category.name);
            let filepath = refs_dir.join(format!("{}.md", namespace));
            fs::write(&filepath, generator.generate_namespace_skill(category)).unwrap();
            assert!(filepath.exists());
        }

        assert!(refs_dir.join("team.md").exists());

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_find_static_skills_dir() {
        // 当 skills/ 不存在时应返回 None 或有值取决于执行环境
        // 此测试仅验证函数不 panic
        let _result = find_static_skills_dir();
    }

    #[test]
    fn test_has_skill_files() {
        let temp_dir = std::env::temp_dir().join("lexiang-skill-test-has-files");
        let _ = fs::remove_dir_all(&temp_dir);

        // 空目录
        fs::create_dir_all(&temp_dir).unwrap();
        assert!(!has_skill_files(&temp_dir));

        // 有 SKILL.md 的子目录
        let sub_dir = temp_dir.join("lx-test");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(sub_dir.join("SKILL.md"), "test").unwrap();
        assert!(has_skill_files(&temp_dir));

        let _ = fs::remove_dir_all(temp_dir);
    }
}
