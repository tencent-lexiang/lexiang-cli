//! 内容转换：markdown ↔ blocks

use super::types::{Block, BlockType};
use super::BlockService;
use anyhow::Result;

/// 纯函数：Block 树转 markdown（不需要 MCP 调用）
///
/// 从 cmd/git/mod.rs 的 `blocks_to_markdown` 提取并增强，
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
