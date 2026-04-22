//! Block 静态 CLI 工具封装
//!
//! 将 block 相关 MCP tool 封装为 `lx block` 静态子命令，
//! 支持自动 MDX ↔ Block JSON 转换。
//!
//! 已有静态实现的命令会优先于动态 MCP proxy 执行，
//! 避免 MCP 动态工具覆盖本地增强功能。

use crate::config::Config;
use crate::mcp::RealMcpCaller;
use crate::service::block::BlockService;
use anyhow::Result;
use clap::{Arg, ArgAction, Command};

/// 所有静态 block 子命令名（用于判断是否命中静态实现）
pub const STATIC_SUBCOMMANDS: &[&str] = &[
    // ── 核心 CRUD ──
    "ls",
    "get",
    "create",
    "update",
    "delete",
    "move",
    // ── 查询 ──
    "find",
    // ── 转换 ──
    "convert",
    "export",
    "import",
    // ── 高级操作 ──
    "table-get",
    "table-set",
    "table-add-row",
    "table-del-row",
    "replace-section",
    "insert-after",
    "append",
    "tree",
];

/// 构建所有静态 block 子命令
pub fn build_block_commands() -> Vec<Command> {
    vec![
        // ══════════════ 核心 CRUD ══════════════
        Command::new("ls")
            .about("List children of a block (static wrapper)")
            .arg(
                Arg::new("block-id")
                    .long("block-id")
                    .short('b')
                    .help("Parent block ID (omit to use entry's root page block)"),
            )
            .arg(
                Arg::new("entry-id")
                    .long("entry-id")
                    .short('e')
                    .help("Entry ID (usually required by MCP server)"),
            )
            .arg(
                Arg::new("recursive")
                    .long("recursive")
                    .short('r')
                    .action(ArgAction::SetTrue)
                    .help("Recursively list all descendants (with_descendants)"),
            ),
        Command::new("get")
            .about("Get block details (static wrapper)")
            .arg(
                Arg::new("block-id")
                    .long("block-id")
                    .short('b')
                    .required(true)
                    .help("Block ID"),
            )
            .arg(
                Arg::new("entry-id")
                    .long("entry-id")
                    .short('e')
                    .help("Entry ID (usually required by MCP server)"),
            )
            .arg(
                Arg::new("format")
                    .long("format")
                    .short('f')
                    .default_value("json")
                    .help("Output format: json, mdx"),
            ),
        Command::new("create")
            .about("Create descendant blocks (supports MDX auto-conversion)")
            .arg(
                Arg::new("block-id")
                    .long("block-id")
                    .required(true)
                    .help("Parent block ID"),
            )
            .arg(
                Arg::new("descendant")
                    .long("descendant")
                    .help("Block JSON or MDX content (auto-detected)"),
            )
            .arg(
                Arg::new("content")
                    .long("content")
                    .help("MDX/Markdown content (shorthand for --descendant)"),
            )
            .arg(
                Arg::new("file")
                    .long("file")
                    .help("Read MDX/Markdown from file (shorthand for --descendant)"),
            )
            .arg(
                Arg::new("content-type")
                    .long("content-type")
                    .help("Force content type: auto, mdx, blocks"),
            )
            .arg(
                Arg::new("after-block-id")
                    .long("after-block-id")
                    .help("Insert after this sibling block"),
            )
            .arg(
                Arg::new("children")
                    .long("children")
                    .help("First-level child IDs (comma-separated)"),
            ),
        Command::new("update")
            .about("Update a block (supports MDX auto-conversion)")
            .arg(
                Arg::new("block-id")
                    .long("block-id")
                    .required(true)
                    .help("Block ID to update"),
            )
            .arg(
                Arg::new("text")
                    .long("text")
                    .help("Update text content"),
            )
            .arg(
                Arg::new("descendant")
                    .long("descendant")
                    .help("Full block JSON or MDX for replacement"),
            )
            .arg(
                Arg::new("content")
                    .long("content")
                    .help("MDX/Markdown content (shorthand)"),
            )
            .arg(
                Arg::new("file")
                    .long("file")
                    .help("Read MDX/Markdown from file"),
            )
            .arg(
                Arg::new("content-type")
                    .long("content-type")
                    .help("Force content type: auto, mdx, blocks"),
            ),
        Command::new("delete")
            .about("Delete a block and its descendants")
            .arg(
                Arg::new("block-id")
                    .long("block-id")
                    .required(true)
                    .help("Block ID to delete"),
            ),
        Command::new("move")
            .about("Move blocks to a new parent position")
            .arg(
                Arg::new("block-ids")
                    .long("block-ids")
                    .required(true)
                    .value_delimiter(',')
                    .help("Block IDs to move (comma-separated)"),
            )
            .arg(
                Arg::new("parent-block-id")
                    .long("parent-block-id")
                    .required(true)
                    .help("Target parent block ID"),
            )
            .arg(
                Arg::new("after-block-id")
                    .long("after-block-id")
                    .help("Insert after this sibling block"),
            ),
        // ══════════════ 查询 ══════════════
        Command::new("find")
            .about("Search blocks by text, heading, or type")
            .arg(
                Arg::new("query")
                    .long("query")
                    .short('q')
                    .required(true)
                    .help("Search query text / heading text / block type name"),
            )
            .arg(
                Arg::new("mode")
                    .long("mode")
                    .short('m')
                    .default_value("text")
                    .help("Search mode: text (substring), heading (exact match), type (h1/h2/table/...)"),
            )
            .arg(
                Arg::new("block-id")
                    .long("block-id")
                    .short('b')
                    .help("Root block to search in (omit for entry root)"),
            )
            .arg(
                Arg::new("entry-id")
                    .long("entry-id")
                    .short('e')
                    .help("Entry ID (usually required)"),
            )
            .arg(
                Arg::new("limit")
                    .long("limit")
                    .default_value("20")
                    .help("Max results to return"),
            ),
        // ══════════════ 转换 ══════════════
        Command::new("convert")
            .about("Convert between MDX and Block JSON (local)")
            .arg(
                Arg::new("content")
                    .long("content")
                    .help("MDX/Markdown or Block JSON content"),
            )
            .arg(Arg::new("file").long("file").help("Read from file"))
            .arg(
                Arg::new("from")
                    .long("from")
                    .default_value("auto")
                    .help("Source type: auto, mdx, blocks"),
            )
            .arg(
                Arg::new("to")
                    .long("to")
                    .default_value("blocks")
                    .help("Target type: mdx, blocks, json"),
            ),
        Command::new("export")
            .about("Export block tree as MDX or JSON")
            .arg(
                Arg::new("block-id")
                    .long("block-id")
                    .required(true)
                    .help("Root block ID"),
            )
            .arg(
                Arg::new("format")
                    .long("format")
                    .short('f')
                    .default_value("mdx")
                    .help("Output format: mdx, json, markdown"),
            ),
        Command::new("import")
            .about("Import MDX/Markdown into block tree (with chunking)")
            .arg(
                Arg::new("block-id")
                    .long("block-id")
                    .required(true)
                    .help("Parent block ID"),
            )
            .arg(Arg::new("file").long("file").required(true).help("MDX/Markdown file path"))
            .arg(
                Arg::new("chunk-size")
                    .long("chunk-size")
                    .default_value("20")
                    .help("Blocks per batch"),
            ),
        // ══════════════ 高级操作（保留） ══════════════
        Command::new("table-get")
            .about("Get table as structured view")
            .arg(Arg::new("block-id").long("block-id").required(true).help("Table block ID"))
            .arg(
                Arg::new("format")
                    .long("format")
                    .short('f')
                    .default_value("table")
                    .help("Output format: table, json, csv, markdown"),
            ),
        Command::new("table-set")
            .about("Set a cell value in a table")
            .arg(Arg::new("block-id").long("block-id").required(true))
            .arg(Arg::new("row").long("row").required(true).value_parser(clap::value_parser!(usize)))
            .arg(Arg::new("col").long("col").required(true).value_parser(clap::value_parser!(usize)))
            .arg(Arg::new("text").long("text").required(true)),
        Command::new("table-add-row")
            .about("Append a row to a table")
            .arg(Arg::new("block-id").long("block-id").required(true))
            .arg(Arg::new("values").long("values").required(true).help("Comma-separated values")),
        Command::new("table-del-row")
            .about("Delete a row from a table")
            .arg(Arg::new("block-id").long("block-id").required(true))
            .arg(Arg::new("row").long("row").required(true).value_parser(clap::value_parser!(usize))),
        Command::new("replace-section")
            .about("Replace content under a heading")
            .arg(Arg::new("block-id").long("block-id").required(true))
            .arg(Arg::new("heading").long("heading").required(true))
            .arg(Arg::new("content").long("content").help("Markdown content"))
            .arg(Arg::new("file").long("file")),
        Command::new("insert-after")
            .about("Insert content after a block")
            .arg(Arg::new("block-id").long("block-id").required(true))
            .arg(Arg::new("content").long("content"))
            .arg(Arg::new("file").long("file")),
        Command::new("append")
            .about("Append content to a parent block")
            .arg(Arg::new("block-id").long("block-id").required(true))
            .arg(Arg::new("content").long("content"))
            .arg(Arg::new("file").long("file")),
        Command::new("tree")
            .about("Display block tree structure")
            .arg(Arg::new("block-id").long("block-id").required(true))
            .arg(
                Arg::new("recursive")
                    .long("recursive")
                    .short('r')
                    .action(ArgAction::SetTrue),
            ),
    ]
}

/// 判断是否为已注册的静态子命令
pub fn is_static_subcommand(name: &str) -> bool {
    STATIC_SUBCOMMANDS.contains(&name)
}

/// 尝试处理静态 block 子命令
///
/// 返回 Ok(true) 表示已处理，Ok(false) 表示不是静态命令应回退到动态命令。
pub async fn try_handle_block_command(args: &[String]) -> Result<bool> {
    if args.len() < 3 {
        return Ok(false);
    }

    let subcommand = &args[2];

    // --help / -h 特殊处理：显示静态命令帮助
    if subcommand == "--help" || subcommand == "-h" {
        let mut block_cmd = Command::new("block")
            .about("Block operations (static enhanced)")
            .long_about(
                "Static block commands with MDX auto-conversion.\n\
                 These override dynamic MCP proxy when available.\n\
                 Use `lx block <command> --help` for details.",
            );
        for sub in build_block_commands() {
            block_cmd = block_cmd.subcommand(sub);
        }
        block_cmd = block_cmd.after_long_help(
            "\nDynamic subcommands (from MCP, no static wrapper):\n\
             apply-attachment-upload, convert-content-to-blocks,\n\
             create-descendant, delete-children, describe,\n\
             list-children, move, update-blocks\n\n\
             Run without static wrapper:\n\
               lx block list-children --block-id <id>",
        );
        block_cmd.print_help().ok();
        println!();
        return Ok(true);
    }

    if !is_static_subcommand(subcommand) {
        return Ok(false);
    }

    // 构建 clap 命令进行解析
    let cmd = Command::new("lx").subcommand_required(true).subcommand({
        let mut block_cmd = Command::new("block").about("Block operations (static + dynamic)");
        for sub in build_block_commands() {
            block_cmd = block_cmd.subcommand(sub);
        }
        block_cmd
    });

    let matches = match cmd.try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            e.print().ok();
            if e.use_stderr() {
                return Err(e.into());
            }
            std::process::exit(0);
        }
    };
    let block_matches = matches.subcommand_matches("block").unwrap();
    let (subcmd, sub_matches) = block_matches.subcommand().unwrap();

    let config = Config::load()?;
    let access_token = crate::auth::get_access_token(&config).await?;
    let mcp = RealMcpCaller::new(&config.mcp.url, Some(access_token));
    let service = BlockService::new(Box::new(mcp));

    match subcmd {
        // ── 核心 CRUD ──
        "ls" => handle_ls(&service, sub_matches).await?,
        "get" => handle_get(&service, sub_matches).await?,
        "create" => handle_create(&service, sub_matches).await?,
        "update" => handle_update(&service, sub_matches).await?,
        "delete" => handle_delete(&service, sub_matches).await?,
        "move" => handle_move(&service, sub_matches).await?,
        // ── 查询 ──
        "find" => handle_find(&service, sub_matches).await?,
        // ── 转换 ──
        "convert" => handle_convert(&service, sub_matches).await?,
        "export" => handle_export(&service, sub_matches).await?,
        "import" => handle_import(&service, sub_matches).await?,
        // ── 高级操作 ──
        "table-get" => handle_table_get(&service, sub_matches).await?,
        "table-set" => handle_table_set(&service, sub_matches).await?,
        "table-add-row" => handle_table_add_row(&service, sub_matches).await?,
        "table-del-row" => handle_table_del_row(&service, sub_matches).await?,
        "replace-section" => handle_replace_section(&service, sub_matches).await?,
        "insert-after" => handle_insert_after(&service, sub_matches).await?,
        "append" => handle_append(&service, sub_matches).await?,
        "tree" => handle_tree(&service, sub_matches).await?,
        _ => return Ok(false),
    }

    Ok(true)
}

// ══════════════════════════════════════════════════
//  核心 CRUD 处理函数
// ══════════════════════════════════════════════════

async fn handle_ls(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_id = matches.get_one::<String>("block-id");
    let entry_id = matches.get_one::<String>("entry-id");
    let recursive = matches.get_flag("recursive");

    if entry_id.is_none() {
        eprintln!(
            "warning: --entry-id is usually required by MCP server. Try --entry-id <ENTRY_ID>"
        );
    }

    // Map CLI args → MCP schema names
    //   --block-id  → parent_block_id (optional, defaults to entry's page root)
    //   --entry-id  → entry_id (usually required)
    //   -r/--recursive → with_descendants (true)
    let mut args = serde_json::json!({
        "with_descendants": recursive,
    });
    if let Some(bid) = block_id {
        args["parent_block_id"] = serde_json::json!(bid);
    }
    if let Some(eid) = entry_id {
        args["entry_id"] = serde_json::json!(eid);
    }

    let result = service
        .mcp()
        .call_tool("block_list_block_children", args)
        .await?;
    println!("{}", serde_json::to_string_pretty(&result)?);

    Ok(())
}

async fn handle_get(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_id = matches.get_one::<String>("block-id").unwrap();
    let entry_id = matches.get_one::<String>("entry-id");
    let format = matches
        .get_one::<String>("format")
        .map(std::string::String::as_str)
        .unwrap_or("json");

    // Build args with optional entry_id
    let mut args = serde_json::json!({ "block_id": block_id });
    if let Some(eid) = entry_id {
        args["entry_id"] = serde_json::json!(eid);
    }

    let result = service
        .mcp()
        .call_tool("block_describe_block", args)
        .await?;

    match format {
        "mdx" | "markdown" => {
            // 使用本地 MDX 引擎（完整语义保真）
            use crate::service::block::BlockService;
            let block_data = result
                .get("data")
                .or_else(|| result.get("block"))
                .cloned()
                .unwrap_or(result.clone());
            let block = crate::service::block::Block::from_json(&block_data);
            let mdx = BlockService::block_to_mdx_local(&block)?;
            print!("{}", mdx);
        }
        _ => {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }

    Ok(())
}

async fn handle_create(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_id = matches.get_one::<String>("block-id").unwrap();
    let content_type = matches
        .get_one::<String>("content-type")
        .map(std::string::String::as_str)
        .unwrap_or("auto");
    let after_block_id = matches.get_one::<String>("after-block-id");
    let children = matches.get_one::<String>("children");

    // 读取内容（--descendant / --content / --file）
    let raw_content = resolve_content(matches)?;

    // 自动转换
    let descendant = resolve_descendant(service, &raw_content, content_type).await?;

    let mut args = serde_json::json!({
        "block_id": block_id,
        "descendant": descendant,
    });
    if let Some(after) = after_block_id {
        args["after_block_id"] = serde_json::json!(after);
    }
    if let Some(c) = children {
        args["children"] = serde_json::json!(c.split(',').map(str::trim).collect::<Vec<_>>());
    }

    let result = service
        .mcp()
        .call_tool("block_create_block_descendant", args)
        .await?;
    println!("{}", serde_json::to_string_pretty(&result)?);

    Ok(())
}

async fn handle_update(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_id = matches.get_one::<String>("block-id").unwrap();

    // 简单 text 更新
    if let Some(text) = matches.get_one::<String>("text") {
        let result = service
            .mcp()
            .call_tool(
                "block_update_block",
                serde_json::json!({
                    "block_id": block_id,
                    "text": text,
                }),
            )
            .await?;
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    // 完整更新（支持 MDX / blocks JSON）
    let content_type = matches
        .get_one::<String>("content-type")
        .map(std::string::String::as_str)
        .unwrap_or("auto");
    let raw_content = resolve_content(matches)?;
    let update_data = resolve_update_data(service, &raw_content, content_type).await?;

    let result = service
        .mcp()
        .call_tool("block_update_block", update_data)
        .await?;
    println!("{}", serde_json::to_string_pretty(&result)?);

    Ok(())
}

async fn handle_delete(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_id = matches.get_one::<String>("block-id").unwrap();

    let result = service
        .mcp()
        .call_tool(
            "block_delete_block",
            serde_json::json!({ "block_id": block_id }),
        )
        .await?;
    println!("{}", serde_json::to_string_pretty(&result)?);

    Ok(())
}

async fn handle_move(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_ids = matches
        .get_many::<String>("block-ids")
        .unwrap()
        .cloned()
        .collect::<Vec<_>>();
    let parent_block_id = matches.get_one::<String>("parent-block-id").unwrap();
    let after_block_id = matches.get_one::<String>("after-block-id");

    let mut args = serde_json::json!({
        "block_ids": block_ids,
        "parent_block_id": parent_block_id,
    });
    if let Some(after) = after_block_id {
        args["after_block_id"] = serde_json::json!(after);
    }

    let result = service.mcp().call_tool("block_move_blocks", args).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);

    Ok(())
}

async fn handle_find(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let query = matches.get_one::<String>("query").unwrap();
    let mode_str = matches
        .get_one::<String>("mode")
        .map(std::string::String::as_str)
        .unwrap_or("text");
    let block_id = matches.get_one::<String>("block-id");
    let entry_id = matches.get_one::<String>("entry-id");
    let limit: usize = matches
        .get_one::<String>("limit")
        .map(|s| s.parse::<usize>().unwrap_or(20))
        .unwrap_or(20);

    if entry_id.is_none() {
        eprintln!("warning: --entry-id is usually required. Try --entry-id <ENTRY_ID>");
    }

    // Parse mode
    use crate::service::block::reader::FindMode;
    let mode = match mode_str {
        "heading" | "h" => FindMode::Heading,
        "type" | "t" => FindMode::Type,
        _ => FindMode::Text,
    };

    let root_id = block_id.map(std::string::String::as_str).unwrap_or("");
    let entry_str = entry_id.map(std::string::String::as_str);

    let matches_result = service.find_blocks(root_id, query, mode, entry_str).await?;

    // Truncate to limit
    let display: Vec<_> = matches_result.into_iter().take(limit).collect();

    if display.is_empty() {
        println!("No blocks found for query: {}", query);
        return Ok(());
    }

    println!(
        "Found {} matching blocks (showing {}):\n",
        display.len(),
        display.len()
    );

    for (i, m) in display.iter().enumerate() {
        let depth = m.path.len() - 1;
        let indent = "  ".repeat(depth);
        let text_preview = m
            .text
            .as_deref()
            .unwrap_or("")
            .chars()
            .take(80)
            .collect::<String>();
        let type_name = m.block_type.as_str();
        println!(
            "[{}] {}[{}] {} \"{}\"",
            i + 1,
            indent,
            type_name,
            &m.id[..std::cmp::min(12, m.id.len())],
            text_preview
        );
        if text_preview.chars().count() >= 80 {
            println!("{}   ...", indent);
        }
        println!("{}   path: {}", indent, m.path.join(" → "));
        println!();
    }
    // Output JSON for programmatic use
    let json_output = serde_json::json!({ "query": query, "mode": mode_str, "count": display.len(), "matches": display });
    println!("{}", serde_json::to_string_pretty(&json_output)?);

    Ok(())
}

// ══════════════════════════════════════════════════
//  转换处理函数
// ══════════════════════════════════════════════════

async fn handle_convert(_service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let raw = resolve_content(matches)?;
    let from = matches
        .get_one::<String>("from")
        .map(std::string::String::as_str)
        .unwrap_or("auto");
    let to = matches
        .get_one::<String>("to")
        .map(std::string::String::as_str)
        .unwrap_or("blocks");

    // 检测输入类型
    let input_type = if from == "auto" {
        detect_content_type(&raw)
    } else {
        from
    };

    match (input_type, to) {
        ("mdx", "blocks" | "json") => {
            // MDX → blocks: 使用本地引擎（不经过 MCP）
            use crate::service::block::BlockService;
            let json = BlockService::mdx_to_blocks_local(&raw)?;
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        ("mdx", "mdx" | "markdown") => {
            // MDX → MDX (验证 round-trip): parse → emit
            use crate::service::block::{mdx::emit_mdx, mdx::parse_mdx};
            let doc = parse_mdx(&raw)?;
            print!("{}", emit_mdx(&doc));
        }
        ("blocks" | "json", "mdx" | "markdown") => {
            // blocks → MDX: 本地引擎
            use crate::service::block::BlockService;
            let parsed: serde_json::Value = serde_json::from_str(&raw)?;
            // 支持单个 block 对象或数组
            if let Some(arr) = parsed.as_array() {
                let blocks: Vec<crate::service::block::Block> = arr
                    .iter()
                    .map(crate::service::block::Block::from_json)
                    .collect();
                print!("{}", BlockService::blocks_to_mdx_local(&blocks)?);
            } else {
                let block = crate::service::block::Block::from_json(&parsed);
                print!("{}", BlockService::block_to_mdx_local(&block)?);
            }
        }
        _ => {
            // 同类型或未知，直接输出格式化的结果
            if input_type == "blocks" || input_type == "json" {
                let parsed: serde_json::Value = serde_json::from_str(&raw)?;
                println!("{}", serde_json::to_string_pretty(&parsed)?);
            } else {
                print!("{}", raw);
            }
        }
    }

    Ok(())
}

async fn handle_export(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_id = matches.get_one::<String>("block-id").unwrap();
    let format = matches
        .get_one::<String>("format")
        .map(std::string::String::as_str)
        .unwrap_or("mdx");

    match format {
        "json" => {
            let tree = service.get_tree(block_id, true).await?;
            println!("{}", serde_json::to_string_pretty(&tree)?);
        }
        "mdx" => {
            // 使用本地 MDX 引擎（完整语义保真：Callout/ColumnList/Table/Todo 等）
            let mdx = service.export_as_mdx(block_id).await?;
            print!("{}", mdx);
        }
        _ => {
            // markdown 兜底（旧链路，简单渲染）
            let md = service.blocks_to_markdown(block_id).await?;
            print!("{}", md);
        }
    }

    Ok(())
}

async fn handle_import(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_id = matches.get_one::<String>("block-id").unwrap();
    let file = matches.get_one::<String>("file").unwrap();
    let chunk_size = *matches.get_one::<usize>("chunk-size").unwrap();

    let content = std::fs::read_to_string(file)?;

    // 检测内容类型：MDX 使用本地引擎，Markdown 走旧 MCP 链路
    let content_type = detect_content_type(&content);

    match content_type {
        "mdx" => {
            // 本地引擎：MDX → DocIR → ir_to_descendant() → 分批插入
            service.import_mdx(block_id, &content, chunk_size).await?;
            println!(
                "\u{2713} Imported from {} using local MDX engine (chunk_size={})",
                file, chunk_size
            );
        }
        _ => {
            // 旧链路：Markdown → MCP 服务端转换
            service
                .import_markdown(block_id, &content, chunk_size)
                .await?;
            println!(
                "\u{2713} Imported from {} via MCP server (chunk_size={})",
                file, chunk_size
            );
        }
    }

    Ok(())
}

// ══════════════════════════════════════════════════
//  高级操作处理函数（保留原有逻辑）
// ══════════════════════════════════════════════════

async fn handle_table_get(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_id = matches.get_one::<String>("block-id").unwrap();
    let format = matches
        .get_one::<String>("format")
        .map(std::string::String::as_str)
        .unwrap_or("table");

    let table = service.get_table(block_id).await?;

    match format {
        "json" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "block_id": table.block_id,
                    "headers": table.headers.iter().map(|c| &c.text).collect::<Vec<_>>(),
                    "rows": table.rows.iter().map(|r| {
                        r.cells.iter().map(|c| &c.text).collect::<Vec<_>>()
                    }).collect::<Vec<_>>(),
                }))?
            );
        }
        "csv" => {
            let header_texts: Vec<&str> = table.headers.iter().map(|c| c.text.as_str()).collect();
            println!("{}", header_texts.join(","));
            for row in &table.rows {
                let cell_texts: Vec<&str> = row.cells.iter().map(|c| c.text.as_str()).collect();
                println!("{}", cell_texts.join(","));
            }
        }
        "markdown" => {
            let header_texts: Vec<&str> = table.headers.iter().map(|c| c.text.as_str()).collect();
            println!("| {} |", header_texts.join(" | "));
            let sep: Vec<&str> = header_texts.iter().map(|_| "---").collect();
            println!("| {} |", sep.join(" | "));
            for row in &table.rows {
                let cell_texts: Vec<&str> = row.cells.iter().map(|c| c.text.as_str()).collect();
                println!("| {} |", cell_texts.join(" | "));
            }
        }
        _ => {
            let header_texts: Vec<&str> = table.headers.iter().map(|c| c.text.as_str()).collect();
            let mut col_widths: Vec<usize> = header_texts.iter().map(|h| h.len()).collect();
            for row in &table.rows {
                for (i, cell) in row.cells.iter().enumerate() {
                    if i < col_widths.len() {
                        col_widths[i] = col_widths[i].max(cell.text.len());
                    }
                }
            }
            let header_line: Vec<String> = header_texts
                .iter()
                .enumerate()
                .map(|(i, h)| format!("{:width$}", h, width = col_widths[i]))
                .collect();
            println!("  {}", header_line.join("  "));
            let sep_line: Vec<String> = col_widths.iter().map(|w| "-".repeat(*w)).collect();
            println!("  {}", sep_line.join("  "));
            for row in &table.rows {
                let cells: Vec<String> = row
                    .cells
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let width = col_widths.get(i).copied().unwrap_or(0);
                        format!("{:width$}", c.text, width = width)
                    })
                    .collect();
                println!("  {}", cells.join("  "));
            }
        }
    }

    Ok(())
}

async fn handle_table_set(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_id = matches.get_one::<String>("block-id").unwrap();
    let row = *matches.get_one::<usize>("row").unwrap();
    let col = *matches.get_one::<usize>("col").unwrap();
    let text = matches.get_one::<String>("text").unwrap();

    service.set_cell(block_id, row, col, text).await?;
    println!("\u{2713} Cell [{}, {}] updated", row, col);

    Ok(())
}

async fn handle_table_add_row(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_id = matches.get_one::<String>("block-id").unwrap();
    let values_str = matches.get_one::<String>("values").unwrap();
    let values: Vec<&str> = values_str.split(',').map(str::trim).collect();

    service.add_row(block_id, &values).await?;
    println!("\u{2713} Row added with {} cells", values.len());

    Ok(())
}

async fn handle_table_del_row(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_id = matches.get_one::<String>("block-id").unwrap();
    let row = *matches.get_one::<usize>("row").unwrap();

    service.delete_row(block_id, row).await?;
    println!("\u{2713} Row {} deleted", row);

    Ok(())
}

fn get_content(matches: &clap::ArgMatches) -> Result<String> {
    if let Some(content) = matches.get_one::<String>("content") {
        return Ok(content.clone());
    }
    if let Some(file) = matches.get_one::<String>("file") {
        return Ok(std::fs::read_to_string(file)?);
    }
    anyhow::bail!("Either --content or --file is required");
}

async fn handle_replace_section(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_id = matches.get_one::<String>("block-id").unwrap();
    let heading = matches.get_one::<String>("heading").unwrap();
    let content = get_content(matches)?;

    service.replace_section(block_id, heading, &content).await?;
    println!("\u{2713} Section '{}' replaced", heading);

    Ok(())
}

async fn handle_insert_after(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_id = matches.get_one::<String>("block-id").unwrap();
    let content = get_content(matches)?;

    service.insert_after(block_id, &content).await?;
    println!("\u{2713} Content inserted after block {}", block_id);

    Ok(())
}

async fn handle_append(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_id = matches.get_one::<String>("block-id").unwrap();
    let content = get_content(matches)?;

    service.append(block_id, &content).await?;
    println!("\u{2713} Content appended to block {}", block_id);

    Ok(())
}

async fn handle_tree(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_id = matches.get_one::<String>("block-id").unwrap();
    let recursive = matches.get_flag("recursive");

    let tree = service.get_tree(block_id, recursive).await?;
    print_block_tree(&tree.children, 0);

    Ok(())
}

fn print_block_tree(blocks: &[crate::service::block::Block], depth: usize) {
    for block in blocks {
        let _indent = "  ".repeat(depth);
        let type_str = block.block_type.as_str();
        let text_preview = block
            .text
            .as_deref()
            .unwrap_or("")
            .chars()
            .take(60)
            .collect::<String>();

        if text_preview.is_empty() {
            println!("├─ [{}] {}", type_str, block.id);
        } else {
            println!("├─ [{}] {} \"{}\"", type_str, block.id, text_preview);
        }

        if !block.children.is_empty() {
            print_block_tree(&block.children, depth + 1);
        }
    }
}

// ══════════════════════════════════════════════════
//  内容解析与自动转换辅助函数
// ══════════════════════════════════════════════════

/// 从 matches 中解析原始内容（--descendant > --content > --file）
fn resolve_content(matches: &clap::ArgMatches) -> Result<String> {
    // --descendant 优先（完整块数据或 MDX）
    if let Some(desc) = matches.get_one::<String>("descendant") {
        return Ok(desc.clone());
    }
    // --content 次之
    if let Some(content) = matches.get_one::<String>("content") {
        return Ok(content.clone());
    }
    // --file 最后
    if let Some(file) = matches.get_one::<String>("file") {
        return Ok(std::fs::read_to_string(file)?);
    }
    Err(anyhow::anyhow!(
        "No content provided. Use --descendant, --content, or --file"
    ))
}

/// 检测输入内容类型
fn detect_content_type(content: &str) -> &'static str {
    let trimmed = content.trim();
    // 以 [ 或 { 开头，尝试作为 JSON 解析
    if (trimmed.starts_with('[') || trimmed.starts_with('{'))
        && serde_json::from_str::<serde_json::Value>(trimmed).is_ok()
    {
        return "blocks";
    }
    // 默认视为 MDX / Markdown
    "mdx"
}

/// 将原始内容转换为 descendant 结构（用于 create）
async fn resolve_descendant(
    service: &BlockService,
    raw: &str,
    content_type: &str,
) -> Result<serde_json::Value> {
    let detected = if content_type == "auto" {
        detect_content_type(raw)
    } else {
        content_type
    };

    match detected {
        "blocks" => {
            // 已经是 JSON，直接使用
            Ok(serde_json::from_str(raw)?)
        }
        "mdx" => {
            // MDX → blocks（本地引擎，不经过 MCP 转换器）
            use crate::service::block::BlockService;
            BlockService::mdx_to_blocks_local(raw)
        }
        "markdown" => {
            // 旧链路：Markdown → MCP 服务端转换
            service.markdown_to_blocks(raw).await
        }
        _ => service.markdown_to_blocks(raw).await,
    }
}

/// 将原始内容转换为 update 数据（用于 update）
async fn resolve_update_data(
    service: &BlockService,
    raw: &str,
    content_type: &str,
) -> Result<serde_json::Value> {
    let detected = if content_type == "auto" {
        detect_content_type(raw)
    } else {
        content_type
    };

    match detected {
        "blocks" => {
            // 已经是完整的 block JSON，包装为 update 参数
            let parsed: serde_json::Value = serde_json::from_str(raw)?;
            Ok(parsed)
        }
        "mdx" => {
            // MDX → blocks（本地引擎，不经过 MCP 转换器）
            use crate::service::block::BlockService;
            BlockService::mdx_to_blocks_local(raw)
        }
        "markdown" => {
            // 旧链路：先转为 blocks 再包装
            let descendant = service.markdown_to_blocks(raw).await?;
            Ok(descendant)
        }
        _ => {
            let descendant = service.markdown_to_blocks(raw).await?;
            Ok(descendant)
        }
    }
}
