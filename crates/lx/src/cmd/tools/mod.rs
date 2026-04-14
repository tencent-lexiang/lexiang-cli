use crate::config::Config;
use crate::mcp::schema::types::{ListCategoriesResponse, McpSchemaCollection};
use crate::mcp::schema::{RuntimeSchemaManager, SchemaManager};
use crate::mcp::McpClient;
use anyhow::{Context, Result};

pub async fn handle_sync(config: &Config) -> Result<()> {
    println!("Syncing tool schema from MCP Server...");

    let access_token = crate::auth::get_access_token(config).await?;
    let client = McpClient::new(&config.mcp.url, Some(access_token))?;
    let runtime = RuntimeSchemaManager::new();
    let schema = runtime.sync_from_server(&client).await?;

    println!(
        "Synced {} tools in {} categories",
        schema.tools.len(),
        schema.categories.len()
    );
    println!("Schema saved to ~/.lexiang/tools/override.json");

    Ok(())
}

pub fn handle_categories() -> Result<()> {
    let manager = SchemaManager::load_from_runtime();
    let categories = manager.get_categories();

    if categories.is_empty() {
        println!("No categories found. Run 'lx tools sync' first.");
    } else {
        println!("Tool Categories ({}):", categories.len());
        for cat in categories {
            let desc = cat.description.as_deref().unwrap_or("");
            println!("  {} ({} tools) - {}", cat.name, cat.tool_count, desc);
        }
    }

    Ok(())
}

pub fn handle_version() -> Result<()> {
    let runtime = RuntimeSchemaManager::new();
    let info = runtime.get_version_info();
    println!("{}", info);
    Ok(())
}

pub fn handle_list(category: Option<&str>, format: &str) -> Result<()> {
    let manager = SchemaManager::load_from_runtime();
    let is_json = format == "json" || format == "json-pretty";

    if let Some(cat) = category {
        let tools = manager.get_tools_by_namespace(cat);
        if is_json {
            // JSON 输出: 返回该分类下的 tools 数组
            let output = serde_json::json!({
                "category": cat,
                "tools": tools
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else if tools.is_empty() {
            println!(
                "No tools found in category '{}'. Run 'lx tools sync' first.",
                cat
            );
        } else {
            println!("Tools in '{}' ({}):", cat, tools.len());
            for tool in tools {
                let desc = tool.description.as_deref().unwrap_or("");
                println!("  {} - {}", tool.name, desc);
            }
        }
    } else {
        let categories = manager.get_categories();
        if is_json {
            // JSON 输出: 返回完整的 McpSchemaCollection
            if let Some(coll) = manager.get_collection() {
                println!("{}", serde_json::to_string_pretty(coll)?);
            } else {
                println!("{{}}");
            }
        } else if categories.is_empty() {
            println!("No categories found. Run 'lx tools sync' first.");
        } else {
            let names: Vec<_> = categories.iter().map(|c| c.name.as_str()).collect();
            println!("Available categories: {}", names.join(", "));
            println!("\nUse 'lx tools list --category <name>' to see tools in a category.");
        }
    }

    Ok(())
}

/// 输出完整 schema JSON (用于 `OpenClaw` 等集成)
pub fn handle_schema() -> Result<()> {
    let manager = SchemaManager::load_from_runtime();

    if let Some(coll) = manager.get_collection() {
        println!("{}", serde_json::to_string_pretty(coll)?);
    } else {
        eprintln!("No schema found. Run 'lx tools sync' first.");
        std::process::exit(1);
    }

    Ok(())
}

/// Sync schema from MCP Server and write directly to schemas/lexiang.json
/// This is a development self-bootstrap command: updates the compile-time embedded schema
/// so that the next `cargo build` picks up the latest tool definitions.
pub async fn handle_sync_embedded(config: &Config) -> Result<()> {
    // 1. 定位 schemas/lexiang.json（从 CARGO_MANIFEST_DIR 或 cwd 推断项目根）
    let project_root = locate_project_root()
        .context("Cannot locate project root (need Cargo.toml or schemas/ directory)")?;
    let target_path = project_root.join("schemas/lexiang.json");

    println!(
        "Syncing embedded schema from MCP Server → {}",
        target_path.display()
    );

    // 2. 从 MCP Server 获取完整 schema
    let access_token = crate::auth::get_access_token(config).await?;
    let client = McpClient::new(&config.mcp.url, Some(access_token))?;

    // 2a. 获取 categories
    let categories_result: ListCategoriesResponse = client
        .call_raw("list_tool_categories", serde_json::json!({}))
        .await
        .context("Failed to fetch tool categories")?;

    let mut schema = McpSchemaCollection::from_categories(categories_result.categories);

    // 2b. 对每个 tool 调用 get_tool_schema 获取完整的 input/output schema
    let tool_names: Vec<String> = schema.tools.keys().cloned().collect();
    let total = tool_names.len();
    let mut filled = 0usize;
    let mut failed = 0usize;

    for (i, tool_name) in tool_names.iter().enumerate() {
        eprint!("  [{}/{}] Fetching {}... ", i + 1, total, tool_name);

        let result = client
            .call_tool(
                "get_tool_schema",
                serde_json::json!({
                    "tool_name": tool_name,
                    "format": "json"
                }),
            )
            .await;

        match result {
            Ok(response) => {
                let full = crate::mcp::schema::types::McpToolSchema::from_raw_response(
                    tool_name, &response,
                );
                if let Some(existing) = schema.tools.get_mut(tool_name.as_str()) {
                    existing.input_schema = full.input_schema;
                    existing.output_schema = full.output_schema;
                    // 用 get_tool_schema 返回的 description（更完整）覆盖 category 里的
                    if full.description.is_some() {
                        existing.description = full.description;
                    }
                }
                filled += 1;
                eprintln!("✓");
            }
            Err(e) => {
                failed += 1;
                eprintln!("✗ ({})", e);
            }
        }
    }

    // 3. 写入 schemas/lexiang.json
    let content = serde_json::to_string_pretty(&schema)?;
    std::fs::write(&target_path, &content)
        .with_context(|| format!("Failed to write {}", target_path.display()))?;

    println!(
        "Done: {} tools in {} categories ({} with full schema, {} failed)",
        schema.tools.len(),
        schema.categories.len(),
        filled,
        failed,
    );
    if failed > 0 {
        eprintln!("Warning: {} tool(s) failed to fetch schema.", failed);
    }
    println!("Run `cargo build` to embed the updated schema into the binary.");

    Ok(())
}

/// Fetch unlisted tool schemas from MCP Server based on `tool_names` in schemas/unlisted.json.
///
/// Reads the `tool_names` list from `schemas/unlisted.json`, calls `get_tool_schema` (format=json)
/// for each tool name, converts the response into `McpToolSchema`, and writes back to the file.
pub async fn handle_sync_unlisted(config: &Config) -> Result<()> {
    use crate::mcp::schema::types::UnlistedSchemaCollection;

    // 1. 定位 schemas/unlisted.json
    let project_root = locate_project_root()
        .context("Cannot locate project root (need Cargo.toml or schemas/ directory)")?;
    let unlisted_path = project_root.join("schemas/unlisted.json");

    if !unlisted_path.exists() {
        anyhow::bail!(
            "schemas/unlisted.json not found at {}",
            unlisted_path.display()
        );
    }

    let content = std::fs::read_to_string(&unlisted_path)
        .with_context(|| format!("Failed to read {}", unlisted_path.display()))?;
    let mut unlisted: UnlistedSchemaCollection = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", unlisted_path.display()))?;

    if unlisted.tool_names.is_empty() {
        println!("No tool_names configured in schemas/unlisted.json, nothing to sync.");
        return Ok(());
    }

    println!(
        "Syncing {} unlisted tool schema(s) from MCP Server...",
        unlisted.tool_names.len()
    );

    // 2. 连接 MCP Server
    let access_token = crate::auth::get_access_token(config).await?;
    let client = McpClient::new(&config.mcp.url, Some(access_token))?;

    // 3. 逐个获取 tool schema
    let mut new_tools = std::collections::HashMap::new();
    let mut success_count = 0usize;
    let mut fail_count = 0usize;

    for tool_name in &unlisted.tool_names {
        eprint!("  Fetching {}... ", tool_name);

        let result = client
            .call_tool(
                "get_tool_schema",
                serde_json::json!({
                    "tool_name": tool_name,
                    "format": "json"
                }),
            )
            .await;

        match result {
            Ok(response) => {
                let schema = crate::mcp::schema::types::McpToolSchema::from_raw_response(
                    tool_name, &response,
                );
                new_tools.insert(tool_name.clone(), schema);
                success_count += 1;
                eprintln!("✓");
            }
            Err(e) => {
                fail_count += 1;
                eprintln!("✗ ({})", e);
            }
        }
    }

    // 4. 写回 unlisted.json
    unlisted.tools = new_tools;
    let output = serde_json::to_string_pretty(&unlisted)?;
    std::fs::write(&unlisted_path, &output)
        .with_context(|| format!("Failed to write {}", unlisted_path.display()))?;

    println!(
        "Done: {}/{} tool schemas updated in {}",
        success_count,
        success_count + fail_count,
        unlisted_path.display()
    );

    if fail_count > 0 {
        eprintln!(
            "Warning: {} tool(s) failed to sync. Check the names in tool_names.",
            fail_count
        );
    }

    Ok(())
}

/// 定位项目根目录（包含 Cargo.toml 和 schemas/ 的目录）
fn locate_project_root() -> Option<std::path::PathBuf> {
    // 优先使用编译时的 CARGO_MANIFEST_DIR（开发阶段 cargo run 时可用）
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let p = std::path::PathBuf::from(manifest_dir);
        if p.join("schemas/lexiang.json").exists() || p.join("Cargo.toml").exists() {
            return Some(p);
        }
    }

    // 从 cwd 向上查找
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if dir.join("Cargo.toml").exists() && dir.join("schemas").is_dir() {
            return Some(dir);
        }
        if !dir.pop() {
            break;
        }
    }

    None
}
