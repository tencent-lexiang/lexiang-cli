//! 表格操作：读取、修改单元格、增删行

use super::types::{Cell, Row, Table};
use super::BlockService;
use anyhow::{bail, Result};

impl BlockService {
    /// 获取表格的结构化视图
    ///
    /// 从 table block 的子块树中解析出 Table 结构。
    /// 第一行视为表头，其余为数据行。
    pub async fn get_table(&self, table_block_id: &str) -> Result<Table> {
        let tree = self.get_tree(table_block_id, true).await?;

        // tree.children 应该是 table_row 块
        let rows = &tree.children;

        if rows.is_empty() {
            return Ok(Table {
                block_id: table_block_id.to_string(),
                headers: Vec::new(),
                rows: Vec::new(),
            });
        }

        // 解析表头（第一行）
        let header_row = &rows[0];
        let headers: Vec<Cell> = header_row
            .children
            .iter()
            .map(|cell| Cell {
                block_id: cell.id.clone(),
                text: cell.text.as_deref().unwrap_or("").to_string(),
            })
            .collect();

        // 解析数据行
        let data_rows: Vec<Row> = rows
            .iter()
            .skip(1)
            .enumerate()
            .map(|(i, row)| Row {
                block_id: row.id.clone(),
                index: i,
                cells: row
                    .children
                    .iter()
                    .map(|cell| Cell {
                        block_id: cell.id.clone(),
                        text: cell.text.as_deref().unwrap_or("").to_string(),
                    })
                    .collect(),
            })
            .collect();

        Ok(Table {
            block_id: table_block_id.to_string(),
            headers,
            rows: data_rows,
        })
    }

    /// 修改单元格内容
    ///
    /// row/col 都是 0-based 索引（不含表头）。
    pub async fn set_cell(
        &self,
        table_block_id: &str,
        row: usize,
        col: usize,
        text: &str,
    ) -> Result<()> {
        let table = self.get_table(table_block_id).await?;

        if row >= table.rows.len() {
            bail!(
                "Row index {} out of range (table has {} data rows)",
                row,
                table.rows.len()
            );
        }

        let target_row = &table.rows[row];
        if col >= target_row.cells.len() {
            bail!(
                "Column index {} out of range (row has {} cells)",
                col,
                target_row.cells.len()
            );
        }

        let cell_block_id = &target_row.cells[col].block_id;

        self.mcp
            .call_tool(
                "block_update_block",
                serde_json::json!({
                    "block_id": cell_block_id,
                    "content": { "text": text },
                }),
            )
            .await?;

        Ok(())
    }

    /// 批量修改单元格（单次 MCP 调用）
    ///
    /// updates: Vec<(row, col, text)>，row/col 均为 0-based（不含表头）。
    #[allow(dead_code)]
    pub async fn set_cells(
        &self,
        table_block_id: &str,
        updates: &[(usize, usize, String)],
    ) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }

        let table = self.get_table(table_block_id).await?;

        let mut blocks_update = Vec::new();
        for (row, col, text) in updates {
            if *row >= table.rows.len() {
                bail!("Row index {} out of range", row);
            }
            let target_row = &table.rows[*row];
            if *col >= target_row.cells.len() {
                bail!("Column index {} out of range", col);
            }

            blocks_update.push(serde_json::json!({
                "block_id": target_row.cells[*col].block_id,
                "content": { "text": text },
            }));
        }

        self.mcp
            .call_tool(
                "block_update_blocks",
                serde_json::json!({ "blocks": blocks_update }),
            )
            .await?;

        Ok(())
    }

    /// 追加一行到表格末尾
    ///
    /// values 长度应该与表格列数一致。
    pub async fn add_row(&self, table_block_id: &str, values: &[&str]) -> Result<()> {
        // 构造 table_row > table_cell 的块结构
        let cells: Vec<serde_json::Value> = values
            .iter()
            .map(|text| {
                serde_json::json!({
                    "type": "table_cell",
                    "content": { "text": *text },
                })
            })
            .collect();

        let descendant = serde_json::json!({
            "children": [{
                "type": "table_row",
                "children": cells,
            }],
        });

        self.mcp
            .call_tool(
                "block_create_block_descendant",
                serde_json::json!({
                    "block_id": table_block_id,
                    "descendant": descendant,
                }),
            )
            .await?;

        Ok(())
    }

    /// 删除一行
    ///
    /// `row_index`: 0-based（不含表头）。
    pub async fn delete_row(&self, table_block_id: &str, row_index: usize) -> Result<()> {
        let table = self.get_table(table_block_id).await?;

        if row_index >= table.rows.len() {
            bail!(
                "Row index {} out of range (table has {} data rows)",
                row_index,
                table.rows.len()
            );
        }

        let row_block_id = &table.rows[row_index].block_id;

        self.mcp
            .call_tool(
                "block_delete_block",
                serde_json::json!({ "block_id": row_block_id }),
            )
            .await?;

        Ok(())
    }
}
