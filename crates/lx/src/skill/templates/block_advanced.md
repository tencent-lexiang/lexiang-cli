## 高级 Block 命令

> **推荐优先使用高级命令**，原子命令适合精细控制单个块。
> 高级命令封装多步 MCP 调用，自动处理块树遍历、内容转换等复杂逻辑。

### 表格操作

```bash
# 读取表格结构
lx block table-get --block-id <TABLE_ID> --format table|json|csv|markdown

# 修改单元格 (row/col 从0开始，不含表头)
lx block table-set --block-id <TABLE_ID> --row 1 --col 2 --text "新值"

# 追加一行
lx block table-add-row --block-id <TABLE_ID> --values "值1,值2,值3"

# 删除一行
lx block table-del-row --block-id <TABLE_ID> --row 1
```

### 文档编辑

```bash
# 替换标题下的段落内容（保留标题，替换正文）
lx block replace-section --block-id <ROOT_ID> --heading "## API" \
  --content "新的段落内容" --file ./new-section.md

# 在指定块后插入内容
lx block insert-after --block-id <ID> --content "插入的 markdown"

# 在父块末尾追加内容
lx block append --block-id <PARENT_ID> --file ./append.md
```

### 内容导入导出

```bash
# 导出为 markdown
lx block export --block-id <ROOT_ID> --format markdown|json

# 显示块树结构
lx block tree --block-id <ROOT_ID> [--recursive]

# 导入 markdown 文件（自动分批）
lx block import --block-id <PARENT_ID> --file ./doc.md --chunk-size 20
```

### 完整工作流示例

#### 修改表格单元格

```bash
# Step 1: 先查看表格当前状态
lx block table-get --block-id tbl_xxx --format table

# Step 2: 定位要修改的行和列后，直接修改
lx block table-set --block-id tbl_xxx --row 2 --col 1 --text "修正后的值"

# Step 3: 验证结果
lx block table-get --block-id tbl_xxx --format json
```

#### 替换文档中的某个章节

```bash
# Step 1: 查看文档树结构，找到目标章节
lx block tree --block-id root_xxx --recursive

# Step 2: 用高级命令一键替换（内部自动处理删除旧内容+转换+插入）
lx block replace-section --block-id root_xxx --heading "## API 参考" \
  --file ./updated-api.md
```

### 注意事项

- 高级命令与原子命令共存于 `lx block` 命名空间
- 高级命令（table-get、replace-section 等）优先匹配
- 未匹配到高级命令时，自动回退到动态生成的原子命令
