//! Block Service — 高级块操作封装
//!
//! 将多步 MCP 调用封装为语义化 API：
//! - 表格读写 (`get_table`, `set_cell`, `add_row`, `delete_row`)
//! - 文档操作 (`replace_section`, `insert_after`, append)
//! - 内容转换 (`blocks_to_markdown`, `markdown_to_blocks`)

pub mod converter;
pub mod document;
pub mod reader;
pub mod table;
pub mod types;

pub use types::*;

use crate::mcp::McpCaller;

/// Block Service — 高级块操作入口
pub struct BlockService {
    mcp: Box<dyn McpCaller>,
}

impl BlockService {
    /// 创建 `BlockService`
    pub fn new(mcp: Box<dyn McpCaller>) -> Self {
        Self { mcp }
    }

    /// 获取内部 `McpCaller` 引用
    #[allow(dead_code)]
    pub fn mcp(&self) -> &dyn McpCaller {
        self.mcp.as_ref()
    }
}
