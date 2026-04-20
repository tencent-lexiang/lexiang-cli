---
name: lx-block
description: |
  乐享文档块编辑。当用户需要对知识库页面进行结构化编辑（增删改查块、表格操作、章节替换、内容导入导出）时使用。
  触发词：block、编辑文档、修改内容、表格、块、插入、追加、替换章节
metadata:
  {"openclaw": {"requires": {"bins": ["lx"]}, "always": false}}
---

# 文档块编辑

> **前置条件：** 需要 `lx` CLI 已配置并登录。

---

## 使用场景

### 场景一：表格操作

**触发条件：**

- 用户说"改表格"/"修改表格单元格"/"表格加一行"

**使用工具：** `lx-block-table-get` / `lx-block-table-set` / `lx-block-table-add-row` / `lx-block-table-del-row`

**SOP：**

1. 获取表格 block ID（通过 `lx-entry-describe-ai-parse-content` 或浏览）
2. 调用对应工具：
   - 读取表格 → `lx-block-table-get { "block_id": "tbl_xxx" }`
   - 修改单元格 → `lx-block-table-set { "block_id": "tbl_xxx", "row": 2, "col": 1, "text": "新值" }`
   - 添加行 → `lx-block-table-add-row { "block_id": "tbl_xxx" }`
   - 删除行 → `lx-block-table-del-row { "block_id": "tbl_xxx", "row": 3 }`

**特殊情况处理：**

- 表格结构复杂 → 先用 `lx-block-tree` 查看完整结构
- 批量修改 → 多次调用 `lx-block-table-set`

---

### 场景二：章节替换

**触发条件：**

- 用户说"替换某个章节"/"把 XX 章节换成 YY 内容"

**使用工具：** `lx-block-replace-section`

**SOP：**

1. 获取页面 entry_id 和 root block ID
2. 确认目标章节标题
3. 调用 `lx-block-replace-section { "block_id": "root_xxx", "heading": "## 目标标题", "content": "新内容" }`

**特殊情况处理：**

- 章节标题不唯一 → 先用 `lx-block-tree` 确认具体位置
- 内容较长 → 分多次调用或考虑其他方式

---

### 场景三：内容插入与追加

**触发条件：**

- 用户说"在 XX 后面插入内容"/"在文档末尾追加"

**使用工具：** `lx-block-insert-after` / `lx-block-append`

**SOP：**

1. 确定插入位置的 block ID
2. 调用对应工具：
   - 指定位置插入 → `lx-block-insert-after { "block_id": "blk_xxx", "content": "内容" }`
   - 末尾追加 → `lx-block-append { "block_id": "page_xxx", "content": "内容" }`

---

### 场景四：批量导入 Markdown

**触发条件：**

- 用户说"导入 markdown 到文档"/"把文件内容导入页面"

**使用工具：** `lx-block-import`

**SOP：**

1. 确认目标页面 block ID
2. 调用 `lx-block-import { "block_id": "page_xxx", "file_path": "./doc.md", "chunk_size": 20 }`

**特殊情况处理：**

- 大文档导入 → 设置 `chunk_size: 20` 自动分批
- 目标页面已有内容 → 考虑使用 `lx-block-replace-section`

---

### 场景五：精细块操作

**触发条件：**

- 高级命令无法满足的精细控制需求

**使用工具：** `lx-block-describe-block` / `lx-block-create-block-descendant` / `lx-block-update-block` / `lx-block-delete-block`

**SOP：**

1. 先用 `lx-block-tree` 或 `lx-block-list-block-children` 获取块结构
2. 根据需求选择工具操作

---

## 工具速查

| 工具名 | 用途 |
|--------|------|
| `lx-block-table-get` | 读取表格 |
| `lx-block-table-set` | 修改单元格 |
| `lx-block-table-add-row` | 添加表格行 |
| `lx-block-table-del-row` | 删除表格行 |
| `lx-block-replace-section` | 按标题替换章节 |
| `lx-block-insert-after` | 在指定块后插入 |
| `lx-block-append` | 追加到末尾 |
| `lx-block-import` | 导入 markdown |
| `lx-block-export` | 导出内容 |
| `lx-block-tree` | 查看块树结构 |

---

## 典型组合流程

### 修改表格单元格

```json
// 1. 读取表格
lx-block-table-get: { "block_id": "tbl_xxx", "format": "table" }

// 2. 修改单元格
lx-block-table-set: { "block_id": "tbl_xxx", "row": 2, "col": 1, "text": "修正值" }
```

### 替换文档章节

```json
// 1. 查看文档树
lx-block-tree: { "block_id": "root_xxx", "recursive": true }

// 2. 替换章节
lx-block-replace-section: { "block_id": "root_xxx", "heading": "## API 参考", "file": "./updated-api.md" }
```
