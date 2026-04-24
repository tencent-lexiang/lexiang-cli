---
name: lx-block
version: 1.1.0
description: "乐享文档块编辑。当用户需要对知识库页面进行结构化编辑（增删改查块、表格操作、章节替换、内容导入导出）时使用。触发词：block、编辑文档、修改内容、表格、块、插入、追加、替换章节"
metadata:
  requires:
    bins: ["lx"]
---

# 文档块编辑

> **前置条件：** 需要 `lx` CLI 已配置并登录。

## ⚡ 什么时候用这个 skill？

**进入场景：**

- 用户说"编辑某个页面"/"修改某个章节"/"改表格"
- 用户说"在文档里插入内容"/"追加内容"
- 用户说"替换某个标题下的内容"/"导入 markdown 到文档"
- Agent 需要对文档进行多次修改

**禁止在本 skill 中执行：**

- **不要创建新页面**：用户说"创建一个新页面" → **立即切换到 lx-entry skill**
- **不要推送修改到远程**：用户说"推送"/"commit"/"push" → **立即切换到 lx-git skill**
- **不要浏览目录结构**：用户说"浏览知识库"/"看看目录" → **立即切换到 lx-sh skill**

## ⚡ Agent 工作流

当 Agent 需要编辑文档时，遵循以下标准流程：

### 标准编辑循环

```text
1. 定位页面 → lx entry list-children / search_kb_search
2. 获取 entry_id 和 page block 信息
3. 查找目标块 → lx block find -q "关键词" -e <entry_id>
4. 执行编辑 → lx block update / insert-after / replace-section / table-set ...
5. 验证结果 → lx block get -b <block_id> -e <entry_id> --format mdx
```

### 多次修改同一文档的场景

当需要对同一页面进行多处修改时：

```bash
# Step 1: 先用 find 定位所有需要修改的块，一次性获取所有 block_id
lx block find -q "API" -e <entry_id> --limit 20
# 输出: [1] h2 5b9490... "什么是 API"
#       [2] h3 e06877... "API 设计原则"

# Step 2: 用获取到的 block_id 逐一修改
lx block update -b 5b9490629433... -e <entry_id> --text "新内容"
lx block update -b e06877de8115... -e <entry_id> --text "更新后的原则"
```

**关键规则：不要让 agent 读完整 MDX 来找内容。** 用 `lx block find` 做索引式定位。

| 场景 | 推荐方式 | 避免 |
|------|---------|------|
| 找特定标题 | `find -m heading -q "完整标题文本"` | 读全部 MDX 再 grep |
| 找包含某词的块 | `find -q "关键词"` | `export` 全文再搜索 |
| 找所有表格 | `find -m type -q "table"` | 遍历整个树 |
| 找特定层级的标题 | `find -m type -q "h2"` + 筛选 | 手动遍历 |

### 参数传递约定

**所有 block 操作都需要 `--entry-id / -e <entry_id>`**，这是 MCP 服务端的必填参数。

```bash
# 正确 ✅ — 总是带 entry_id
lx block ls -e <entry_id>
lx block find -q "关键词" -e <entry_id>
lx block update -b <block_id> -e <entry_id> --text "新内容"
```

## ⚡ 怎么选命令？（决策树）

```text
├── 需要**查找/定位**块？
│   ├── 按文本搜索? → lx block find -q "关键词" [-m text]
│   ├── 按标题精确匹配? → lx block find -q "标题文本" [-m heading]
│   ├── 按类型过滤? → lx block find -q "h2/table/code" [-m type]
│   └── 看树形结构? → lx block tree
├── 修改表格?
│   ├── 读取表格 → lx block table-get
│   ├── 改单元格 → lx block table-set
│   ├── 加一行 → lx block table-add-row
│   └── 删一行 → lx block table-del-row
├── 替换某个章节? → lx block replace-section
├── 在某处插入内容? → lx block insert-after
├── 在页面末尾追加? → lx block append
├── 导入整个 markdown 文件? → lx block import
├── 导出文档内容? → lx block export
└── 精细控制单个块?
    ├── 读取块 → lx block get
    ├── 创建块 → lx block create
    ├── 更新块 → lx block update
    ├── 删除块 → lx block delete
    ├── 移动块 → lx block move
    └── 转换内容 → lx block convert
```

## ⚠️ 高风险操作与默认优先路径

**高级命令优先，原子命令兜底：**

- 能用高级命令（table-* / replace-section / import 等）就不要读原子命令
- 原子命令只在高级命令无法表达时使用
- **查询优先用 `find`，不要 `ls` + 人肉筛选**

**默认优先路径：**

1. **先查再改** → `find` 定位目标块 → 再执行修改
2. 表格操作 → 用 `table-*` 高级命令
3. 章节替换 → 用 `replace-section`
4. 批量导入 → 用 `import --chunk-size 20` 自动分批
5. 精细控制单个块 → 才回退到原子命令

**大文档必须分批：**

- 大文档导入使用 `lx block import --chunk-size 20` 自动分批，避免单次请求过大

## 可用工具

### 查询（新增）

| 命令 | 说明 |
|------|------|
| `lx block find` | 按文本/标题/类型搜索块 |

**三种搜索模式：**

```bash
# 文本子串搜索（不区分大小写）
lx block find -q "API" -e <entry_id>

# 标题精确匹配（需完整标题文本）
lx block find -q "一、什么是 Harness Engineering" -m heading -e <entry_id>

# 按类型过滤
lx block find -q "h2" -m type -e <entry_id>      # 所有二级标题
lx block find -q "table" -m type -e <entry_id>     # 所有表格
lx block find -q "code" -m type -e <entry_id>      # 所有代码块
```

**输出格式：** 可读摘要 + JSON（程序化消费）

```text
[1]   [h2] 5b9490629433 "一、什么是 Harness Engineering"
     path:  → 5b94906294334f7aaf29190a5e4ab20e

{
  "query": "...",
  "mode": "text",
  "count": 1,
  "matches": [{ "id": "...", "block_type": "H2", "text": "...", "path": [...] }]
}
```

### 核心 CRUD

| 命令 | 说明 |
|------|------|
| `lx block ls` | 列出子块（支持 `-r` 递归） |
| `lx block get` | 获取块详情（支持 `--format mdx` 导出为 MDX） |
| `lx block create` | 创建子块（支持 MDX 自动转换） |
| `lx block update` | 更新块（支持 MDX 自动转换） |
| `lx block delete` | 删除块及其子孙 |
| `lx block move` | 移动块到新位置 |

### 高级命令（默认优先使用）

| 命令 | 说明 | 参考 |
|------|------|------|
| `lx block table-get` | 读取表格结构 | [block-advanced.md](references/block-advanced.md) |
| `lx block table-set` | 修改单元格 | [block-advanced.md](references/block-advanced.md) |
| `lx block table-add-row` | 追加行 | [block-advanced.md](references/block-advanced.md) |
| `lx block table-del-row` | 删除行 | [block-advanced.md](references/block-advanced.md) |
| `lx block replace-section` | 按标题替换章节 | [block-advanced.md](references/block-advanced.md) |
| `lx block insert-after` | 在指定块后插入 | [block-advanced.md](references/block-advanced.md) |
| `lx block append` | 追加到末尾 | [block-advanced.md](references/block-advanced.md) |
| `lx block export` | 导出为 markdown/json | [block-advanced.md](references/block-advanced.md) |
| `lx block tree` | 显示块树结构 | [block-advanced.md](references/block-advanced.md) |
| `lx block import` | 导入 markdown（自动分批）| [block-advanced.md](references/block-advanced.md) |

### 转换

| 命令 | 说明 |
|------|------|
| `lx block convert` | MDX ↔ Block JSON 本地转换（不经过服务端） |
| `lx block export --format mdx` | 导出页面为 MDX 格式 |
| `lx block import` | 从 Markdown/MDX 文件导入（自动分批） |

MDX 格式参考: [mdx-reference.md](references/mdx-reference.md)

## 典型组合流程

### Agent 编辑文档的标准流程

```bash
# 1. 确认目标页面（从上下文或搜索获得 entry_id）
ENTRY_ID="26ab85ae2ff74cf3a22564846920d628"

# 2. 查找需要修改的块
lx block find -q "API" -e $ENTRY_ID
# 结果: [1] h2 xxx "API 接口说明" → block_id=5b9490629433...

# 3. 执行修改（用上一步获得的 block_id）
lx block update -b 5b9490629433... -e $ENTRY_ID \
  --content "## API 接口说明（已更新）"

# 4. 验证修改结果
lx block get -b 5b9490629433... -e $ENTRY_ID --format mdx
```

### 修改多个不相关的块

```bash
# 一次搜索找到所有目标
lx block find -q "TODO" -e $ENTRY_ID --limit 50
# 输出 10+ 个匹配块，每个都有 block_id

# 逐个修改
for bid in $(cat find_result.json | jq -r '.matches[].id'); do
  lx block update -b "$bid" -e $ENTRY_ID --text "已完成"
done
```

### 修改表格单元格

```bash
# 查看
lx block table-get -b tbl_xxx

# 修改
lx block table-set -b tbl_xxx --row 2 --col 1 --text "修正值"
```

### 替换文档中的某个章节

```bash
lx block replace-section -b root_xxx --heading "## API 参考" \
  --file ./updated-api.md
```

### 使用原子命令精细编辑

```bash
# 创建块（MDX 自动转换）
lx block create -b parent_xxx -e $ENTRY_ID --content "## 新章节\n\n详细内容..."

# 批量更新
lx block update -b blk_xxx -e $ENTRY_ID --descendant '<blocks JSON>'
```

### AI 生成 MDX 结构化内容导入

```bash
# 方式 1：直接导入 .mdx 文件
lx block import -b page_xxx --file ./doc.mdx --chunk-size 20

# 方式 2：导出页面为 MDX 格式
lx block export -b page_xxx --format mdx --output ./doc.mdx

# 方式 3：转换预览（不写入，只看结果）
lx block convert --from mdx --to blocks --content "<MyCallout title='注意'>...</MyCallout>"
```

> 详见 [MDX 格式参考](references/mdx-reference.md) 了解完整的 MDX 组件语法和对应 Block JSON 结构。
