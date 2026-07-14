# 命令参考

## 全局参数

大多数动态命令支持：

| 参数 | 说明 |
|------|------|
| `-o, --format <FORMAT>` | 输出格式：json/json-pretty/table/yaml/csv/markdown |
| `-d, --data-raw <JSON>` | 用 JSON 一次性传入全部参数 |
| `-h, --help` | 查看帮助 |

两种传参方式：

```bash
# 方式 1：逐参数传入（日常使用）
lx search kb --keyword "test" --limit 10 --type doc

# 方式 2：JSON 传入（脚本/复杂参数）
lx search kb -d '{"keyword":"test","limit":10,"type":"doc"}'
```

## 命令总览

| Namespace | 用途 | 常用命令 |
|-----------|------|---------|
| `search` | 搜索知识 | `lx search kb`, `lx search kb-embedding` |
| `team` | 团队信息 | `lx team list`, `lx team describe` |
| `space` | 知识库信息 | `lx space list`, `lx space describe` |
| `entry` | 条目与页面 | `lx entry describe`, `lx entry create`, `lx entry import` |
| `block` | 页面正文与文档块 | `lx block fetch`, `lx block update` |
| `smartsheet` | 智能表格详细操作 | `lx smartsheet list-fields`, `lx smartsheet create-records` |
| `file` | 文件上传下载 | `lx file upload`, `lx file download` |
| `comment` | 评论查询 | `lx comment list` |
| `ppt` | PPT 服务 | `lx ppt generate` |
| `meeting` | 会议录制 | `lx meeting import` |
| `contact` | 联系人 | `lx contact whoami` |

## 搜索

```bash
# 全局搜索
lx search kb --keyword "关键词"

# 指定知识库
lx search kb --keyword "关键词" --space-id <SPACE_ID>

# 指定类型
lx search kb --keyword "关键词" --type doc
lx search kb --keyword "关键词" --type kb_doc
lx search kb --keyword "关键词" --title-only

# 向量语义检索
lx search kb-embedding --keyword "如何部署服务"
```

## 团队与知识库

```bash
# 团队
lx team list
lx team list-frequent
lx team describe --team-id <TEAM_ID>

# 知识库
lx space list --team-id <TEAM_ID>
lx space list-recently
lx space describe --space-id <SPACE_ID>
```

`lx space describe` 返回的 `root_entry_id` 用于后续遍历目录。

## 条目

```bash
# 查看条目详情
lx entry describe --entry-id <ENTRY_ID>

# 遍历目录
lx entry list-children --parent-id <PARENT_ID>
lx entry list-latest --space-id <SPACE_ID>
lx entry list-parents --entry-id <ENTRY_ID>

# 创建
lx entry create --parent-entry-id <PARENT_ID> --name "新文档" --entry-type page
lx entry create --parent-entry-id <PARENT_ID> --name "新文件夹" --entry-type folder

# 导入内容
lx entry import markdown ./document.md --parent-id <PARENT_ID> --name "导入的文档"
lx entry import html ./page.html --space-id <SPACE_ID>
lx entry import markdown ./appendix.md --entry-id <ENTRY_ID>

# HTML 物料目录：自动压缩为 ZIP 并上传为 HTML bundle
lx entry import html ./dist --dir --parent-id <PARENT_ID>

# 低层：直接传原文
lx entry import-content --parent-id <PARENT_ID> --name "导入的文档" --content "# 标题\n\n正文"
lx entry import-content-to-entry --entry-id <ENTRY_ID> --content "追加的内容"

# 移动重命名
lx entry rename --entry-id <ENTRY_ID> --name "新名称"
lx entry move --entry-id <ENTRY_ID> --parent-entry-id <NEW_PARENT_ID>

# AI 可解析内容（Markdown/HTML）
lx entry describe-ai-parse-content --entry-id <ENTRY_ID>
```

## 文档块（Block）

读取和修改在线页面正文时，优先使用页面级 `fetch + update`；只有明确需要低层 block 能力时才使用原子命令。

### 页面级命令（默认）

```bash
# 先发现并读取服务端当前 MDX DSL
lx mcp resource list --format json
lx mcp resource read lexiang://docs/block-mdx/v0

# 文本编辑：获取可回写 block-markdown
lx block fetch --entry-id <ENTRY_ID> --render-mode markdown

# 结构化编辑：获取带 data-id 的 MDX
lx block fetch --entry-id <ENTRY_ID> --render-mode mdx

# 精确替换：old_str 必须原样来自 fetch 输出
lx block update -d '{
  "entry_id":"<ENTRY_ID>",
  "command":"update_content",
  "content_format":"markdown",
  "content_updates":[{"old_str":"旧内容","new_str":"新内容","replace_all_matches":false}],
  "dry_run":true
}'

# 整页替换
lx block update -d '{
  "entry_id":"<ENTRY_ID>",
  "command":"replace_content",
  "new_str":"# 新正文",
  "dry_run":true
}'
```

先以 `dry_run=true` 校验，通过后再执行相同 payload，并重新 `fetch` 验证。删除 child page、database 或 smartsheet 时必须显式设置 `allow_deleting_content=true`。

`fetch-page` / `update-page` 作为兼容 alias 保留；旧的单块更新已迁移为 `lx block update-block`。

### 动态命令（基础操作）

```bash
lx block describe --block-id <BLOCK_ID>
lx block list-children --block-id <BLOCK_ID>
lx block list-children --block-id <BLOCK_ID> --recursive
lx block update-block -d '{"block_id":"xxx","content":{"text":"新内容"}}'
lx block create-descendant -d '{"block_id":"xxx","descendant":{...}}'
lx block delete --block-id <BLOCK_ID>
lx block delete-children -d '{"block_id":"xxx","children_ids":["id1","id2"]}'
lx block move -d '{"block_ids":["id1"],"parent_block_id":"xxx"}'
lx block convert-content-to-blocks -d '{"content":"# 标题","content_type":"markdown"}'
```

### 低层静态增强命令（明确需要时）

```bash
# 列出子块（树形展示）
lx block ls --block-id <BLOCK_ID>

# 获取块内容（支持 MDX 输出）
lx block get --block-id <BLOCK_ID>
lx block get --block-id <BLOCK_ID> --format mdx

# 创建子块（自动 MDX→blocks 转换）
lx block create --block-id <BLOCK_ID> --content "# 标题\n\n正文"
lx block create --block-id <BLOCK_ID> --file ./doc.mdx

# 更新块
lx block update-block --block-id <BLOCK_ID> --text "新文本"
lx block update-block --block-id <BLOCK_ID> --content "$(cat doc.mdx)"

# 删除/移动
lx block delete --block-id <BLOCK_ID>
lx block move --block-ids id1,id2 --parent-block-id <TARGET_ID>

# 转换预览
lx block convert --content "# 标题" --from mdx --to blocks
```

> 静态命令优先于动态命令。当 `lx block <subcmd>` 匹配到静态实现时，不会走动态 MCP 调用。
> 见 [架构文档](../dev/module-boundaries.md) 的「规则 1：静态优先于动态」。

## 智能表格（Smartsheet）

新增高层接口与原有详细接口统一位于 `lx smartsheet`：

```bash
# schema DDL、记录和值类型、视图 DSL 的服务端契约
lx mcp resource list --format json
lx mcp resource read lexiang://docs/block-view-dsl/v0

# 列出与获取 schema/视图摘要
lx smartsheet list --entry-id <ENTRY_ID>
lx smartsheet fetch --entry-id <ENTRY_ID> --smartsheet-id <SMARTSHEET_ID>

# 分页读取记录
lx smartsheet list-records -d '{
  "entry_id":"<ENTRY_ID>",
  "smartsheet_id":"<SMARTSHEET_ID>",
  "page_size":50
}'

# 通过完整 SQL DDL 创建/修改 schema
lx smartsheet create -d '{
  "target_type":"kb_entry",
  "parent_entry_id":"<PARENT_ENTRY_ID>",
  "title":"项目跟踪",
  "schema":"CREATE TABLE ..."
}'
lx smartsheet update-schema -d '{
  "entry_id":"<ENTRY_ID>",
  "smartsheet_id":"<SMARTSHEET_ID>",
  "statements":["ALTER TABLE ..."]
}'
```

字段、记录和视图的详细 CRUD 也位于 `lx smartsheet *`：

```bash
lx smartsheet --help
lx smartsheet list-fields --help
lx smartsheet create-records --help
lx smartsheet update-view --help
```

复杂对象和数组统一使用 `-d '<JSON>'`；记录变更单次最多 50 条。写入前先 fetch/list 获取真实的 `entry_id`、`smartsheet_id`、字段/记录/视图 ID。

服务端目前仍把部分高层接口归类在 `knowledge.block`；CLI 已将它们提升到 `lx smartsheet`，原有 `lx block smartsheet-*` 入口暂时保留兼容。

## 文件

```bash
# 查看
lx file describe --file-id <FILE_ID>
lx file download --file-id <FILE_ID>

# 推荐：从本地文件完整上传
lx file upload ./file.pdf --parent-entry-id <PARENT_ID>
lx file upload ./demo.mp4 --parent-entry-id <PARENT_ID>
lx file upload ./site.zip --parent-entry-id <PARENT_ID> --html-bundle

# 更新已有文件（parent-entry-id 是文件条目自身 ID）
lx file upload ./file-v2.pdf --parent-entry-id <FILE_ENTRY_ID> --file-id <FILE_ID>

# 低层调试：手工三步走
lx file apply-upload --parent-entry-id <PARENT_ID>   # 1. 申请凭证
curl -X PUT "<upload_url>" --data-binary @file.pdf     # 2. 上传文件
lx file commit-upload --session-id <SESSION_ID>        # 3. 确认

# 导入链接
lx file create-hyperlink --url "https://..." --space-id <SID> --parent-entry-id <PID>
```

## 其他业务 Tool

```bash
# 评论
lx comment list --target-id <ENTRY_ID>
lx comment describe --target-id <ENTRY_ID>

# PPT
lx ppt generate -d '{"planning":"10页产品介绍","context":"..."}'
lx ppt get-task --id <TASK_ID>
lx ppt add-pages / modify-pages / delete-pages / reorder-pages

# 会议
lx meeting search --meeting-id <CODE>
lx meeting describe --record-id <RECORD_ID>
lx meeting import -d '{"record_id":"xxx", ...}'

# 联系人
lx contact search-staff --keyword "张三"
lx contact whoami

# iWiki
lx iwiki import -d '{"page_id":"123", ...}'
```

## 低层调试：`lx mcp`

绕过动态命令直接调用 MCP Tool，或读取 MCP Resource：

```bash
lx mcp list                          # 列出所有可用工具
lx mcp call entry_describe --params '{"entry_id":"xxx"}'  # 直接调用
lx mcp resource list --format json   # 发现服务端 Resource
lx mcp resource read lexiang://docs/block-mdx/v0
```

当动态命令描述关联了 `lexiang://` Resource 时，`lx <namespace> <command> --help` 会显示 Resource preflight 和可执行的读取命令。智能表格工具在服务端补齐 URI 描述前，CLI 会主动提示读取 `block-view-dsl`。
