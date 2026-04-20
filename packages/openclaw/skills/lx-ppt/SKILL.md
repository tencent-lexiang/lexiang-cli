---
name: lx-ppt
description: |
  乐享 AI PPT 生成与编辑。基于服务端 AI 能力，从文字描述直接生成专业 PPT，无需本地模板或 python-pptx。
  支持生成、修改页面内容、增删页面、调整顺序。
  触发词：PPT、幻灯片、演示文稿、slide、deck、制作PPT、生成PPT
metadata:
  {"openclaw": {"requires": {"bins": ["lx"]}, "always": false}}
---

# AI PPT 生成与编辑

> **前置条件：** 需要 `lx` CLI 已配置并登录。

---

## 使用场景

### 场景一：从零生成 PPT

**触发条件：**

- 用户说"做个 PPT"/"生成幻灯片"/"制作演示文稿"

**使用工具：** `lx-ppt-generate-ppt` / `lx-ppt-get-ppt-task`

**SOP：**

1. 调用 `lx-ppt-generate-ppt { "planning": "10页，主题：Q2业绩汇报", "context": "Q2 营收 1.5 亿..." }`
2. 获取 task_id 后，轮询 `lx-ppt-get-ppt-task { "id": "task_xxx" }`
3. 等待 `status` 变为 `completed`，获取 `title` 和 `preview_url`

**特殊情况处理：**

- 有深度研究报告 → 传入 `deep_research_report_url` 提升生成质量
- 生成超时 → 继续轮询或提示用户稍后查看

---

### 场景二：修改 PPT 页面内容

**触发条件：**

- 用户说"修改第 X 页"/"把标题改成 XX"

**使用工具：** `lx-ppt-modify-ppt-pages`

**SOP：**

1. 使用 PPT 的 `title` 标识（从 `get-ppt-task` 获取）
2. 调用 `lx-ppt-modify-ppt-pages { "title": "Q2业绩汇报", "pages": [{"page_index": 3, "modification": "数据图表换成柱状图"}] }`

**特殊情况处理：**

- 页面索引从 1 开始，不是 0
- `modification` 用自然语言描述即可

---

### 场景三：添加新页面

**触发条件：**

- 用户说"加一页"/"插入新页面"

**使用工具：** `lx-ppt-add-ppt-pages`

**SOP：**

1. 调用 `lx-ppt-add-ppt-pages { "title": "Q2业绩汇报", "pages": [{"insert_after": 5, "title": "风险分析", "key_points": "...", "slide_type": "content"}] }`

**特殊情况处理：**

- `slide_type` 仅支持 `cover`（封面）、`content`（内容页）、`ending`（结束页）

---

### 场景四：删除页面

**触发条件：**

- 用户说"删掉第 X 页"

**使用工具：** `lx-ppt-delete-ppt-pages`

**SOP：**

1. 调用 `lx-ppt-delete-ppt-pages { "title": "Q2业绩汇报", "page_indexes": [2] }`

---

### 场景五：调整页面顺序

**触发条件：**

- 用户说"调整顺序"/"把第 X 页移到前面"

**使用工具：** `lx-ppt-reorder-ppt-pages`

**SOP：**

1. 调用 `lx-ppt-reorder-ppt-pages { "title": "Q2业绩汇报", "new_order": [1, 3, 4, 2] }`

---

## 工具速查

| 工具名 | 用途 |
|--------|------|
| `lx-ppt-generate-ppt` | 生成 PPT |
| `lx-ppt-get-ppt-task` | 查询生成任务状态（轮询） |
| `lx-ppt-modify-ppt-pages` | 修改页面内容 |
| `lx-ppt-add-ppt-pages` | 添加新页面 |
| `lx-ppt-delete-ppt-pages` | 删除页面 |
| `lx-ppt-reorder-ppt-pages` | 调整页面顺序 |

---

## 典型组合流程

### 从零生成 PPT

```json
// 1. 生成
lx-ppt-generate-ppt: {
  "planning": "10页，主题：Q2业绩汇报，风格：商务简约",
  "context": "Q2 营收 1.5 亿..."
}

// 2. 轮询任务状态
lx-ppt-get-ppt-task: { "id": "task_xxx" }
// → status="completed" 后拿到 title + preview_url

// 3. 根据反馈微调
lx-ppt-modify-ppt-pages: {
  "title": "Q2业绩汇报",
  "pages": [{"page_index": 3, "modification": "数据图表换成柱状图"}]
}
```

### 在已有 PPT 上增删调整

```json
// 添加新页面
lx-ppt-add-ppt-pages: {
  "title": "Q2业绩汇报",
  "pages": [{"insert_after": 5, "title": "风险分析", "key_points": "...", "slide_type": "content"}]
}

// 删除第 2 页
lx-ppt-delete-ppt-pages: { "title": "Q2业绩汇报", "page_indexes": [2] }

// 调整顺序
lx-ppt-reorder-ppt-pages: { "title": "Q2业绩汇报", "new_order": [1, 3, 4, 2] }
```
