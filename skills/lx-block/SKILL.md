---
name: lx-block
version: 1.3.0
description: "乐享页面正文与文档块编辑。当用户需要读取或修改页面正文、替换章节、增删块、编辑表格或导入导出内容时使用。触发词：block、编辑页面、修改正文、替换内容、表格、块、MDX"
metadata:
  requires:
    bins: ["lx"]
---

# 页面与文档块编辑

> 前置条件：`lx` CLI 已配置并登录；所有远端修改都应明确携带 `entry_id`。

## 默认工作流

普通 `entry_type=page` 正文编辑，默认使用页面级协议：

```text
1. 发现并读取服务端 DSL Resource
2. 获取页面 → lx block fetch
3. 根据任务选择 markdown 或 mdx
4. 先 dry-run → lx block update --dry-run
5. 执行同一更新
6. 再次 fetch 验证
```

使用 MDX 或不确定页面协议时，不要凭记忆构造语法，先执行：

```bash
lx mcp resource list --format json
lx mcp resource read lexiang://docs/block-mdx/v0
```

若列表返回了更新版本的 `block-mdx` URI，应读取并使用列表中的当前 URI。

- 文本搜索替换：先以 `render_mode=markdown` 获取原文，再用 `update_content`。
- 结构化编辑：先以 `render_mode=mdx` 获取带 `data-id` 的内容，再用 `replace_blocks`。
- 整页重写：使用 `replace_content`。
- 明确删除若干块：使用 `delete_blocks`。
- 数组或对象参数统一通过 `-d '<JSON>'` 传入。

具体命令见 [page-update.md](references/page-update.md)。

## 边界

- 创建页面条目、移动或重命名条目：切换到 `lx-entry`。
- 智能表格 schema、记录和视图：切换到 `lx-smartsheet`。
- 需要可回滚的多步修改：切换到 `lx-git`，先建立本地工作区。
- 普通页面正文更新不要优先走导入/转换链路；导入只用于外部文件或格式转换。
- 原子 block 命令只用于调用方明确要求的低层 block 操作。

## 命令选择

```text
页面正文编辑？
├── 阅读或取得可回写内容 → fetch
├── 整页替换 → update / replace_content
├── 精确搜索替换或追加 → update / update_content
├── 按 block_id 删除 → update / delete_blocks
├── 按 block_id 替换结构 → update / replace_blocks
├── 智能表格 → lx-smartsheet
└── 明确需要低层块能力
    ├── 查找块 → find
    ├── 单块增删改查 → get/create/update-block/delete
    ├── 移动块 → move
    ├── 表格块辅助操作 → table-*
    └── 导入导出 → import/export
```

## 页面级命令

| 命令 | 用途 |
|------|------|
| `lx block fetch` | 获取可回写 markdown、结构化 mdx 或只读 clean 内容 |
| `lx block update` | 命令式整页替换、精确替换、删除块或替换块 |

兼容 alias `fetch-page` / `update-page` 仍可用；旧的单块更新命令现为 `lx block update-block`。

## 低层与本地增强命令

| 场景 | 命令 | 参考 |
|------|------|------|
| 搜索和单块 CRUD | `find`, `ls`, `get`, `create`, `update-block`, `delete`, `move` | [block-basic.md](references/block-basic.md) |
| 表格、章节、插入、导入导出 | `table-*`, `replace-section`, `insert-after`, `append`, `import`, `export`, `tree` | [block-advanced.md](references/block-advanced.md) |
| MDX 组件格式 | `convert` 及结构化内容 | [mdx-reference.md](references/mdx-reference.md) |

## 安全规则

1. 修改前必须 `fetch`，更新中的 `old_str` 必须精确取自其输出。
2. 默认先设置 `dry_run=true`；通过后再以相同 payload 执行真实写入。
3. 删除 child page、database 或 smartsheet 时，必须显式设置 `allow_deleting_content=true`。
4. `clean` 只适合阅读，不作为回写输入。
5. MDX 属性使用双引号，已有块保留 `data-id`。
6. 修改完成后再次 `fetch` 页面，验证目标内容和结构。
7. `--help` 中出现 Resource preflight 时，必须先执行其中的读取命令。
