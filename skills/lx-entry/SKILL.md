---
name: lx-entry
version: 1.2.0
description: "乐享知识库条目管理。当用户需要操作知识条目（创建、查看、编辑、删除页面/文件夹），导入内容，管理文件（上传、下载、版本控制），或处理 Markdown 草稿时使用。触发词：页面、文档、条目、文件夹、创建文档、导入、上传文件、草稿、版本"
metadata:
  requires:
    bins: ["lx"]
---

# 条目管理

> **前置条件：** 需要 `lx` CLI 已配置并登录。

## ⚡ 什么时候用这个 skill？

### 进入场景

- 用户说"创建页面"/"上传文件"/"查看某个文档"
- 用户说"导入 markdown 创建文档"/"管理草稿"
- 用户说"浏览目录树"/"重命名条目"/"移动条目"

### 禁止在本 skill 中执行

- **不要修改页面内部内容**：用户说"改一段内容"/"替换某个章节"/"改表格" → **立即切换到 lx-block skill**
- **不要进行可回滚的批量修改**：多步高风险修改 → **立即切换到 lx-git skill**，先用 `lx git clone` 建立本地工作区
- **不要在知识库中搜索**：用户说"搜索" → **立即切换到 lx-search skill**

## ⚡ 怎么选命令？（决策树）

```text
识别场景 →
├── 创建新页面/文件夹? → lx entry create-entry（需先获取 root_entry_id）
├── 查看/读取文档内容? → lx entry describe-ai-parse-content
├── 浏览目录树? → lx entry list-children（需先拿到 parent_id）
├── 导入 Markdown/HTML?
│   ├── Markdown 文件 → lx entry import markdown
│   ├── HTML 文件 → lx entry import html
│   ├── HTML 物料目录 → lx entry import html --dir
│   ├── 创建新文档（原文参数）→ lx entry import-content
│   └── 追加到已有页面 → lx entry import-content-to-entry（优先用 lx-block）
├── 上传本地文件? → lx file upload（自动完成 apply → PUT → commit）
├── 下载文件? → lx file download-file
├── 管理 Markdown 草稿? → lx draft describe/save/publish-markdown-draft
├── 管理条目标签? → lx knowledge-tag list-entry-tags / set-entry-tags
└── 移动/重命名条目? → lx entry move-entry / rename-entry
```

## ⚠️ 高风险操作与默认路径

**多步修改必须建立 checkpoint：**

- 用户要进行多步编辑、批量修改、或需要可回退的变更管理时
- **必须引导用户使用 lx-git skill**
- 纯在线编辑没有版本记录，一旦覆盖无法回滚

**默认优先路径：**

1. 已有页面内容改动 → 先切到 lx-block skill，**禁止** `import-content-to-entry --force-write`
2. 从本地 Markdown/HTML 整段导入 → 使用 `lx entry import markdown|html`，由 CLI 做标准 JSON 序列化
3. HTML 物料目录 → 使用 `lx entry import html <目录> --dir` 自动压缩上传
4. 其他文件条目上传 → 使用 `lx file upload`；预制 HTML ZIP 显式加 `--html-bundle`
5. 只有调试底层协议时才手工执行 `apply-upload` → HTTP PUT → `commit-upload`

## 可用工具（场景分组）

### 创建与浏览

| 命令 | 说明 | 参考 |
|------|------|------|
| `lx entry create-entry` | 创建页面/文件夹 | [entry-crud.md](references/entry-crud.md) |
| `lx entry list-children` | 列出子条目 | [entry-crud.md](references/entry-crud.md) |
| `lx entry describe-entry` | 获取条目详情 | [entry-crud.md](references/entry-crud.md) |
| `lx entry describe-ai-parse-content` | 获取 AI 可解析内容 | [entry-crud.md](references/entry-crud.md) |

### 内容导入

| 命令 | 说明 | 参考 |
|------|------|------|
| `lx entry import markdown` | 从本地 Markdown 导入 | [entry-import.md](references/entry-import.md) |
| `lx entry import html` | 导入 HTML 文件或物料目录 | [entry-import.md](references/entry-import.md) |
| `lx entry import-content` | 导入内容创建新文档 | [entry-import.md](references/entry-import.md) |
| `lx entry import-content-to-entry` | 导入内容到已有页面 | [entry-import.md](references/entry-import.md) |

### 文件管理

| 命令 | 说明 | 参考 |
|------|------|------|
| `lx file upload` | 上传本地文件并自动完成三步流程 | [entry-file.md](references/entry-file.md) |
| `lx file apply-upload` | 申请上传凭证（Step 1） | [entry-file.md](references/entry-file.md) |
| `lx file commit-upload` | 确认上传完成（Step 3） | [entry-file.md](references/entry-file.md) |
| `lx file download-file` | 获取文件下载地址 | [entry-file.md](references/entry-file.md) |
| `lx file describe-file` | 获取文件详情 | [entry-file.md](references/entry-file.md) |
| `lx file list-revisions` | 文件历史版本 | [entry-file.md](references/entry-file.md) |
| `lx file revert-file` | 恢复到指定版本 | [entry-file.md](references/entry-file.md) |

### 草稿与标签

| 命令 | 说明 | 参考 |
|------|------|------|
| `lx draft describe-markdown-draft` | 获取草稿 | [entry-draft.md](references/entry-draft.md) |
| `lx draft save-markdown-draft` | 保存草稿 | [entry-draft.md](references/entry-draft.md) |
| `lx draft publish-markdown-draft` | 发布草稿 | [entry-draft.md](references/entry-draft.md) |
| `lx knowledge-tag list-entry-tags` | 获取条目标签 | [entry-tag.md](references/entry-tag.md) |
| `lx knowledge-tag set-entry-tags` | 设置条目标签（增删） | [entry-tag.md](references/entry-tag.md) |

## 🎯 执行规则

1. **创建一级条目**：必须先通过 `lx space describe-space` 获取 `root_entry_id`，再将其作为 `--parent-entry-id` 传入。
2. **内容编码**：本地文件使用 `lx entry import markdown|html`，CLI 读取 UTF-8 原文并通过标准 JSON 序列化；`markdown_base64` 仅作历史兼容。
3. **已有页面优先局部编辑**：若目标页面已存在，**禁止**用 `lx entry import-content-to-entry --force-write` 覆盖，应优先使用 lx-block skill 的高级命令进行局部更新。
4. **文件上传**：优先 `lx file upload` 自动完成三步；手工调用时仍必须完整执行 apply → PUT → commit。
5. **条目访问链接**：`{domain}/pages/{entry_id}`
6. **`--after-block-id` 限制**：`lx entry import-content-to-entry` 的 `--after-block-id` 只能是页面第一层（根级别）的 block ID，不能是嵌套的子 block。

## 典型组合流程

### 创建页面并导入内容

```bash
# 获取 root_entry_id
lx space describe-space --space-id sp_xxx

# 直接从本地 Markdown 创建页面
lx entry import markdown ./document.md --parent-id root_xxx --name "新文档"
```

### 上传文件到知识库

```bash
lx file upload /path/to/report.pdf --parent-entry-id folder_xxx

# HTML 物料目录（自动压缩，目录根包含 index.html）
lx entry import html ./site --dir --parent-id folder_xxx

# 已经打好的 HTML bundle ZIP
lx file upload ./site.zip --parent-entry-id folder_xxx --html-bundle
```

### 浏览文档目录

```bash
# 获取 root_entry_id
lx space describe-space --space-id sp_xxx

# 获取一级目录
lx entry list-children --parent-id root_xxx

# 逐级展开子目录
lx entry list-children --parent-id folder_xxx
```

### 草稿编辑流程

```bash
# 检查是否有未发布草稿
lx draft describe-markdown-draft --entry-id entry_xxx

# 保存草稿
lx draft save-markdown-draft --entry-id entry_xxx --revision-id rev_xxx --content "..." --seq 0

# 发布为正式版本
lx draft publish-markdown-draft --entry-id entry_xxx --revision-id rev_xxx
```

### 管理条目标签

```bash
# 查看现有标签
lx knowledge-tag list-entry-tags --entry-id entry_xxx

# 增删标签
lx knowledge-tag set-entry-tags --entry-id entry_xxx --add-tags "重要" --del-tags "过时"
```
