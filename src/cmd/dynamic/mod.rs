use crate::config::Config;
use crate::mcp;
use crate::mcp::schema::{build_tool_args, CommandGenerator, McpSchemaCollection};
use anyhow::{Context, Result};

use super::output::{print_csv, print_markdown, print_table, FieldFilter};

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

    let matches = cmd.try_get_matches_from(args)?;

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

    match format {
        "json" => println!("{}", result),
        "table" => print_table(&result, &filter),
        "yaml" => {
            let yaml = serde_yaml::to_string(&result)
                .context("Failed to convert result to YAML format")?;
            println!("{}", yaml);
        }
        "csv" => print_csv(&result, &filter),
        "markdown" => print_markdown(&result, &filter),
        _ => println!("{}", serde_json::to_string_pretty(&result)?),
    }

    Ok(())
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
            let tool_count = category.tool_count;
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
