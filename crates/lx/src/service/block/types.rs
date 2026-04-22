//! Block Service 强类型定义
//!
//! 将 MCP 返回的 JSON 映射为 Rust 强类型，
//! 支持 table/heading/code 等结构化操作。

use serde::{Deserialize, Serialize};

/// 块类型枚举
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BlockType {
    Paragraph,
    H1,
    H2,
    H3,
    H4,
    H5,
    Code,
    Table,
    TableRow,
    TableCell,
    BulletList,
    NumberedList,
    ListItem,
    Quote,
    /// 高亮提示框（容器类型，内容在 children 中）
    Callout,
    Task,
    /// 折叠块（可展开/收起）
    Toggle,
    /// 分栏容器（children 仅限 Column）
    ColumnList,
    /// 分栏列
    Column,
    Image,
    Attachment,
    Video,
    Divider,
    Mermaid,
    PlantUml,
    /// 未识别类型，保留原始字符串
    Unknown(String),
}

impl BlockType {
    /// 从 MCP 返回的 type 字符串解析
    pub fn from_str(s: &str) -> Self {
        match s {
            "paragraph" | "text" | "" | "p" => Self::Paragraph,
            "h1" => Self::H1,
            "h2" => Self::H2,
            "h3" => Self::H3,
            "h4" => Self::H4,
            "h5" => Self::H5,
            "code" => Self::Code,
            "table" => Self::Table,
            "table_row" => Self::TableRow,
            "table_cell" => Self::TableCell,
            "bullet_list" | "bulleted_list" => Self::BulletList,
            "numbered_list" => Self::NumberedList,
            "list_item" | "listitem" => Self::ListItem,
            "quote" | "blockquote" => Self::Quote,
            "callout" => Self::Callout,
            "toggle" => Self::Toggle,
            "column_list" => Self::ColumnList,
            "column" => Self::Column,
            "task" | "task_item" => Self::Task,
            "image" => Self::Image,
            "attachment" => Self::Attachment,
            "video" => Self::Video,
            "divider" => Self::Divider,
            "mermaid" => Self::Mermaid,
            "plantuml" => Self::PlantUml,
            other => Self::Unknown(other.to_string()),
        }
    }

    /// 转回 MCP API 使用的字符串
    pub fn as_str(&self) -> &str {
        match self {
            Self::Paragraph => "paragraph",
            Self::H1 => "h1",
            Self::H2 => "h2",
            Self::H3 => "h3",
            Self::H4 => "h4",
            Self::H5 => "h5",
            Self::Code => "code",
            Self::Table => "table",
            Self::TableRow => "table_row",
            Self::TableCell => "table_cell",
            Self::BulletList => "bullet_list",
            Self::NumberedList => "numbered_list",
            Self::ListItem => "list_item",
            Self::Quote => "quote",
            Self::Callout => "callout",
            Self::Toggle => "toggle",
            Self::ColumnList => "column_list",
            Self::Column => "column",
            Self::Task => "task",
            Self::Image => "image",
            Self::Attachment => "attachment",
            Self::Video => "video",
            Self::Divider => "divider",
            Self::Mermaid => "mermaid",
            Self::PlantUml => "plantuml",
            Self::Unknown(s) => s.as_str(),
        }
    }

    /// 是否为标题类型
    pub fn is_heading(&self) -> bool {
        matches!(self, Self::H1 | Self::H2 | Self::H3 | Self::H4 | Self::H5)
    }

    /// 标题级别 (H1=1, H2=2, ..., 非标题返回 0)
    pub fn heading_level(&self) -> u8 {
        match self {
            Self::H1 => 1,
            Self::H2 => 2,
            Self::H3 => 3,
            Self::H4 => 4,
            Self::H5 => 5,
            _ => 0,
        }
    }
}

/// 强类型块节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    /// 块 ID
    pub id: String,
    /// 块类型
    pub block_type: BlockType,
    /// 文本内容（大多数块类型都有）
    pub text: Option<String>,
    /// 原始 content JSON（保留所有字段，用于 round-trip）
    pub content: serde_json::Value,
    /// 子块
    pub children: Vec<Block>,
}

impl Block {
    /// 从 MCP JSON 解析为 Block
    ///
    /// 支持两种 MCP 返回格式:
    /// 1. `describe_block`: `{ id, type, content: { text }, children: [...] }`
    /// 2. `list_block_children`: `{ block_id, block_type, heading1/h2/paragraph: { elements: [...] } }`
    pub fn from_json(value: &serde_json::Value) -> Self {
        // ID: block_id > id
        let id = value
            .get("block_id")
            .and_then(|v| v.as_str())
            .or_else(|| value.get("id").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string();

        // 类型: block_type > type
        let type_str = value
            .get("block_type")
            .and_then(|v| v.as_str())
            .or_else(|| value.get("type").and_then(|v| v.as_str()))
            .unwrap_or("");
        let block_type = BlockType::from_str(type_str);

        // 文本内容 — 多种可能的路径
        // 优先级: content.text > text > heading*.elements[*].text_run.content > paragraph.elements[*].text_run.content
        let text = extract_text_content(value);

        // 保留原始 content
        let content = value.get("content").cloned().unwrap_or_else(|| {
            // 如果没有 content 字段，把整个值作为 content 保存（去掉 children）
            let mut c = value.clone();
            c.as_object_mut().map(|o| o.remove("children"));
            c
        });

        // 子节点: children (describe_block 格式)
        let children = value
            .get("children")
            .and_then(|c| c.as_array())
            .map(|arr| arr.iter().map(Block::from_json).collect())
            .unwrap_or_default();

        Self {
            id,
            block_type,
            text,
            content,
            children,
        }
    }

    /// 递归在块树中查找指定类型的所有块
    pub fn find_by_type(&self, block_type: &BlockType) -> Vec<&Block> {
        let mut result = Vec::new();
        if &self.block_type == block_type {
            result.push(self);
        }
        for child in &self.children {
            result.extend(child.find_by_type(block_type));
        }
        result
    }

    /// 递归查找标题块（按文本匹配）
    pub fn find_heading(&self, heading_text: &str) -> Option<&Block> {
        // 去掉 markdown 标记（如 "## API" -> "API"）
        let clean = heading_text.trim_start_matches('#').trim();

        if self.block_type.is_heading() {
            if let Some(ref text) = self.text {
                if text.trim() == clean {
                    return Some(self);
                }
            }
        }
        for child in &self.children {
            if let Some(found) = child.find_heading(heading_text) {
                return Some(found);
            }
        }
        None
    }

    /// 递归搜索包含指定文本的块（子串匹配，不区分大小写）
    pub fn find_text(&self, query: &str) -> Vec<BlockMatch> {
        let mut result = Vec::new();
        self.find_text_recursive(query, &mut Vec::new(), &mut result);
        result
    }

    fn find_text_recursive(&self, query: &str, path: &mut Vec<String>, out: &mut Vec<BlockMatch>) {
        path.push(self.id.clone());
        if let Some(ref text) = self.text {
            if text.to_lowercase().contains(&query.to_lowercase()) {
                out.push(BlockMatch {
                    id: self.id.clone(),
                    block_type: self.block_type.clone(),
                    text: Some(text.clone()),
                    path: path.clone(),
                });
            }
        }
        for child in &self.children {
            child.find_text_recursive(query, path, out);
        }
        path.pop();
    }
}

/// 文本搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockMatch {
    pub id: String,
    pub block_type: BlockType,
    pub text: Option<String>,
    /// 从根到当前块的 ID 路径
    pub path: Vec<String>,
}

/// 表格视图（从块树投影出来）
#[derive(Debug, Clone)]
pub struct Table {
    /// 表格块本身的 ID
    pub block_id: String,
    /// 表头行（第一行）
    pub headers: Vec<Cell>,
    /// 数据行（不含表头）
    pub rows: Vec<Row>,
}

/// 表格行
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Row {
    /// 行块的 `ID（table_row` block）
    pub block_id: String,
    /// 行在表格中的索引（0-based，不含表头）
    pub index: usize,
    /// 单元格
    pub cells: Vec<Cell>,
}

/// 单元格
#[derive(Debug, Clone)]
pub struct Cell {
    /// 单元格块的 `ID（table_cell` block）
    pub block_id: String,
    /// 文本内容
    pub text: String,
}

/// 从块 JSON 中提取文本内容
///
/// MCP 返回格式多样，需要按优先级尝试多种路径:
/// 1. `content.text` (标准格式)
/// 2. `text` (简化格式)
/// 3. `heading1.elements[*].text_run.content` / `heading2.elements[*].text_run.content` ...
/// 4. `paragraph.elements[*].text_run.content`
/// 5. `quote.elements[*].text_run.content`
/// 6. `task.elements[*].text_run.content`
fn extract_text_content(value: &serde_json::Value) -> Option<String> {
    // 路径 1: content.text > text
    if let Some(text) = value
        .get("content")
        .and_then(|c| c.get("text"))
        .and_then(|t| t.as_str())
    {
        return Some(text.to_string());
    }
    if let Some(text) = value.get("text").and_then(|t| t.as_str()) {
        return Some(text.to_string());
    }

    // 路径 2: heading/paragraph/quote/task 的 elements 结构
    // 检测类型字段名来确定用哪个 key
    const TYPE_KEYS: &[&str] = &[
        "heading1",
        "heading2",
        "heading3",
        "heading4",
        "heading5",
        "paragraph",
        "quote",
        "task",
    ];
    for key in TYPE_KEYS {
        if let Some(content) = value.get(key) {
            if let Some(text) = extract_from_elements(content) {
                return Some(text);
            }
        }
    }

    None
}

/// 从 elements 数组中提取文本: elements[*].`text_run.content`
fn extract_from_elements(content: &serde_json::Value) -> Option<String> {
    let elements = content.get("elements")?.as_array()?;
    let parts: Vec<String> = elements
        .iter()
        .filter_map(|el| {
            el.get("text_run")
                .and_then(|t| t.get("content").and_then(|c| c.as_str()))
        })
        .map(std::string::ToString::to_string)
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_type_from_str() {
        assert_eq!(BlockType::from_str("h1"), BlockType::H1);
        assert_eq!(BlockType::from_str("paragraph"), BlockType::Paragraph);
        assert_eq!(BlockType::from_str(""), BlockType::Paragraph);
        assert_eq!(BlockType::from_str("table"), BlockType::Table);
        assert_eq!(
            BlockType::from_str("custom"),
            BlockType::Unknown("custom".to_string())
        );
    }

    #[test]
    fn test_block_type_heading() {
        assert!(BlockType::H1.is_heading());
        assert!(BlockType::H3.is_heading());
        assert!(!BlockType::Paragraph.is_heading());
        assert_eq!(BlockType::H2.heading_level(), 2);
        assert_eq!(BlockType::Paragraph.heading_level(), 0);
    }

    #[test]
    fn test_block_from_json() {
        let json = serde_json::json!({
            "id": "block_001",
            "type": "h2",
            "content": { "text": "API Reference" },
            "children": [
                {
                    "id": "block_002",
                    "type": "paragraph",
                    "content": { "text": "Some text" },
                    "children": []
                }
            ]
        });

        let block = Block::from_json(&json);
        assert_eq!(block.id, "block_001");
        assert_eq!(block.block_type, BlockType::H2);
        assert_eq!(block.text.as_deref(), Some("API Reference"));
        assert_eq!(block.children.len(), 1);
        assert_eq!(block.children[0].id, "block_002");
    }

    #[test]
    fn test_block_find_heading() {
        let json = serde_json::json!({
            "id": "root",
            "type": "paragraph",
            "content": {},
            "children": [
                {
                    "id": "h2_1",
                    "type": "h2",
                    "content": { "text": "API" },
                    "children": []
                },
                {
                    "id": "h2_2",
                    "type": "h2",
                    "content": { "text": "FAQ" },
                    "children": []
                }
            ]
        });

        let block = Block::from_json(&json);
        let found = block.find_heading("## API");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "h2_1");

        let found2 = block.find_heading("FAQ");
        assert!(found2.is_some());
        assert_eq!(found2.unwrap().id, "h2_2");

        assert!(block.find_heading("Missing").is_none());
    }

    #[test]
    fn test_table_types() {
        let table = Table {
            block_id: "tbl_1".to_string(),
            headers: vec![
                Cell {
                    block_id: "c1".to_string(),
                    text: "Name".to_string(),
                },
                Cell {
                    block_id: "c2".to_string(),
                    text: "Value".to_string(),
                },
            ],
            rows: vec![Row {
                block_id: "r1".to_string(),
                index: 0,
                cells: vec![
                    Cell {
                        block_id: "c3".to_string(),
                        text: "foo".to_string(),
                    },
                    Cell {
                        block_id: "c4".to_string(),
                        text: "bar".to_string(),
                    },
                ],
            }],
        };
        assert_eq!(table.headers.len(), 2);
        assert_eq!(table.rows[0].cells[1].text, "bar");
    }
}
