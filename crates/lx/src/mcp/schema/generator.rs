use crate::mcp::schema::types::{
    extract_command_name, extract_namespace, to_kebab_case, McpCategory, McpPropertySchema,
    McpSchemaCollection, McpToolSchema,
};
use clap::{Arg, ArgAction, Command};

/// 将 String 转换为 &'static str（通过 leak）
/// 注意：这会产生内存泄漏，但对于 CLI 应用来说是可接受的
fn leak_string(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// 动态命令生成器
pub struct CommandGenerator<'a> {
    schema: &'a McpSchemaCollection,
}

impl<'a> CommandGenerator<'a> {
    pub fn new(schema: &'a McpSchemaCollection) -> Self {
        Self { schema }
    }

    /// 生成所有 namespace 子命令
    pub fn generate_namespaces(&self) -> Vec<Command> {
        self.schema
            .categories
            .iter()
            .map(|cat| self.generate_namespace_command(cat))
            .collect()
    }

    /// 生成单个 namespace 命令
    fn generate_namespace_command(&self, category: &McpCategory) -> Command {
        let namespace = extract_namespace(&category.name);
        let description = category
            .description
            .clone()
            .unwrap_or_else(|| format!("{} operations", namespace));

        let mut cmd = Command::new(leak_string(namespace.clone()))
            .about(leak_string(description))
            .subcommand_required(true);

        // 添加该 namespace 下的所有命令
        for tool in &category.tools {
            let tool_cmd = self.generate_tool_command(tool, &namespace);
            cmd = cmd.subcommand(tool_cmd);
        }

        cmd
    }

    /// 生成单个工具命令
    fn generate_tool_command(
        &self,
        tool: &crate::mcp::schema::types::McpCategoryTool,
        namespace: &str,
    ) -> Command {
        let command_name = extract_command_name(&tool.name, namespace);
        let description = tool
            .description
            .clone()
            .unwrap_or_else(|| format!("Execute {}", tool.name));

        let mut cmd = Command::new(leak_string(command_name))
            .about(leak_string(description))
            // 存储原始 tool name 用于执行
            .alias(leak_string(tool.name.clone()))
            // 添加通用的 --format 参数
            .arg(
                Arg::new("format")
                    .short('o')
                    .long("format")
                    .value_name("FORMAT")
                    .help("Output format: json, json-pretty, table, yaml, csv, markdown")
                    .default_value("json-pretty")
                    .value_parser(["json", "json-pretty", "table", "yaml", "csv", "markdown"]),
            )
            // 添加 --fields 参数，指定显示的字段
            .arg(
                Arg::new("fields")
                    .long("fields")
                    .value_name("FIELDS")
                    .help("Comma-separated list of fields to display in table/csv/markdown output"),
            )
            // 添加 --all-fields 参数，显示所有字段（包括默认隐藏的）
            .arg(
                Arg::new("all_fields")
                    .long("all-fields")
                    .help("Show all fields including normally hidden ones (cover, created_by, etc.)")
                    .action(ArgAction::SetTrue),
            )
            // 添加 --data-raw 参数，支持一次性传入所有参数（类似 curl）
            .arg(
                Arg::new("data_raw")
                    .long("data-raw")
                    .short('d')
                    .value_name("JSON")
                    .help("Pass all arguments as JSON (like curl), e.g. -d '{\"keyword\":\"test\"}'"),
            );

        // 获取完整的 tool schema 以添加参数
        if let Some(full_schema) = self.schema.tools.get(&tool.name) {
            cmd = self.add_arguments(cmd, full_schema);
        }

        cmd
    }

    /// 根据 schema 添加命令参数
    fn add_arguments(&self, mut cmd: Command, schema: &McpToolSchema) -> Command {
        if let Some(ref input_schema) = schema.input_schema {
            for (name, prop) in &input_schema.properties {
                let arg = self.create_argument(name, prop, &input_schema.required);
                cmd = cmd.arg(arg);
            }
        }
        cmd
    }

    /// 创建单个参数
    fn create_argument(&self, name: &str, prop: &McpPropertySchema, required: &[String]) -> Arg {
        let arg_name = to_kebab_case(name);
        let is_required = required.contains(&name.to_string());

        let mut arg = Arg::new(leak_string(name.to_string()))
            .long(leak_string(arg_name))
            .required(is_required)
            .value_name(leak_string(name.to_uppercase()));

        // 设置帮助文本
        if let Some(ref desc) = prop.description {
            arg = arg.help(leak_string(desc.clone()));
        }

        // 根据类型设置 action
        let type_str = prop.type_.as_deref().unwrap_or("string");
        match type_str {
            "boolean" => {
                arg = arg.action(ArgAction::SetTrue);
            }
            "array" => {
                arg = arg.action(ArgAction::Append);
            }
            "integer" | "number" => {
                arg = arg.value_parser(clap::value_parser!(i64));
            }
            _ => {
                // string, object 等都作为字符串处理
            }
        }

        // 设置默认值
        if let Some(ref default) = prop.default {
            if let Some(s) = default.as_str() {
                arg = arg.default_value(leak_string(s.to_string()));
            }
        }

        // 设置枚举值
        if let Some(ref enum_values) = prop.enum_values {
            let leaked: Vec<&'static str> =
                enum_values.iter().map(|s| leak_string(s.clone())).collect();
            arg = arg.value_parser(leaked);
        }

        arg
    }
}

/// 从命令行参数构建 MCP tool 调用参数
pub fn build_tool_args(matches: &clap::ArgMatches, schema: &McpToolSchema) -> serde_json::Value {
    // 优先使用 --data-raw / -d 参数
    if let Some(json_str) = matches.get_one::<String>("data_raw") {
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(json_str) {
            return json_value;
        }
        // JSON 解析失败，打印警告并回退到逐参数解析
        eprintln!(
            "Warning: Failed to parse --data-raw argument, falling back to individual arguments"
        );
    }

    let mut args = serde_json::Map::new();

    if let Some(ref input_schema) = schema.input_schema {
        for (name, prop) in &input_schema.properties {
            let type_str = prop.type_.as_deref().unwrap_or("string");

            let value = match type_str {
                "boolean" => {
                    let v = matches.get_flag(name);
                    serde_json::Value::Bool(v)
                }
                "integer" | "number" => {
                    if let Some(v) = matches.get_one::<i64>(name) {
                        serde_json::Value::Number((*v).into())
                    } else {
                        continue;
                    }
                }
                "array" => {
                    let values: Vec<&String> = matches
                        .get_many::<String>(name)
                        .map(std::iter::Iterator::collect)
                        .unwrap_or_default();
                    if values.is_empty() {
                        continue;
                    }
                    serde_json::Value::Array(
                        values
                            .into_iter()
                            .map(|s| serde_json::Value::String(s.clone()))
                            .collect(),
                    )
                }
                "object" => {
                    if let Some(v) = matches.get_one::<String>(name) {
                        // 尝试解析为 JSON
                        if let Ok(obj) = serde_json::from_str(v) {
                            obj
                        } else {
                            serde_json::Value::String(v.clone())
                        }
                    } else {
                        continue;
                    }
                }
                _ => {
                    // string
                    if let Some(v) = matches.get_one::<String>(name) {
                        serde_json::Value::String(v.clone())
                    } else {
                        continue;
                    }
                }
            };

            args.insert(name.clone(), value);
        }
    }

    serde_json::Value::Object(args)
}

/// 找到匹配的工具名（从 alias 或命令名）
#[allow(dead_code)]
pub fn find_tool_name(
    namespace: &str,
    command: &str,
    schema: &McpSchemaCollection,
) -> Option<String> {
    // 先尝试直接匹配
    for (name, tool) in &schema.tools {
        if tool.namespace.as_deref() == Some(namespace) {
            // 检查命令名是否匹配
            if tool.command_name.as_deref() == Some(command) {
                return Some(name.clone());
            }
            // 检查是否是原始工具名
            if name == command {
                return Some(name.clone());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_generator() {
        // 创建测试 schema
        let mut schema = McpSchemaCollection::new();
        schema.categories.push(McpCategory {
            name: "teamspace.team".to_string(),
            description: Some("Team operations".to_string()),
            tool_count: 1,
            tools: vec![crate::mcp::schema::types::McpCategoryTool {
                name: "team_list_teams".to_string(),
                description: Some("List teams".to_string()),
            }],
        });

        let generator = CommandGenerator::new(&schema);
        let namespaces = generator.generate_namespaces();

        assert_eq!(namespaces.len(), 1);
        assert_eq!(namespaces[0].get_name(), "team");
    }
}
