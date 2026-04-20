---
name: lx-entry
description: |
  乐享知识库条目管理。当用户需要操作知识条目（创建、查看、编辑、删除页面/文件夹），导入内容，管理文件，或处理草稿时使用。
  触发词：页面、文档、条目、文件夹、创建文档、导入、上传文件、草稿、版本
metadata:
  {"openclaw": {"requires": {"bins": ["lx"]}, "always": false}}
---

# 条目管理

> **前置条件：** 需要 `lx` CLI 已配置并登录。

---

## 使用场景

### 场景一：创建页面/文件夹

**触发条件：**

- 用户说"创建页面"/"新建文件夹"/"在 XX 下创建文档"

**使用工具：** `lx-entry-create-entry`

**SOP：**

1. 获取父条目 ID（`parent_entry_id`）
   - 知识库根目录：通过 `lx-space-describe-space` 获取 `root_entry_id`
   - 已有文件夹：通过 `lx-entry-list-children` 获取
2. 调用 `lx-entry-create-entry { "parent_entry_id": "parent_xxx", "name": "新页面", "entry_type": "page" }`

**特殊情况处理：**

- 不知道父条目 ID → 先用 `lx-space-describe-space` 或 `lx-entry-list-children` 获取
- 重名 → 系统会自动处理或报错，根据错误提示调整

---

### 场景二：查看文档内容

**触发条件：**

- 用户说"查看 XX 文档"/"读取页面内容"

**使用工具：** `lx-entry-describe-ai-parse-content`

**SOP：**

1. 获取 entry_id（通过搜索或浏览）
2. 调用 `lx-entry-describe-ai-parse-content { "entry_id": "entry_xxx" }`

---

### 场景三：浏览目录树

**触发条件：**

- 用户说"看看目录"/"列出子页面"/"浏览文件夹"

**使用工具：** `lx-entry-list-children`

**SOP：**

1. 获取父条目 ID
2. 调用 `lx-entry-list-children { "parent_id": "parent_xxx" }`
3. 如需递归展开，对子文件夹继续调用

---

### 场景四：导入 Markdown/HTML

**触发条件：**

- 用户说"导入 markdown"/"把内容导入到新页面"

**使用工具：** `lx-entry-import-content` / `lx-entry-import-content-to-entry`

**SOP：**

1. 确定导入方式：
   - 创建新文档 → `lx-entry-import-content`
   - 追加到已有页面 → `lx-entry-import-content-to-entry`
2. 准备 base64 编码的内容
3. 调用对应工具传入 `content` 和 `content_type`

**特殊情况处理：**

- 已有页面优先局部编辑 → 切换到 `lx-block` skill
- 内容编码 → 使用 `markdown_base64` 或 `html_base64` 类型

---

### 场景五：文件上传

**触发条件：**

- 用户说"上传文件"/"传个附件"

**使用工具：** `lx-file-apply-upload` / `lx-file-commit-upload`

**SOP：**

1. 调用 `lx-file-apply-upload { "parent_entry_id": "folder_xxx", "name": "report.pdf", "upload_type": "PRE_SIGNED_URL" }`
2. 获取到 `upload_url` 后，HTTP PUT 上传文件内容
3. 调用 `lx-file-commit-upload { "session_id": "sess_xxx" }` 确认

---

### 场景六：草稿管理

**触发条件：**

- 用户说"保存草稿"/"发布草稿"

**使用工具：** `lx-draft-describe-markdown-draft` / `lx-draft-save-markdown-draft` / `lx-draft-publish-markdown-draft`

**SOP：**

1. 检查现有草稿 → `lx-draft-describe-markdown-draft { "entry_id": "entry_xxx" }`
2. 保存草稿 → `lx-draft-save-markdown-draft { "entry_id": "entry_xxx", "content": "...", "seq": 0 }`
3. 发布 → `lx-draft-publish-markdown-draft { "entry_id": "entry_xxx" }`

---

## 工具速查

| 工具名 | 用途 |
|--------|------|
| `lx-entry-create-entry` | 创建页面/文件夹 |
| `lx-entry-list-children` | 列出子条目 |
| `lx-entry-describe-entry` | 获取条目详情 |
| `lx-entry-describe-ai-parse-content` | 获取 AI 可解析内容 |
| `lx-entry-import-content` | 导入内容创建新文档 |
| `lx-entry-import-content-to-entry` | 导入内容到已有页面 |
| `lx-entry-move-entry` | 移动条目 |
| `lx-entry-rename-entry` | 重命名条目 |
| `lx-file-apply-upload` | 申请上传凭证 |
| `lx-file-commit-upload` | 确认上传完成 |
| `lx-file-download-file` | 获取下载地址 |
| `lx-draft-save-markdown-draft` | 保存草稿 |
| `lx-draft-publish-markdown-draft` | 发布草稿 |

---

## 典型组合流程

### 创建页面并导入内容

```json
// 1. 获取 root_entry_id
lx-space-describe-space: { "space_id": "sp_xxx" }

// 2. 创建空白页面
lx-entry-create-entry: { "parent_entry_id": "root_xxx", "name": "新文档", "entry_type": "page" }

// 3. 导入内容（切换到 lx-block 或 lx-entry-import-content-to-entry）
```

### 浏览文档目录

```json
// 1. 获取 root_entry_id
lx-space-describe-space: { "space_id": "sp_xxx" }

// 2. 获取一级目录
lx-entry-list-children: { "parent_id": "root_xxx" }

// 3. 逐级展开
lx-entry-list-children: { "parent_id": "folder_xxx" }
```
