use crate::config::Config;
use crate::mcp;
use crate::mcp::schema::{build_tool_args, CommandGenerator, McpSchemaCollection};
use anyhow::Result;

use super::output::{print_output, FieldFilter};

fn parse_field_list(fields_arg: Option<&String>) -> Option<Vec<String>> {
    fields_arg.map(|s| s.split(',').map(|f| f.trim().to_string()).collect())
}

pub async fn handle_dynamic_command(args: &[String], schema: &McpSchemaCollection) -> Result<()> {
    let config = Config::load()?;

    let base = clap::Command::new("lx")
        .about("Lexiang CLI - A command-line tool for Lexiang MCP")
        .subcommand_required(true);

    let generator = CommandGenerator::new(schema);
    let ns_commands = generator.generate_namespaces();

    let mut cmd = base;
    for ns_cmd in ns_commands {
        cmd = cmd.subcommand(ns_cmd);
    }

    let matches = match cmd.try_get_matches_from(args) {
        Ok(matches) => matches,
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            error.print()?;
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };

    let (namespace, sub_matches) = matches
        .subcommand()
        .ok_or_else(|| anyhow::anyhow!("No namespace subcommand provided"))?;
    let (subcommand, tool_matches) = sub_matches.subcommand().ok_or_else(|| {
        anyhow::anyhow!("No tool subcommand provided for namespace: {}", namespace)
    })?;

    let tool_name = find_tool_by_command(schema, namespace, subcommand)?;

    let tool_schema = schema
        .tools
        .get(&tool_name)
        .ok_or_else(|| anyhow::anyhow!("Tool schema not found: {}", tool_name))?;

    let mcp_args = build_tool_args(tool_matches, tool_schema);

    let access_token = crate::auth::get_access_token(&config).await?;

    let client = mcp::McpClient::new(&config.mcp.url, Some(access_token))?;
    let result = client.call_tool(&tool_name, mcp_args).await?;

    let format = tool_matches
        .get_one::<String>("format")
        .map(std::string::String::as_str)
        .unwrap_or("json-pretty");

    // Build field filter from --fields and --all-fields flags
    let fields: Option<Vec<String>> = parse_field_list(tool_matches.get_one::<String>("fields"));
    let all_fields = tool_matches.get_flag("all_fields");
    let filter = FieldFilter::new(fields, all_fields);

    // 统一提取 data 层：MCP 返回 { code, data, message, request_id }，
    // 用户关心的是 data 内的实际数据，其余为传输元数据。
    let data = result.get("data").unwrap_or(&result);

    print_output(data, format, &filter)
}

fn find_tool_by_command(
    schema: &McpSchemaCollection,
    namespace: &str,
    command: &str,
) -> Result<String> {
    use mcp::schema::{extract_command_name, extract_namespace};

    for category in &schema.categories {
        let cat_namespace = extract_namespace(&category.name);
        if cat_namespace == namespace {
            for tool in &category.tools {
                let cmd_name = extract_command_name(&tool.name, namespace);
                if cmd_name == command {
                    return Ok(tool.name.clone());
                }
            }
        }
    }

    // 部分高层 smartsheet 工具在服务端 category 中属于 knowledge.block，
    // 但 CLI 将它们提升为 `lx smartsheet <command>`。
    if namespace == "smartsheet" {
        for tool in schema
            .categories
            .iter()
            .flat_map(|category| &category.tools)
        {
            if mcp::schema::types::is_promoted_smartsheet_tool(&tool.name)
                && extract_command_name(&tool.name, namespace) == command
            {
                return Ok(tool.name.clone());
            }
        }
    }

    anyhow::bail!(
        "Tool not found for namespace '{}' command '{}'",
        namespace,
        command
    )
}

pub fn print_help_with_dynamic_commands(schema: Option<&McpSchemaCollection>) {
    use mcp::schema::extract_namespace;

    println!(
        "Lexiang CLI - A command-line tool for Lexiang MCP

Usage: lx [COMMAND]

Commands:
  search         Search in knowledge base (shortcut for 'lexiang search')
  lexiang        Lexiang namespace commands
  mcp            MCP operations
  tools          Tools schema management
  skill          Manage AI agent skill files (generate, install, uninstall)
  git            Git-style commands for local workspace
  worktree       Worktree management (manage multiple local workspaces)
  completion     Generate shell completion script
  login          Login via OAuth
  logout         Logout and remove credentials
  start          Start daemon with virtual filesystem
  stop           Stop daemon
  status         Show daemon status
  version        Print version
  update         Check for updates from GitHub releases
  sh             Virtual shell for knowledge base exploration"
    );

    if let Some(schema) = schema {
        println!();
        println!("Dynamic Commands (from MCP schema):");

        let mut namespaces: Vec<_> = schema.categories.iter().collect();
        namespaces.sort_by(|a, b| a.name.cmp(&b.name));

        for category in namespaces {
            let namespace = extract_namespace(&category.name);
            let desc = category.description.as_deref().unwrap_or("");
            let tool_count = if namespace == "smartsheet" {
                let mut names: Vec<_> = category.tools.iter().map(|tool| &tool.name).collect();
                for tool in schema.categories.iter().flat_map(|cat| &cat.tools) {
                    if mcp::schema::is_promoted_smartsheet_tool(&tool.name)
                        && !names.contains(&&tool.name)
                    {
                        names.push(&tool.name);
                    }
                }
                names.len() as u32
            } else {
                category.tool_count
            };
            println!("  {namespace:14} {desc} ({tool_count} commands)");
        }
    }

    println!(
        "
  help           Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_commands_resolve_to_page_tools() {
        let schema = crate::mcp::schema::test_command_schema();

        assert_eq!(
            find_tool_by_command(&schema, "block", "fetch").unwrap(),
            "block_fetch_page"
        );
        assert_eq!(
            find_tool_by_command(&schema, "block", "update").unwrap(),
            "block_update_page"
        );
        assert_eq!(
            find_tool_by_command(&schema, "block", "update-block").unwrap(),
            "block_update_block"
        );
        assert_eq!(
            find_tool_by_command(&schema, "block", "update-blocks").unwrap(),
            "block_update_blocks"
        );
    }

    #[test]
    fn smartsheet_commands_resolve_to_expected_tools() {
        let schema = crate::mcp::schema::test_command_schema();
        let cases = [
            ("smartsheet", "create", "smartsheet_create"),
            ("smartsheet", "fetch", "smartsheet_fetch"),
            ("smartsheet", "update-schema", "smartsheet_update_schema"),
            ("block", "smartsheet-create", "smartsheet_create"),
            ("block", "smartsheet-fetch", "smartsheet_fetch"),
            ("block", "smartsheet-list", "smartsheet_list"),
            (
                "block",
                "smartsheet-list-records",
                "smartsheet_list_records",
            ),
            (
                "block",
                "smartsheet-update-records",
                "smartsheet_update_records",
            ),
            (
                "block",
                "smartsheet-update-schema",
                "smartsheet_update_schema",
            ),
            ("block", "smartsheet-update-view", "smartsheet_update_view"),
            ("smartsheet", "create-field", "smartsheet_create_field"),
            ("smartsheet", "create-records", "smartsheet_create_records"),
            ("smartsheet", "create-view", "smartsheet_create_view"),
            ("smartsheet", "delete-field", "smartsheet_delete_field"),
            ("smartsheet", "delete-records", "smartsheet_delete_records"),
            ("smartsheet", "delete-view", "smartsheet_delete_view"),
            (
                "smartsheet",
                "describe-record",
                "smartsheet_describe_record",
            ),
            ("smartsheet", "list-fields", "smartsheet_list_fields"),
            ("smartsheet", "list-records", "smartsheet_list_records"),
            ("smartsheet", "list", "smartsheet_list_smartsheets"),
            ("smartsheet", "list-views", "smartsheet_list_views"),
            ("smartsheet", "update-field", "smartsheet_update_field"),
            ("smartsheet", "update-records", "smartsheet_update_records"),
            ("smartsheet", "update-view", "smartsheet_update_view"),
        ];

        for (namespace, command, expected_tool) in cases {
            assert_eq!(
                find_tool_by_command(&schema, namespace, command).unwrap(),
                expected_tool,
                "unexpected tool mapping for {namespace} {command}"
            );
        }
    }
}
