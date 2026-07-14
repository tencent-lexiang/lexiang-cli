# 页面级读取与更新

## 获取当前 DSL

页面 MDX 是服务端 Resource 管理的版本化协议。结构化读取或写入前先发现并读取当前版本：

```bash
lx mcp resource list --format json
lx mcp resource read lexiang://docs/block-mdx/v0
```

如果列表中的 `block-mdx` URI 版本不同，以列表返回值为准。CLI 不内置或复制这份 DSL。

## 读取页面

```bash
# 文本编辑：返回可回写 block-markdown
lx block fetch --entry-id <ENTRY_ID> --render-mode markdown

# 结构化编辑：返回带 data-id 的 MDX
lx block fetch --entry-id <ENTRY_ID> --render-mode mdx

# 只读摘要，不用于回写
lx block fetch --entry-id <ENTRY_ID> --render-mode clean
```

## 整页替换

先校验，再去掉 `dry_run` 执行同一个 payload：

```bash
lx block update -d '{
  "entry_id": "<ENTRY_ID>",
  "command": "replace_content",
  "content_format": "markdown",
  "new_str": "# 新标题\n\n新的正文",
  "dry_run": true
}'
```

## 精确替换、删除或追加文本

`old_str` 必须原样复制自 `fetch --render-mode markdown` 的输出。

```bash
lx block update -d '{
  "entry_id": "<ENTRY_ID>",
  "command": "update_content",
  "content_format": "markdown",
  "content_updates": [
    {"old_str": "旧的精确片段", "new_str": "新的片段", "replace_all_matches": false}
  ],
  "dry_run": true
}'
```

- 删除内容：将 `new_str` 设为空字符串。
- 追加内容：把某段 `old_str` 替换为“原片段 + 新内容”。
- 多处替换：在 `content_updates` 中放入多项。

## 按 ID 删除块

```bash
lx block update -d '{
  "entry_id": "<ENTRY_ID>",
  "command": "delete_blocks",
  "block_ids": ["<BLOCK_ID_1>", "<BLOCK_ID_2>"],
  "dry_run": true
}'
```

若删除范围包含子页面、数据库或智能表格，添加：

```json
{"allow_deleting_content": true}
```

## 替换结构化块

先用 `render_mode=mdx` 获取现有 `data-id`，再提交 MDX block 片段：

```bash
lx block update -d '{
  "entry_id": "<ENTRY_ID>",
  "command": "replace_blocks",
  "content_format": "mdx",
  "block_replacements": [
    {"block_id": "<BLOCK_ID>", "new_str": "<paragraph data-id=\"<BLOCK_ID>\">新内容</paragraph>"}
  ],
  "dry_run": true
}'
```

校验成功后，将 `dry_run` 改为 `false` 或删除该字段，再执行并重新 `fetch` 验证。
