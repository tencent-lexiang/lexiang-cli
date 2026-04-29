//! 内容转换：markdown / mdx ↔ blocks
//!
//! 两条链路:
//!   - **Markdown 链路**: 调 MCP `block_convert_content_to_blocks`，服务端转换
//!   - **MDX 链路**: 本地 MDX parser → `DocIR` → `ir_to_descendant()` → 直接构造 Block JSON

use super::adapter::{block_to_ir, ir_to_descendant};
use super::mdx::{emit_mdx, parse_mdx};
use super::types::{Block, BlockType};
use super::BlockService;
use anyhow::Result;

// ============================================================
//  Markdown 渲染（纯函数，用于 git 导出等场景）
// ============================================================

/// 纯函数：Block 树转 markdown（不需要 MCP 调用）
///
/// 支持强类型 Block 和嵌套子块递归。
pub fn render_blocks_to_markdown(blocks: &[Block]) -> String {
    let mut lines = Vec::new();
    render_blocks_recursive(blocks, &mut lines, 0);
    lines.join("\n\n")
}

fn render_blocks_recursive(blocks: &[Block], lines: &mut Vec<String>, depth: usize) {
    for block in blocks {
        let text = block.text.as_deref().unwrap_or("");

        let line = match &block.block_type {
            BlockType::H1 => format!("# {}", text),
            BlockType::H2 => format!("## {}", text),
            BlockType::H3 => format!("### {}", text),
            BlockType::H4 => format!("#### {}", text),
            BlockType::H5 => format!("##### {}", text),
            BlockType::Code => {
                let lang = block
                    .content
                    .get("language")
                    .and_then(|l| l.as_str())
                    .unwrap_or("");
                format!("```{}\n{}\n```", lang, text)
            }
            BlockType::BulletList | BlockType::ListItem => {
                let indent = "  ".repeat(depth);
                format!("{}- {}", indent, text)
            }
            BlockType::NumberedList => {
                let indent = "  ".repeat(depth);
                format!("{}1. {}", indent, text)
            }
            BlockType::Quote => format!("> {}", text),
            BlockType::Divider => "---".to_string(),
            BlockType::Task => {
                let done = block
                    .content
                    .get("done")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                let checkbox = if done { "[x]" } else { "[ ]" };
                format!("- {} {}", checkbox, text)
            }
            BlockType::Image => {
                let url = block
                    .content
                    .get("url")
                    .and_then(|u| u.as_str())
                    .unwrap_or("");
                format!("![{}]({})", text, url)
            }
            BlockType::Table => {
                // 表格由子块（table_row > table_cell）渲染
                let mut table_lines = Vec::new();
                render_table_blocks(&block.children, &mut table_lines);
                table_lines.join("\n")
            }
            BlockType::TableRow | BlockType::TableCell => {
                // 通常由 Table 处理，单独出现时直接输出文本
                text.to_string()
            }
            _ => text.to_string(),
        };

        if !line.is_empty() {
            lines.push(line);
        }

        // 递归处理子块（Table 已经自行处理子块，跳过）
        if block.block_type != BlockType::Table
            && block.block_type != BlockType::TableRow
            && block.block_type != BlockType::TableCell
            && !block.children.is_empty()
        {
            render_blocks_recursive(&block.children, lines, depth + 1);
        }
    }
}

/// 渲染表格块为 markdown 表格
fn render_table_blocks(rows: &[Block], lines: &mut Vec<String>) {
    for (i, row) in rows.iter().enumerate() {
        let cells: Vec<String> = row
            .children
            .iter()
            .map(|cell| cell.text.as_deref().unwrap_or("").to_string())
            .collect();

        lines.push(format!("| {} |", cells.join(" | ")));

        // 表头后加分隔线
        if i == 0 {
            let sep: Vec<String> = cells.iter().map(|_| "---".to_string()).collect();
            lines.push(format!("| {} |", sep.join(" | ")));
        }
    }
}

/// 纯函数：从原始 JSON 数组转 markdown（兼容 cmd/git 模块）
#[allow(dead_code)]
pub fn render_json_blocks_to_markdown(blocks: &[serde_json::Value]) -> String {
    let typed: Vec<Block> = blocks.iter().map(Block::from_json).collect();
    render_blocks_to_markdown(&typed)
}

impl BlockService {
    /// markdown → Block 结构（调用 `block_convert_content_to_blocks`）
    ///
    /// 返回原始 JSON（与 `CreateBlockDescendant` 兼容的 descendant 结构）
    pub async fn markdown_to_blocks(&self, markdown: &str) -> Result<serde_json::Value> {
        let result = self
            .mcp
            .call_tool(
                "block_convert_content_to_blocks",
                serde_json::json!({
                    "content": markdown,
                    "content_type": "markdown",
                }),
            )
            .await?;

        // 返回 { "data": { "descendant": { ... } } } 中的 descendant
        let descendant = result
            .get("data")
            .and_then(|d| d.get("descendant"))
            .cloned()
            .unwrap_or_else(|| result.get("descendant").cloned().unwrap_or(result.clone()));

        Ok(descendant)
    }

    /// 块树 → markdown
    pub async fn blocks_to_markdown(&self, root_id: &str) -> Result<String> {
        let tree = self.get_tree(root_id, true).await?;
        Ok(render_blocks_to_markdown(&tree.children))
    }

    /// 大文档分块导入
    ///
    /// 将 markdown 转换为 blocks 后，按 `chunk_size` 分批调用
    /// `block_create_block_descendant` 插入。
    pub async fn import_markdown(
        &self,
        parent_id: &str,
        markdown: &str,
        chunk_size: usize,
    ) -> Result<()> {
        let descendant = self.markdown_to_blocks(markdown).await?;

        // 获取 children 数组
        let children = descendant
            .get("children")
            .and_then(|c| c.as_array())
            .cloned()
            .unwrap_or_default();

        if children.is_empty() {
            // 没有子块，直接创建整个 descendant
            self.mcp
                .call_tool(
                    "block_create_block_descendant",
                    serde_json::json!({
                        "block_id": parent_id,
                        "descendant": descendant,
                    }),
                )
                .await?;
            return Ok(());
        }

        // 分块插入
        for chunk in children.chunks(chunk_size) {
            let chunk_descendant = serde_json::json!({
                "children": chunk,
            });

            self.mcp
                .call_tool(
                    "block_create_block_descendant",
                    serde_json::json!({
                        "block_id": parent_id,
                        "descendant": chunk_descendant,
                    }),
                )
                .await?;
        }

        Ok(())
    }

    // ============================================================
    //  新链路：本地 MDX 引擎（不经过 MCP 转换器）
    // ============================================================

    /// Block 树 → MDX 文本（本地引擎，完整语义保真）
    ///
    /// 流程: Block JSON → `block_to_ir()` → `DocIR` → `emit_mdx()` → String
    pub fn blocks_to_mdx_local(blocks: &[Block]) -> Result<String> {
        let doc = block_to_ir(blocks);
        let mdx = emit_mdx(&doc);
        Ok(mdx)
    }

    /// 单个 Block → MDX（便捷方法）
    pub fn block_to_mdx_local(block: &Block) -> Result<String> {
        Self::blocks_to_mdx_local(std::slice::from_ref(block))
    }

    /// MDX 文本 → 完整 descendant JSON（本地引擎）
    ///
    /// 流程: MDX string → `parse_mdx()` → `DocIR` → `ir_to_descendant()` → JSON
    ///
    /// 返回的 JSON 可直接传给 `block_create_block_descendant` 的 `descendant` 参数。
    /// **不调用 MCP** `block_convert_content_to_blocks`。
    pub fn mdx_to_blocks_local(mdx_str: &str) -> Result<serde_json::Value> {
        let doc = parse_mdx(mdx_str)?;
        let json = ir_to_descendant(&doc);
        Ok(json)
    }

    /// MDX 文本导入到指定父块（两阶段安全写入）
    ///
    /// ## 设计说明
    ///
    /// `block_create_block_descendant` 有两个关键行为：
    /// 1. `descendant` 参数是**块数组**，不是嵌套对象
    /// 2. 创建的子块会**替换**父块的现有所有子块（不是追加！）
    /// 3. 需要 `entry_id` 参数
    /// 4. 嵌套 children 在创建时必须是**字符串 ID 数组**
    ///
    /// 因此采用**两阶段写入**：
    /// **阶段 1**: 扁平化 `DocIR` 树，创建所有顶层块（children 为空），获取返回的 `block_id`
    /// **阶段 2**: 对有子块的容器块（callout/toggle/columns 等），逐个用返回的 ID 创建子块
    pub async fn import_mdx(
        &self,
        parent_id: &str,
        entry_id: Option<&str>,
        mdx_str: &str,
        chunk_size: usize,
    ) -> Result<()> {
        // 验证 entry_id
        let eid = entry_id.ok_or_else(|| {
            anyhow::anyhow!("entry_id is required for import_mdx. Use --entry-id <page_id>")
        })?;

        // Phase 1: 解析 + 扁平化
        let doc = parse_mdx(mdx_str)?;
        let flat = FlattenDocIr::flatten(&doc);

        eprintln!(
            "[import_mdx] flattened: {} top-level blocks, {} container blocks with pending children",
            flat.blocks.len(),
            flat.containers.len()
        );

        if flat.blocks.is_empty() {
            println!("  (empty document, nothing to import)");
            return Ok(());
        }

        // Phase 2a: 分批创建顶层块（每批 chunk_size 个，children 全部为空）
        let mut created_ids: Vec<String> = Vec::with_capacity(flat.blocks.len());
        for (i, chunk) in flat.blocks.chunks(chunk_size).enumerate() {
            let params = serde_json::json!({
                "block_id": parent_id,
                "entry_id": eid,
                "descendant": chunk,  // 扁平块数组，每个块的 children: []
            });
            eprintln!(
                "[import_mdx] phase 2a: batch {}/{}, creating {} top-level blocks",
                i + 1,
                flat.blocks.len().div_ceil(chunk_size),
                chunk.len()
            );

            let result = self
                .mcp
                .call_tool("block_create_block_descendant", params)
                .await?;

            // 检查 API 错误
            check_api_result(&result)?;

            // 收集创建后的 block_id
            if let Some(data) = result.get("data") {
                if let Some(desc) = data.get("descendant") {
                    collect_created_ids(desc, &mut created_ids);
                } else if data.get("id").is_some() {
                    created_ids.push(data["id"].as_str().unwrap_or("").to_string());
                } else if let Some(arr) = data.get("children").and_then(|v| v.as_array()) {
                    for item in arr.iter() {
                        if let Some(id) = item
                            .get("id")
                            .or(item.get("block_id"))
                            .and_then(|v| v.as_str())
                        {
                            created_ids.push(id.to_string());
                        }
                    }
                }
            }
        }

        eprintln!(
            "[import_mdx] phase 2a complete: created {} blocks",
            created_ids.len()
        );

        // 如果没有容器块需要处理子块，直接完成
        if flat.containers.is_empty() {
            return Ok(());
        }

        // Phase 2b: 为容器块创建子块（递归处理嵌套容器）
        // 建立 block 索引: flat_index -> created_block_id
        // 注意：created_ids 的顺序与 flat.blocks 的顺序一致（API 按输入顺序返回）
        let mut id_map: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
        for (i, _block_json) in flat.blocks.iter().enumerate() {
            if i < created_ids.len() {
                id_map.insert(i, created_ids[i].clone());
            } else {
                eprintln!("[import_mdx] warning: block {} has no returned ID", i);
            }
        }

        // 处理每个容器的子块
        for container_info in &flat.containers {
            let parent_flat_idx = container_info.self_idx;
            let child_indices = &container_info.child_indices;

            // 查找父块的 API 返回 ID
            let parent_api_id = match id_map.get(&parent_flat_idx) {
                Some(id) => id.clone(),
                None => {
                    eprintln!(
                        "[import_mdx] warning: no API ID for container at flat index {}, skipping children",
                        parent_flat_idx
                    );
                    continue;
                }
            };

            // 获取子块 JSON（已经是扁平格式）
            let child_blocks: Vec<serde_json::Value> = child_indices
                .iter()
                .filter_map(|&idx| flat.blocks.get(idx).cloned())
                .collect();

            if child_blocks.is_empty() {
                continue;
            }

            eprintln!(
                "[import_mdx] phase 2b: creating {} children for container {} ({})",
                child_blocks.len(),
                parent_flat_idx,
                parent_api_id
            );

            // 分批创建子块
            for chunk in child_blocks.chunks(chunk_size) {
                let params = serde_json::json!({
                    "block_id": parent_api_id,
                    "entry_id": eid,
                    "descendant": chunk,
                });

                let result = self
                    .mcp
                    .call_tool("block_create_block_descendant", params)
                    .await?;
                check_api_result(&result)?;

                // 记录子块 ID 到 id_map
                if let Some(data) = result.get("data") {
                    if let Some(desc) = data.get("descendant") {
                        let before = created_ids.len();
                        collect_created_ids(desc, &mut created_ids);
                        let new_count = created_ids.len() - before;
                        // 按顺序映射新创建的子块 ID 到对应的 flat 索引
                        let mut mapped = 0;
                        for ci in child_indices.iter() {
                            if !id_map.contains_key(ci) && mapped < new_count {
                                id_map.insert(*ci, created_ids[before + mapped].clone());
                                mapped += 1;
                            }
                        }
                    }
                }
            }
        }

        eprintln!("[import_mdx] import complete!");
        Ok(())
    }

    /// 获取整个文档为 MDX（通过 `get_tree` + 本地转换）
    pub async fn export_as_mdx(&self, root_id: &str) -> Result<String> {
        let tree = self.get_tree(root_id, true).await?;
        Self::blocks_to_mdx_local(&tree.children)
    }
}

// ============================================================
//  DocIR 扁平化工具（两阶段写入辅助）
// ============================================================

/// 扁平化的 `DocIR` 结构：用于两阶段 API 写入
struct FlattenDocIr {
    /// 所有块（扁平列表，每个块的 children 为空数组 []）
    blocks: Vec<serde_json::Value>,
    /// 容器信息：哪些块是容器，它们的子块在 blocks 中的索引
    containers: Vec<ContainerInfo>,
}

struct ContainerInfo {
    self_idx: usize,
    child_indices: Vec<usize>,
}

impl FlattenDocIr {
    fn flatten(doc: &super::ir::Node) -> Self {
        let mut blocks = Vec::new();
        let mut containers = Vec::new();

        for child in &doc.children {
            flatten_node(child, &mut blocks, &mut containers);
        }

        FlattenDocIr { blocks, containers }
    }
}

fn flatten_node(
    node: &super::ir::Node,
    blocks: &mut Vec<serde_json::Value>,
    containers: &mut Vec<ContainerInfo>,
) -> usize {
    let idx = blocks.len();
    let mut block_json = super::adapter::ir_block(node);
    block_json["children"] = serde_json::json!([]);

    let has_children = !node.children.is_empty();
    let is_cont = matches!(
        node.node_type,
        super::ir::NodeType::Callout { .. }
            | super::ir::NodeType::Toggle
            | super::ir::NodeType::ColumnList
            | super::ir::NodeType::Column { .. }
            | super::ir::NodeType::Table
            | super::ir::NodeType::Document
            | super::ir::NodeType::BlockQuote
    );

    blocks.push(block_json);

    if has_children {
        let mut child_indices = Vec::new();
        for child in &node.children {
            let child_idx = flatten_node(child, blocks, containers);
            child_indices.push(child_idx);
        }
        if is_cont {
            containers.push(ContainerInfo {
                self_idx: idx,
                child_indices,
            });
        }
    }

    idx
}

/// 检查 MCP API 响应是否成功
fn check_api_result(result: &serde_json::Value) -> Result<()> {
    let code = result
        .get("code")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    if code != 0 {
        let message = result
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        anyhow::bail!("API error (code={}): {}", code, message);
    }
    Ok(())
}

/// 从 API 返回中提取所有 `block_id`
fn collect_created_ids(descendant: &serde_json::Value, ids: &mut Vec<String>) {
    if let Some(id) = descendant
        .get("id")
        .or(descendant.get("block_id"))
        .and_then(|v| v.as_str())
    {
        ids.push(id.to_string());
    }
    if let Some(children) = descendant.get("children").and_then(|v| v.as_array()) {
        for child in children {
            collect_created_ids(child, ids);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_simple_blocks() {
        let blocks = vec![
            Block {
                id: "1".to_string(),
                block_type: BlockType::H1,
                text: Some("Title".to_string()),
                content: serde_json::json!({"text": "Title"}),
                children: vec![],
            },
            Block {
                id: "2".to_string(),
                block_type: BlockType::Paragraph,
                text: Some("Hello world".to_string()),
                content: serde_json::json!({"text": "Hello world"}),
                children: vec![],
            },
        ];

        let md = render_blocks_to_markdown(&blocks);
        assert!(md.contains("# Title"));
        assert!(md.contains("Hello world"));
    }

    #[test]
    fn test_render_code_block() {
        let blocks = vec![Block {
            id: "1".to_string(),
            block_type: BlockType::Code,
            text: Some("fn main() {}".to_string()),
            content: serde_json::json!({"text": "fn main() {}", "language": "rust"}),
            children: vec![],
        }];

        let md = render_blocks_to_markdown(&blocks);
        assert!(md.contains("```rust"));
        assert!(md.contains("fn main() {}"));
    }

    #[test]
    fn test_render_table() {
        let blocks = vec![Block {
            id: "tbl".to_string(),
            block_type: BlockType::Table,
            text: None,
            content: serde_json::json!({}),
            children: vec![
                Block {
                    id: "r0".to_string(),
                    block_type: BlockType::TableRow,
                    text: None,
                    content: serde_json::json!({}),
                    children: vec![
                        Block {
                            id: "c0".to_string(),
                            block_type: BlockType::TableCell,
                            text: Some("Name".to_string()),
                            content: serde_json::json!({}),
                            children: vec![],
                        },
                        Block {
                            id: "c1".to_string(),
                            block_type: BlockType::TableCell,
                            text: Some("Value".to_string()),
                            content: serde_json::json!({}),
                            children: vec![],
                        },
                    ],
                },
                Block {
                    id: "r1".to_string(),
                    block_type: BlockType::TableRow,
                    text: None,
                    content: serde_json::json!({}),
                    children: vec![
                        Block {
                            id: "c2".to_string(),
                            block_type: BlockType::TableCell,
                            text: Some("foo".to_string()),
                            content: serde_json::json!({}),
                            children: vec![],
                        },
                        Block {
                            id: "c3".to_string(),
                            block_type: BlockType::TableCell,
                            text: Some("bar".to_string()),
                            content: serde_json::json!({}),
                            children: vec![],
                        },
                    ],
                },
            ],
        }];

        let md = render_blocks_to_markdown(&blocks);
        assert!(md.contains("| Name | Value |"));
        assert!(md.contains("| --- | --- |"));
        assert!(md.contains("| foo | bar |"));
    }

    #[test]
    fn test_render_json_blocks_compat() {
        let json_blocks = vec![
            serde_json::json!({ "id": "1", "type": "h2", "content": { "text": "Heading" } }),
            serde_json::json!({ "id": "2", "type": "paragraph", "content": { "text": "Body" } }),
        ];
        let md = render_json_blocks_to_markdown(&json_blocks);
        assert!(md.contains("## Heading"));
        assert!(md.contains("Body"));
    }
}
