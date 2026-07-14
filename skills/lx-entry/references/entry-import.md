# entry import — 本地内容与 HTML 物料导入

> **前置条件：** 先阅读 [`../SKILL.md`](../SKILL.md) 了解条目管理的整体决策树。

命令路径显式选择 Markdown 或 HTML。普通文件导入为可编辑页面；只有 HTML `--dir` 模式会创建自动压缩的 HTML bundle 文件条目。

## 从本地文件创建新页面

```bash
# Markdown：在父目录下创建
lx entry import markdown ./plan.md \
  --parent-id root_xxx \
  --name "项目计划书"

# HTML：在知识库根节点创建
lx entry import html ./index.html \
  --space-id sp_xxx \
  --name "产品介绍"
```

格式由命令路径决定，不依赖文件扩展名。因此 Markdown 原文即使使用 `.txt` 后缀，也可以明确导入：

```bash
lx entry import markdown ./source.txt --parent-id root_xxx
```

## 导入 HTML 物料目录

```bash
lx entry import html ./dist \
  --dir \
  --parent-id folder_xxx \
  --name product-site
```

CLI 会在申请上传 session 前完成以下工作：

1. 要求 `./dist/index.html` 是普通文件。
2. 递归收集普通文件并拒绝符号链接。
3. 以目录内部相对路径生成 ZIP，不添加 `dist/` 包装层。
4. 将远端名称补成 `.zip`，并检查压缩后不超过 10 MiB。
5. 按 `application/zip + extension=html` 完成 apply → PUT → commit。

目录模式只接受 `--parent-id`、可选 `--name` 和输出格式；它创建 HTML bundle 文件条目，不是可编辑页面。

## 导入到已有页面

```bash
# 默认追加到末尾
lx entry import markdown ./appendix.md --entry-id entry_xxx

# 在页面第一层 block 后插入
lx entry import html ./appendix.html \
  --entry-id entry_xxx \
  --after-block-id block_xxx
```

`--after-block-id` 只能是页面第一层（根级别）的 block ID。

## 完全覆盖已有页面（危险）

```bash
lx entry import markdown ./replacement.md \
  --entry-id entry_xxx \
  --force-write
```

`--force-write` 会清空页面现有内容。只有用户明确要求整体重写时才能使用；局部修改应切换到 `lx-block`。

## 直接传原文（低层动态命令）

```bash
lx entry import-content \
  --content "# 标题\n\n正文" \
  --content-type markdown \
  --name "新文档" \
  --parent-id root_xxx
```

当前 MCP schema 要求通过标准 JSON 序列化传输 Markdown/HTML 原文。`markdown_base64` 仅为历史兼容；新接入不要强制 base64，也不存在 `html_base64` 这一当前枚举值。

## 关键规则

1. 本地文件使用 `lx entry import markdown|html`，避免 shell 转义破坏内容。
2. 页面导入使用 `--space-id`、`--parent-id` 或 `--entry-id`，三者互斥。
3. HTML `--dir` 只支持 `--parent-id`，并创建文件条目。
4. 已有页面局部修改优先使用 `lx block` 页面级命令。
5. 页面导入源必须是有效 UTF-8；目录 bundle 中的静态资源可以是任意字节。

## 详细参数

```bash
lx entry import markdown --help
lx entry import html --help
lx entry import-content --help
lx entry import-content-to-entry --help
```
