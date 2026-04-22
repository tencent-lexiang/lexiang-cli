//! Document Intermediate Representation (`DocIR`)
//!
//! Canonical AST between MDX and Block JSON.
//! All MDX parsing and Block serialization go through this layer.
//! Aligned with xiaokeai MCP Server's `block_create_block_descendant` schema.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// 文档节点类型 — 对应 MCP `block_type` 枚举值
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum NodeType {
    // === 容器类型（支持 children 子块）===
    Document,
    /// 高亮提示框 — 内容在 children 子块中，content 只有 { color, icon? }
    Callout {
        color: Option<String>,
        icon: Option<String>,
    },
    /// 分栏容器 — children 仅限 Column 类型
    ColumnList,
    /// 分栏列 — `任意子块，width_ratio` 为数字比例
    Column {
        width_ratio: Option<f64>,
    },
    /// 表格容器 — children 仅限 `TableCell` 类型
    Table,
    /// 表格行 — children 仅限 `TableCell` 类型
    TableRow,
    /// 表格单元格 — 任意子块（通常 p）
    TableCell {
        align: Option<String>, // left | center | right
        background_color: Option<String>,
        col_span: Option<u32>,
        row_span: Option<u32>,
        vertical_align: Option<String>, // top | middle | bottom
    },

    // === 叶子类型（不支持 children）===
    #[default]
    Paragraph,
    Heading {
        level: u8,
    }, // h1-h5
    BulletedList,
    NumberedList,
    CodeBlock {
        language: Option<String>,
    },
    Divider,
    Image {
        // 用 file_id 引用
        file_id: Option<String>,
        caption: Option<String>,
        width: Option<u32>,
        height: Option<u32>,
        align: Option<String>,
    },
    /// 折叠块（可展开/收起）
    Toggle,
    /// 任务 — 含名称、完成状态、负责人、截止时间
    Task {
        done: bool,
        name: Option<String>,
    },
    Mermaid {
        content: String,
    },
    PlantUml {
        content: String,
    },
    SmartSheet {
        smartsheet_id: Option<String>,
    },
    Attachment {
        // 用 file_id/session_id 引用
        file_id: Option<String>,
        session_id: Option<String>,
        view_type: Option<String>,
    },
    Video {
        // 用 file_id 引用
        file_id: Option<String>,
        width: Option<u32>,
        height: Option<u32>,
        align: Option<String>,
    },

    // === 内联/辅助节点 ===
    /// 行内文本叶子节点
    Text,
    /// 行内链接包装
    Link {
        href: String,
    },
    /// 块引用（对应 Notion quote block）
    BlockQuote,
    /// Math 公式（Phase 2）
    MathBlock {
        width: Option<f64>,
    },
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Document => write!(f, "Document"),
            Self::Paragraph => write!(f, "p"),
            Self::Heading { level } => write!(f, "h{level}"),
            Self::BulletedList => write!(f, "bulleted_list"),
            Self::NumberedList => write!(f, "numbered_list"),
            Self::CodeBlock { .. } => write!(f, "code"),
            Self::Divider => write!(f, "divider"),
            Self::Callout { .. } => write!(f, "callout"),
            Self::ColumnList => write!(f, "column_list"),
            Self::Column { .. } => write!(f, "column"),
            Self::Table => write!(f, "table"),
            Self::TableRow => write!(f, "table_row"),
            Self::TableCell { .. } => write!(f, "table_cell"),
            Self::Task { .. } => write!(f, "task"),
            Self::Toggle => write!(f, "toggle"),
            Self::Image { .. } => write!(f, "image"),
            Self::Mermaid { .. } => write!(f, "mermaid"),
            Self::PlantUml { .. } => write!(f, "plantuml"),
            Self::SmartSheet { .. } => write!(f, "smartsheet"),
            Self::Attachment { .. } => write!(f, "attachment"),
            Self::Video { .. } => write!(f, "video"),
            Self::Text => write!(f, "text"),
            Self::Link { .. } => write!(f, "link"),
            Self::BlockQuote => write!(f, "block_quote"),
            Self::MathBlock { .. } => write!(f, "math"),
        }
    }
}

/// 行内文本样式（对应 MCP `TextStyle`）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct InlineStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    #[serde(rename = "strikethrough")]
    pub strike_through: bool,
    #[serde(rename = "inline_code")]
    pub inline_code: bool,
    #[serde(rename = "link")]
    pub link: Option<String>,
    #[serde(rename = "textColor")]
    pub text_color: Option<String>,
    #[serde(rename = "backgroundColor")]
    pub background_color: Option<String>,
}

impl InlineStyle {
    pub fn is_plain(&self) -> bool {
        !self.bold
            && !self.italic
            && !self.underline
            && !self.strike_through
            && !self.inline_code
            && self.link.is_none()
            && self.text_color.is_none()
            && self.background_color.is_none()
    }
}

/// 块级样式（对应 MCP `BlockStyle`）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BlockStyle {
    #[serde(rename = "align", skip_serializing_if = "Option::is_none")]
    pub align: Option<String>, // left | center | right
    #[serde(rename = "backgroundColor", skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>, // 代码语言
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrap: Option<bool>, // 自动换行
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BlockAttrs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<String>,
}

/// 文档树节点
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Node {
    /// 块唯一标识符 — UUID v4 字符串（来自 Block JSON 或自动生成）
    #[serde(default)]
    pub id: Option<String>,
    pub node_type: NodeType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// 行内文本样式（仅用于 Text/Link 类型节点）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_style: Option<InlineStyle>,
    #[serde(default)]
    pub attrs: BlockAttrs,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<BlockStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(default)]
    pub children: Vec<Node>,

    // === Task 特有字段（扁平化存储，adapter 输出时提取到 task 对象中）===
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_assignees: Option<Vec<TaskAssignee>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_due_at: Option<TaskDueAt>,

    // === Callout 特有字段 ===
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callout_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callout_icon: Option<String>,

    // === Image 特有字段 ===
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_file_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_caption: Option<String>,

    // === Column 特有字段 ===
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_width_ratio: Option<f64>,

    // === TableCell 特有字段 ===
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell_align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell_bg_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell_col_span: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell_row_span: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell_vertical_align: Option<String>,

    // === Mermaid / PlantUml 特有字段 ===
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagram_content: Option<String>,

    // === 临时 ID（用于构建父子关系） ===
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temp_id: Option<String>,

    // === Image/Video 通用尺寸字段 ===
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

/// 任务负责人
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskAssignee {
    pub staff_id: String,
}

/// 任务截止时间
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskDueAt {
    pub date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
}

// ====== Node 构造器 ======

impl Node {
    pub fn document(children: Vec<Node>) -> Self {
        Self {
            id: None,
            node_type: NodeType::Document,
            text: None,
            attrs: Default::default(),
            style: None,
            href: None,
            children,
            task_name: None,
            task_assignees: None,
            task_due_at: None,
            callout_color: None,
            callout_icon: None,
            inline_style: None,
            image_file_id: None,
            image_caption: None,
            column_width_ratio: None,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: None,
            temp_id: None,
            width: None,
            height: None,
        }
    }

    pub fn text(content: impl Into<String>, style: Option<InlineStyle>) -> Self {
        Self {
            id: None,
            node_type: NodeType::Text,
            text: Some(content.into()),
            attrs: Default::default(),
            inline_style: style,
            style: None,
            href: None,
            children: vec![],
            task_name: None,
            task_assignees: None,
            task_due_at: None,
            callout_color: None,
            callout_icon: None,
            image_file_id: None,
            image_caption: None,
            column_width_ratio: None,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: None,
            temp_id: None,
            width: None,
            height: None,
        }
    }

    pub fn plain_text(text: impl Into<String>) -> Self {
        Self::text(text, None)
    }

    pub fn bold(text: impl Into<String>) -> Self {
        Self::text(
            text,
            Some(InlineStyle {
                bold: true,
                ..Default::default()
            }),
        )
    }

    pub fn paragraph(inlines: Vec<Node>) -> Self {
        Self {
            id: None,
            node_type: NodeType::Paragraph,
            text: None,
            attrs: Default::default(),
            style: None,
            href: None,
            children: inlines,
            task_name: None,
            task_assignees: None,
            task_due_at: None,
            callout_color: None,
            callout_icon: None,
            inline_style: None,
            image_file_id: None,
            image_caption: None,
            column_width_ratio: None,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: None,
            temp_id: None,
            width: None,
            height: None,
        }
    }

    pub fn heading(level: u8, inlines: Vec<Node>) -> Self {
        assert!((1..=6).contains(&level));
        Self {
            id: None,
            node_type: NodeType::Heading { level },
            text: None,
            attrs: Default::default(),
            style: None,
            href: None,
            children: inlines,
            task_name: None,
            task_assignees: None,
            task_due_at: None,
            callout_color: None,
            callout_icon: None,
            inline_style: None,
            image_file_id: None,
            image_caption: None,
            column_width_ratio: None,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: None,
            temp_id: None,
            width: None,
            height: None,
        }
    }

    /// Callout — 容器类型，内容在 children 中
    pub fn callout(color: Option<&str>, icon: Option<&str>, children: Vec<Node>) -> Self {
        Self {
            id: None,
            node_type: NodeType::Callout {
                color: color.map(String::from),
                icon: icon.map(String::from),
            },
            text: None,
            attrs: Default::default(),
            style: None,
            href: None,
            children,
            task_name: None,
            task_assignees: None,
            task_due_at: None,
            callout_color: color.map(String::from),
            callout_icon: icon.map(String::from),
            inline_style: None,
            image_file_id: None,
            image_caption: None,
            column_width_ratio: None,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: None,
            temp_id: None,
            width: None,
            height: None,
        }
    }

    pub fn quote(inlines: Vec<Node>) -> Self {
        Self {
            id: None,
            node_type: NodeType::BlockQuote,
            text: None,
            attrs: Default::default(),
            style: None,
            href: None,
            children: inlines,
            task_name: None,
            task_assignees: None,
            task_due_at: None,
            callout_color: None,
            callout_icon: None,
            inline_style: None,
            image_file_id: None,
            image_caption: None,
            column_width_ratio: None,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: None,
            temp_id: None,
            width: None,
            height: None,
        }
    }

    pub fn bullet_item(inlines: Vec<Node>) -> Self {
        Self {
            id: None,
            node_type: NodeType::BulletedList,
            text: None,
            attrs: Default::default(),
            style: None,
            href: None,
            children: inlines,
            task_name: None,
            task_assignees: None,
            task_due_at: None,
            callout_color: None,
            callout_icon: None,
            inline_style: None,
            image_file_id: None,
            image_caption: None,
            column_width_ratio: None,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: None,
            temp_id: None,
            width: None,
            height: None,
        }
    }

    pub fn numbered_item(inlines: Vec<Node>) -> Self {
        Self {
            id: None,
            node_type: NodeType::NumberedList,
            text: None,
            attrs: Default::default(),
            style: None,
            href: None,
            children: inlines,
            task_name: None,
            task_assignees: None,
            task_due_at: None,
            callout_color: None,
            callout_icon: None,
            inline_style: None,
            image_file_id: None,
            image_caption: None,
            column_width_ratio: None,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: None,
            temp_id: None,
            width: None,
            height: None,
        }
    }

    /// Task — 支持名称和完成状态
    pub fn task(done: bool, name: impl Into<String>) -> Self {
        Self {
            id: None,
            node_type: NodeType::Task { done, name: None },
            text: None,
            attrs: Default::default(),
            style: None,
            href: None,
            children: vec![],
            task_name: Some(name.into()),
            task_assignees: None,
            task_due_at: None,
            callout_color: None,
            callout_icon: None,
            inline_style: None,
            image_file_id: None,
            image_caption: None,
            column_width_ratio: None,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: None,
            temp_id: None,
            width: None,
            height: None,
        }
    }

    pub fn code_block(language: Option<&str>, code: &str) -> Self {
        Self {
            id: None,
            node_type: NodeType::CodeBlock {
                language: language.map(String::from),
            },
            text: Some(code.to_string()),
            attrs: Default::default(),
            style: None,
            href: None,
            children: vec![],
            task_name: None,
            task_assignees: None,
            task_due_at: None,
            callout_color: None,
            callout_icon: None,
            inline_style: None,
            image_file_id: None,
            image_caption: None,
            column_width_ratio: None,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: None,
            temp_id: None,
            width: None,
            height: None,
        }
    }

    pub fn divider() -> Self {
        Self {
            id: None,
            node_type: NodeType::Divider,
            text: None,
            attrs: Default::default(),
            style: None,
            href: None,
            children: vec![],
            task_name: None,
            task_assignees: None,
            task_due_at: None,
            callout_color: None,
            callout_icon: None,
            inline_style: None,
            image_file_id: None,
            image_caption: None,
            column_width_ratio: None,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: None,
            temp_id: None,
            width: None,
            height: None,
        }
    }

    pub fn image(file_id: Option<&str>, caption: Option<&str>) -> Self {
        Self {
            id: None,
            node_type: NodeType::Image {
                file_id: file_id.map(String::from),
                caption: caption.map(String::from),
                width: None,
                height: None,
                align: None,
            },
            text: caption.map(String::from),
            attrs: Default::default(),
            style: None,
            href: None,
            children: vec![],
            task_name: None,
            task_assignees: None,
            task_due_at: None,
            callout_color: None,
            callout_icon: None,
            inline_style: None,
            image_file_id: file_id.map(String::from),
            image_caption: caption.map(String::from),
            column_width_ratio: None,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: None,
            temp_id: None,
            width: None,
            height: None,
        }
    }

    pub fn table(rows: Vec<Node>) -> Self {
        Self {
            id: None,
            node_type: NodeType::Table,
            text: None,
            attrs: Default::default(),
            style: None,
            href: None,
            children: rows,
            task_name: None,
            task_assignees: None,
            task_due_at: None,
            callout_color: None,
            callout_icon: None,
            inline_style: None,
            image_file_id: None,
            image_caption: None,
            column_width_ratio: None,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: None,
            temp_id: None,
            width: None,
            height: None,
        }
    }

    pub fn table_row(cells: Vec<Node>) -> Self {
        Self {
            id: None,
            node_type: NodeType::TableRow,
            text: None,
            attrs: Default::default(),
            style: None,
            href: None,
            children: cells,
            task_name: None,
            task_assignees: None,
            task_due_at: None,
            callout_color: None,
            callout_icon: None,
            inline_style: None,
            image_file_id: None,
            image_caption: None,
            column_width_ratio: None,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: None,
            temp_id: None,
            width: None,
            height: None,
        }
    }

    pub fn table_cell(inlines: Vec<Node>) -> Self {
        Self {
            id: None,
            node_type: NodeType::TableCell {
                align: None,
                background_color: None,
                col_span: None,
                row_span: None,
                vertical_align: None,
            },
            text: None,
            attrs: Default::default(),
            style: None,
            href: None,
            children: inlines,
            task_name: None,
            task_assignees: None,
            task_due_at: None,
            callout_color: None,
            callout_icon: None,
            inline_style: None,
            image_file_id: None,
            image_caption: None,
            column_width_ratio: None,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: None,
            temp_id: None,
            width: None,
            height: None,
        }
    }

    pub fn column_list(columns: Vec<Node>) -> Self {
        Self {
            id: None,
            node_type: NodeType::ColumnList,
            text: None,
            attrs: Default::default(),
            style: None,
            href: None,
            children: columns,
            task_name: None,
            task_assignees: None,
            task_due_at: None,
            callout_color: None,
            callout_icon: None,
            inline_style: None,
            image_file_id: None,
            image_caption: None,
            column_width_ratio: None,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: None,
            temp_id: None,
            width: None,
            height: None,
        }
    }

    pub fn column(width_ratio: Option<f64>, children: Vec<Node>) -> Self {
        Self {
            id: None,
            node_type: NodeType::Column { width_ratio },
            text: None,
            attrs: Default::default(),
            style: None,
            href: None,
            children,
            task_name: None,
            task_assignees: None,
            task_due_at: None,
            callout_color: None,
            callout_icon: None,
            inline_style: None,
            image_file_id: None,
            image_caption: None,
            column_width_ratio: width_ratio,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: None,
            temp_id: None,
            width: None,
            height: None,
        }
    }

    pub fn link(href: &str, inlines: Vec<Node>) -> Self {
        Self {
            id: None,
            node_type: NodeType::Link {
                href: href.to_string(),
            },
            text: None,
            attrs: Default::default(),
            style: None,
            href: Some(href.to_string()),
            children: inlines,
            task_name: None,
            task_assignees: None,
            task_due_at: None,
            callout_color: None,
            callout_icon: None,
            inline_style: None,
            image_file_id: None,
            image_caption: None,
            column_width_ratio: None,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: None,
            temp_id: None,
            width: None,
            height: None,
        }
    }

    pub fn toggle(inlines: Vec<Node>) -> Self {
        Self {
            id: None,
            node_type: NodeType::Toggle,
            text: None,
            attrs: Default::default(),
            style: None,
            href: None,
            children: inlines,
            task_name: None,
            task_assignees: None,
            task_due_at: None,
            callout_color: None,
            callout_icon: None,
            inline_style: None,
            image_file_id: None,
            image_caption: None,
            column_width_ratio: None,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: None,
            temp_id: None,
            width: None,
            height: None,
        }
    }

    pub fn mermaid(code: &str) -> Self {
        Self {
            id: None,
            node_type: NodeType::Mermaid {
                content: code.to_string(),
            },
            text: None,
            attrs: Default::default(),
            style: None,
            href: None,
            children: vec![],
            task_name: None,
            task_assignees: None,
            task_due_at: None,
            callout_color: None,
            callout_icon: None,
            inline_style: None,
            image_file_id: None,
            image_caption: None,
            column_width_ratio: None,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: Some(code.to_string()),
            temp_id: None,
            width: None,
            height: None,
        }
    }

    pub fn plantuml(code: &str) -> Self {
        Self {
            id: None,
            node_type: NodeType::PlantUml {
                content: code.to_string(),
            },
            text: None,
            attrs: Default::default(),
            style: None,
            href: None,
            children: vec![],
            task_name: None,
            task_assignees: None,
            task_due_at: None,
            callout_color: None,
            callout_icon: None,
            inline_style: None,
            image_file_id: None,
            image_caption: None,
            column_width_ratio: None,
            cell_align: None,
            cell_bg_color: None,
            cell_col_span: None,
            cell_row_span: None,
            cell_vertical_align: None,
            diagram_content: Some(code.to_string()),
            temp_id: None,
            width: None,
            height: None,
        }
    }

    /// Set block ID (chainable builder-style)
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn plain_content(&self) -> String {
        if let Some(ref t) = self.text {
            return t.clone();
        }
        self.children
            .iter()
            .map(Node::plain_content)
            .collect::<Vec<_>>()
            .join("")
    }

    pub fn find_child(&self, target: &NodeType) -> Option<&Node> {
        self.children.iter().find(|n| &n.node_type == target)
    }

    pub fn find_all(&self, target: &NodeType) -> Vec<&Node> {
        let mut result = Vec::new();
        self.collect_by_type(target, &mut result);
        result
    }

    fn collect_by_type<'a>(&'a self, target: &NodeType, result: &mut Vec<&'a Node>) {
        if std::mem::discriminant(&self.node_type) == std::mem::discriminant(target) {
            result.push(self);
        }
        for child in &self.children {
            child.collect_by_type(target, result);
        }
    }

    /// 生成临时 ID（用于构建父子关系）
    pub fn next_temp_id(counter: &mut u64) -> String {
        *counter += 1;
        format!("temp_{:08x}", counter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_constructors() {
        let p = Node::paragraph(vec![Node::plain_text("hello")]);
        assert_eq!(p.node_type, NodeType::Paragraph);
        assert_eq!(p.children[0].plain_content(), "hello");

        let h2 = Node::heading(2, vec![Node::plain_text("title")]);
        assert_eq!(h2.node_type, NodeType::Heading { level: 2 });

        let t = Node::task(true, "Buy milk");
        assert_eq!(
            t.node_type,
            NodeType::Task {
                done: true,
                name: None
            }
        );
        assert_eq!(t.task_name.as_deref(), Some("Buy milk"));

        let cb = Node::code_block(Some("rust"), "fn main() {}");
        assert_eq!(cb.text.as_deref(), Some("fn main() {}"));

        let d = Node::divider();
        assert_eq!(d.node_type, NodeType::Divider);

        let img = Node::image(Some("fid_123"), Some("alt"));
        assert_eq!(
            img.node_type,
            NodeType::Image {
                file_id: Some("fid_123".into()),
                caption: Some("alt".into()),
                width: None,
                height: None,
                align: None
            }
        );
        assert_eq!(img.image_file_id.as_deref(), Some("fid_123"));
    }

    #[test]
    fn test_inline_style() {
        let plain = InlineStyle::default();
        assert!(plain.is_plain());
        let s = InlineStyle {
            bold: true,
            ..Default::default()
        };
        assert!(!s.is_plain());
    }

    #[test]
    fn test_callout_is_container() {
        let co = Node::callout(
            Some("red"),
            Some("\u{1f6a7}"),
            vec![
                Node::heading(1, vec![Node::bold("Note")]),
                Node::paragraph(vec![Node::plain_text("Content")]),
            ],
        );
        // Callout should have children (container type)
        assert!(!co.children.is_empty(), "callout should be container");
        assert_eq!(co.callout_color.as_deref(), Some("red"));
        assert_eq!(co.callout_icon.as_deref(), Some("\u{1f6a7}"));
    }

    #[test]
    fn test_column_width_ratio_is_number() {
        let col = Node::column(Some(0.5), vec![Node::plain_text("col")]);
        assert_eq!(col.column_width_ratio, Some(0.5));
    }

    #[test]
    fn test_toggle() {
        let t = Node::toggle(vec![Node::plain_text("Click to expand")]);
        assert_eq!(t.node_type, NodeType::Toggle);
    }

    #[test]
    fn test_mermaid() {
        let m = Node::mermaid("graph LR\nA --> B");
        assert_eq!(
            m.node_type,
            NodeType::Mermaid {
                content: "graph LR\nA --> B".into()
            }
        );
        assert_eq!(m.diagram_content.as_deref(), Some("graph LR\nA --> B"));
    }

    #[test]
    fn test_serde_roundtrip_callout() {
        let node = Node::callout(
            Some("\u{1f4dd}"),
            Some("blue"),
            vec![
                Node::heading(1, vec![Node::bold("Note")]),
                Node::paragraph(vec![Node::plain_text("Content")]),
            ],
        );
        let json = serde_json::to_string(&node).unwrap();
        let back: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node, back);
    }

    #[test]
    fn test_table_and_columns() {
        let cell = Node::table_cell(vec![Node::plain_text("data")]);
        let row = Node::table_row(vec![cell.clone()]);
        let tbl = Node::table(vec![row]);
        assert_eq!(tbl.node_type, NodeType::Table);
        let col = Node::column(
            Some(0.5),
            vec![Node::paragraph(vec![Node::plain_text("c")])],
        );
        assert_eq!(col.column_width_ratio, Some(0.5));
        let cl = Node::column_list(vec![col]);
        assert_eq!(cl.node_type, NodeType::ColumnList);
    }

    #[test]
    fn test_find_all() {
        let doc = Node::document(vec![
            Node::paragraph(vec![Node::plain_text("p1")]),
            Node::paragraph(vec![Node::plain_text("p2")]),
        ]);
        assert_eq!(doc.find_all(&NodeType::Paragraph).len(), 2);
    }
}
