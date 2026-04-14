//! 文档级操作：替换段落、插入、追加

use super::BlockService;
use anyhow::Result;

impl BlockService {
    /// 替换标题下的段落内容
    ///
    /// 流程:
    /// 1. `get_tree(root_id)` 获取完整块树
    /// 2. find heading 定位标题块
    /// 3. `collect_section` 收集段落范围
    /// 4. delete old blocks
    /// 5. convert markdown → blocks
    /// 6. insert new blocks after heading
    pub async fn replace_section(
        &self,
        root_id: &str,
        heading: &str,
        new_content_markdown: &str,
    ) -> Result<()> {
        let (heading_block, section_blocks) = self.collect_section(root_id, heading).await?;

        // 删除旧段落块
        if !section_blocks.is_empty() {
            let children_ids: Vec<String> = section_blocks.iter().map(|b| b.id.clone()).collect();

            self.mcp
                .call_tool(
                    "block_delete_block_children",
                    serde_json::json!({
                        "block_id": root_id,
                        "children_ids": children_ids,
                    }),
                )
                .await?;
        }

        // 转换新内容
        let descendant = self.markdown_to_blocks(new_content_markdown).await?;

        // 在标题块下插入新内容
        self.mcp
            .call_tool(
                "block_create_block_descendant",
                serde_json::json!({
                    "block_id": heading_block.id,
                    "descendant": descendant,
                }),
            )
            .await?;

        Ok(())
    }

    /// 在指定块之后插入内容
    pub async fn insert_after(&self, block_id: &str, markdown: &str) -> Result<()> {
        let descendant = self.markdown_to_blocks(markdown).await?;

        self.mcp
            .call_tool(
                "block_create_block_descendant",
                serde_json::json!({
                    "block_id": block_id,
                    "descendant": descendant,
                }),
            )
            .await?;

        Ok(())
    }

    /// 在父块末尾追加内容
    pub async fn append(&self, parent_id: &str, markdown: &str) -> Result<()> {
        let descendant = self.markdown_to_blocks(markdown).await?;

        self.mcp
            .call_tool(
                "block_create_block_descendant",
                serde_json::json!({
                    "block_id": parent_id,
                    "descendant": descendant,
                }),
            )
            .await?;

        Ok(())
    }
}
