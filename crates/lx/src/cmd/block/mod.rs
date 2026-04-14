//! Block 高级 CLI 命令
//!
//! 提供 `lx block` 命名空间下的高级操作命令，
//! 与动态生成的原子命令共存。

use crate::config::Config;
use crate::mcp::RealMcpCaller;
use crate::service::block::BlockService;
use anyhow::Result;
use clap::{Arg, ArgAction, Command};

/// 高级 block 子命令名列表（用于判断是否命中静态命令）
pub const HIGH_LEVEL_SUBCOMMANDS: &[&str] = &[
    "table-get",
    "table-set",
    "table-add-row",
    "table-del-row",
    "replace-section",
    "insert-after",
    "append",
    "export",
    "import",
    "tree",
];

/// 构建高级 block 子命令
pub fn build_block_commands() -> Vec<Command> {
    vec![
        Command::new("table-get")
            .about("Get table as structured view")
            .arg(
                Arg::new("block-id")
                    .long("block-id")
                    .required(true)
                    .help("Table block ID"),
            )
            .arg(
                Arg::new("format")
                    .long("format")
                    .short('f')
                    .default_value("table")
                    .help("Output format: table, json, csv, markdown"),
            ),
        Command::new("table-set")
            .about("Set a cell value in a table")
            .arg(
                Arg::new("block-id")
                    .long("block-id")
                    .required(true)
                    .help("Table block ID"),
            )
            .arg(
                Arg::new("row")
                    .long("row")
                    .required(true)
                    .value_parser(clap::value_parser!(usize))
                    .help("Row index (0-based, excluding header)"),
            )
            .arg(
                Arg::new("col")
                    .long("col")
                    .required(true)
                    .value_parser(clap::value_parser!(usize))
                    .help("Column index (0-based)"),
            )
            .arg(
                Arg::new("text")
                    .long("text")
                    .required(true)
                    .help("New cell text"),
            ),
        Command::new("table-add-row")
            .about("Append a row to a table")
            .arg(
                Arg::new("block-id")
                    .long("block-id")
                    .required(true)
                    .help("Table block ID"),
            )
            .arg(
                Arg::new("values")
                    .long("values")
                    .required(true)
                    .help("Comma-separated cell values"),
            ),
        Command::new("table-del-row")
            .about("Delete a row from a table")
            .arg(
                Arg::new("block-id")
                    .long("block-id")
                    .required(true)
                    .help("Table block ID"),
            )
            .arg(
                Arg::new("row")
                    .long("row")
                    .required(true)
                    .value_parser(clap::value_parser!(usize))
                    .help("Row index (0-based, excluding header)"),
            ),
        Command::new("replace-section")
            .about("Replace content under a heading")
            .arg(
                Arg::new("block-id")
                    .long("block-id")
                    .required(true)
                    .help("Root block ID"),
            )
            .arg(
                Arg::new("heading")
                    .long("heading")
                    .required(true)
                    .help("Heading text (e.g. '## API')"),
            )
            .arg(
                Arg::new("content")
                    .long("content")
                    .help("New markdown content"),
            )
            .arg(Arg::new("file").long("file").help("Read content from file")),
        Command::new("insert-after")
            .about("Insert content after a block")
            .arg(
                Arg::new("block-id")
                    .long("block-id")
                    .required(true)
                    .help("Block ID to insert after"),
            )
            .arg(Arg::new("content").long("content").help("Markdown content"))
            .arg(Arg::new("file").long("file").help("Read content from file")),
        Command::new("append")
            .about("Append content to a parent block")
            .arg(
                Arg::new("block-id")
                    .long("block-id")
                    .required(true)
                    .help("Parent block ID"),
            )
            .arg(Arg::new("content").long("content").help("Markdown content"))
            .arg(Arg::new("file").long("file").help("Read content from file")),
        Command::new("export")
            .about("Export block tree as markdown or json")
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
                    .default_value("markdown")
                    .help("Output format: markdown, json"),
            ),
        Command::new("import")
            .about("Import markdown file into block tree")
            .arg(
                Arg::new("block-id")
                    .long("block-id")
                    .required(true)
                    .help("Parent block ID"),
            )
            .arg(
                Arg::new("file")
                    .long("file")
                    .required(true)
                    .help("Markdown file path"),
            )
            .arg(
                Arg::new("chunk-size")
                    .long("chunk-size")
                    .default_value("20")
                    .value_parser(clap::value_parser!(usize))
                    .help("Blocks per batch"),
            ),
        Command::new("tree")
            .about("Display block tree structure")
            .arg(
                Arg::new("block-id")
                    .long("block-id")
                    .required(true)
                    .help("Root block ID"),
            )
            .arg(
                Arg::new("recursive")
                    .long("recursive")
                    .short('r')
                    .action(ArgAction::SetTrue)
                    .help("Recursively list all descendants"),
            ),
    ]
}

/// 尝试处理高级 block 子命令
///
/// 返回 Ok(true) 表示已处理，Ok(false) 表示不是高级命令应回退到动态命令。
pub async fn try_handle_block_command(args: &[String]) -> Result<bool> {
    // args: ["lx", "block", <subcommand>, ...]
    if args.len() < 3 {
        return Ok(false);
    }

    let subcommand = &args[2];

    // --help / -h 特殊处理：显示高级命令帮助后回退
    if subcommand == "--help" || subcommand == "-h" {
        return Ok(false);
    }

    if !HIGH_LEVEL_SUBCOMMANDS.contains(&subcommand.as_str()) {
        return Ok(false);
    }

    // 构建 clap 命令进行解析
    let cmd = Command::new("lx").subcommand_required(true).subcommand({
        let mut block_cmd = Command::new("block").about("Block operations");
        for sub in build_block_commands() {
            block_cmd = block_cmd.subcommand(sub);
        }
        block_cmd
    });

    let matches = match cmd.try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            // --help / --version 等非错误退出
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
        "table-get" => handle_table_get(&service, sub_matches).await?,
        "table-set" => handle_table_set(&service, sub_matches).await?,
        "table-add-row" => handle_table_add_row(&service, sub_matches).await?,
        "table-del-row" => handle_table_del_row(&service, sub_matches).await?,
        "replace-section" => handle_replace_section(&service, sub_matches).await?,
        "insert-after" => handle_insert_after(&service, sub_matches).await?,
        "append" => handle_append(&service, sub_matches).await?,
        "export" => handle_export(&service, sub_matches).await?,
        "import" => handle_import(&service, sub_matches).await?,
        "tree" => handle_tree(&service, sub_matches).await?,
        _ => return Ok(false),
    }

    Ok(true)
}

// ── 子命令处理函数 ──

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
            // table format (default) — 简单的文本表格输出
            let header_texts: Vec<&str> = table.headers.iter().map(|c| c.text.as_str()).collect();

            // 计算列宽
            let mut col_widths: Vec<usize> = header_texts.iter().map(|h| h.len()).collect();
            for row in &table.rows {
                for (i, cell) in row.cells.iter().enumerate() {
                    if i < col_widths.len() {
                        col_widths[i] = col_widths[i].max(cell.text.len());
                    }
                }
            }

            // 打印表头
            let header_line: Vec<String> = header_texts
                .iter()
                .enumerate()
                .map(|(i, h)| format!("{:width$}", h, width = col_widths[i]))
                .collect();
            println!("  {}", header_line.join("  "));

            // 分隔线
            let sep_line: Vec<String> = col_widths.iter().map(|w| "-".repeat(*w)).collect();
            println!("  {}", sep_line.join("  "));

            // 数据行
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
    println!("✓ Cell [{}, {}] updated", row, col);

    Ok(())
}

async fn handle_table_add_row(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_id = matches.get_one::<String>("block-id").unwrap();
    let values_str = matches.get_one::<String>("values").unwrap();
    let values: Vec<&str> = values_str.split(',').map(str::trim).collect();

    service.add_row(block_id, &values).await?;
    println!("✓ Row added with {} cells", values.len());

    Ok(())
}

async fn handle_table_del_row(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_id = matches.get_one::<String>("block-id").unwrap();
    let row = *matches.get_one::<usize>("row").unwrap();

    service.delete_row(block_id, row).await?;
    println!("✓ Row {} deleted", row);

    Ok(())
}

/// 读取 --content 或 --file 参数获取 markdown 内容
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
    println!("✓ Section '{}' replaced", heading);

    Ok(())
}

async fn handle_insert_after(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_id = matches.get_one::<String>("block-id").unwrap();
    let content = get_content(matches)?;

    service.insert_after(block_id, &content).await?;
    println!("✓ Content inserted after block {}", block_id);

    Ok(())
}

async fn handle_append(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_id = matches.get_one::<String>("block-id").unwrap();
    let content = get_content(matches)?;

    service.append(block_id, &content).await?;
    println!("✓ Content appended to block {}", block_id);

    Ok(())
}

async fn handle_export(service: &BlockService, matches: &clap::ArgMatches) -> Result<()> {
    let block_id = matches.get_one::<String>("block-id").unwrap();
    let format = matches
        .get_one::<String>("format")
        .map(std::string::String::as_str)
        .unwrap_or("markdown");

    match format {
        "json" => {
            let tree = service.get_tree(block_id, true).await?;
            println!("{}", serde_json::to_string_pretty(&tree)?);
        }
        _ => {
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
    service
        .import_markdown(block_id, &content, chunk_size)
        .await?;
    println!(
        "✓ Markdown imported from {} (chunk_size={})",
        file, chunk_size
    );

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
        let indent = "  ".repeat(depth);
        let type_str = block.block_type.as_str();
        let text_preview = block
            .text
            .as_deref()
            .unwrap_or("")
            .chars()
            .take(60)
            .collect::<String>();

        if text_preview.is_empty() {
            println!("{}├─ [{}] {}", indent, type_str, block.id);
        } else {
            println!(
                "{}├─ [{}] {} \"{}\"",
                indent, type_str, block.id, text_preview
            );
        }

        if !block.children.is_empty() {
            print_block_tree(&block.children, depth + 1);
        }
    }
}
