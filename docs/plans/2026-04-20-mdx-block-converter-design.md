# Design: MDX ↔ Block 双向转换器

## Context

当前 `lexiang-cli` 的 block 服务以 **Markdown** 作为 AI 到 block 的唯一中间格式：

- 输入：`markdown_to_blocks()` → 调 MCP `block_convert_content_to_blocks(content_type: "markdown")`
- 输出：`render_blocks_to_markdown()` → 简单的 block→markdown 渲染（丢失大量语义）

**问题**：Markdown 无法表达你贴出的 AI Ingest Spec 中的丰富结构：

- 无 `Callout` / `ColumnList` / `Todo` 嵌套
- 无法携带 block-level 属性 (`textAlign`, `blockColor`, `borderColor`)
- 无法表达 frontmatter 元数据
- 行内样式只能用 Markdown 原生语法，无法精确映射到 block 的 style system

**目标**：新增独立的 MDX 工作流，实现双向可靠转换。

## Constraints (Hard)

1. **MCP 后端不变**：`block_convert_content_to_blocks` 只接受 `"markdown"` | `"html"`
2. **MDX 在 CLI 本地解析**：不能要求后端支持 mdx
3. **MDX 不走 `block_convert_content_to_blocks`**：DocIR 直接在 CLI 本地构造完整 Block JSON（descendant 结构），然后调用 `block_create_block_descendant` 插入。**绕过后端转换器**。
4. **现有 markdown 链路保留**：CLI 的 git/pull/push 等链路继续用 markdown，不强制切换
5. **Block JSON 是 ground truth**：所有 round-trip 必须与 MCP 返回的 block JSON 一致

## Architecture

```text
┌─────────────┐     parse      ┌──────────┐     serialize     ┌─────────────┐
│   MDX 文本   │ ──────────►  │   DocIR  │ ◄────────────── │  Block JSON  │
│  (.mdx 文件) │               │ (中间表示) │                 │ (MCP 返回)   │
└─────────────┘                └──────────┘                  └─────────────┘
        │                              │ ▲                            │
        │ render                       │ │ from_json()                │
        ▼                              │ │                            ▼
┌─────────────┐               ┌──────────┴┴──┐               ┌─────────────┐
│   MDX 输出   │ ◄──────────── │ BlockAdapter │               │  后端 MCP    │
│  (roundtrip) │   emit_mdx()  │              │               │             │
└─────────────┘               └──────┬───────┘               └─────────────┘
                                      │
                    ir_to_descendant()│  (本地构造完整 Block JSON)
                                      ▼
                               ┌─────────────────────┐
                               │block_create_block_    │
                               │descendant (直接插入)  │  ← 不经过后端转换器
                               └─────────────────────┘

  旧链路（保留不动）：
  Markdown → block_convert_content_to_blocks(MCP) → Block JSON → 插入
```

### 核心组件

#### 1. DocIR (Document Intermediate Representation)

```rust
// crates/lx/src/service/block/ir/mod.rs

/// 文档节点类型
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeType {
    Document,
    Paragraph,
    Heading { level: u8 },
    BlockQuote,
    Callout { icon: Option<String>, border_color: Option<String> },
    ColumnList,
    Column { width: Option<String> },
    Divider,
    Image { src: String, alt: Option<String>, align: Option<String> },
    Table,
    TableRow,
    TableCell,
    Todo { checked: bool },
    BulletedList,
    NumberedList,
    CodeBlock { language: Option<String> },
    MathBlock { width: Option<f64> },
    Text,           // 叶子文本节点
}

/// 内联样式属性
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InlineStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub color: Option<String>,
    pub background_color: Option<String>,
}

/// 块级属性
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlockAttrs {
    pub text_align: Option<String>,       // left | center | right
    pub block_color: Option<String>,      // BLOCK_COLORS token
    pub border_color: Option<String>,     // BORDER_COLORS token (Callout)
    pub icon: Option<String>,             // Emoji (Callout)
    pub width: Option<String>,            // px (Image) or % (Column)
    pub height: Option<String>,           // px (Image)
}

/// 文档树节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub node_type: NodeType,
    pub text: Option<String>,             // 叶子文本内容
    pub attrs: BlockAttrs,                // 块级属性
    pub inline_style: Option<InlineStyle>, // 行内样式 (仅 Text 节点)
    pub href: Option<String>,             // 链接地址 (仅 Link 场景)
    pub children: Vec<Node>,              // 子节点
}
```

**设计决策**：

- `Node` 同时承载块级和行内信息（通过 `inline_style` 区分）
- 属性值使用 **String token** 而非 enum，方便扩展且与你的 AI Ingest Spec 白名单对齐
- `Option<T>` 表示"未设置"，区分于默认值

#### 2. MDX Parser (MDX → DocIR)

```rust
// crates/lx/src/service/block/mdx/parser.rs

/// 将 MDX 文本解析为 DocIR
///
/// 支持的语法（基于 AI Ingest Spec + 扩展）：
/// - Markdown 原生：# ## ###、```code```、> quote、---、![img]()
/// - MDX 组件：<Heading level="1">、<Callout>、<Table>、<Todo>、<Mark>
/// - Frontmatter：YAML → Node::Document.attrs 扩展字段
pub fn parse_mdx(input: &str) -> Result<Node>

/// 解析 YAML frontmatter
fn parse_frontmatter(input: &str) -> Result<(Option<Frontmatter>, &str)>

/// 解析 MDX 组件标签
fn parse_component(tag_name: &str, attrs: &Attributes, body: &str) -> Result<Node>

/// 解析行内 <Mark> 和 <Link>
fn parse_inline(text: &str) -> Result<Vec<Node>>
```

**Parser 策略**：

- 不依赖外部 MDX parser 库（Rust 生态选择有限）
- 使用 **手写 recursive descent parser**，分 3 层：
  1. **Block layer**：识别组件标签 / Markdown 块结构
  2. **Inline layer**：识别 `<Mark>` / `<Link>` / 纯文本
  3. **Text layer**：提取纯文本内容

**优先级规则**（当同一位置同时出现 Markdown 和 MDX 组件时）：

1. MDX 组件优先（`<Callout>` > `> quote`）
2. 纯 Markdown 兜底（不涉及降级，只是兼容）

#### 3. MDX Emitter (DocIR → MDX)

```rust
// crates/lx/src/service/block/mdx/emitter.rs

/// 将 DocIR 序列化为 MDX 文本
///
/// 输出严格遵循 AI Ingress Spec 格式规范：
/// - 三段式多行写法（强制）
/// - 4 空格缩进
/// - 属性使用双引号
/// - 布尔属性不写值
pub fn emit_mdx(node: &Node) -> String

pub fn emit_markdown_fallback(node: &Node) -> String
```

**Emitter 保证**：

- Round-trip 一致性：`parse_mdx(emit_mdx(node)) == node`（在支持范围内）
- 格式化输出：统一缩进、换行、空行规则
- 未知组件：跳过并 warning（不 crash）

#### 4. Block Adapter (DocIR ↔ Block)

```rust
// crates/lx/src/service/block/adapter.rs

/// DocIR → 完整 descendant JSON（直接传给 block_create_block_descendant）
///
/// 关键：不经过后端 block_convert_content_to_blocks，
/// CLI 本地构造与后端完全兼容的 Block JSON 结构。
pub fn ir_to_descendant(node: &Node) -> serde_json::Value

/// Block JSON → DocIR（从 MCP 读取后转换）
pub fn block_to_ir(blocks: &[Block]) -> Node

/// DocIR → Markdown（仅用于 git/push 等旧链路的兼容输出）
pub fn ir_to_markdown(node: &Node) -> String
```

**`ir_to_descendant` 输出结构**（必须与 MCP `CreateBlockDescendant` 的 descendant 参数完全一致）：

```json
{
  "type": "paragraph",
  "content": {
    "text": [
      { "type": "text", "text": "hello ", "bold": false },
      { "type": "text", "text": "world", "bold": true }
    ]
  },
  "children": [
    {
      "type": "h2",
      "content": { "text": [{ "type": "text", "text": "标题" }] },
      "children": []
    }
  ]
}
```

每个节点必须包含：

- **`type`**: 对应 `BlockType::as_str()` 返回的字符串
- **`content`**: 块内容对象（结构因 type 而异，见下方映射表）
- **`children`**: 子块数组

**映射表**（DocIR → Block JSON content 结构）：

| DocIR NodeType | → Block.type | → Block.content 结构 |
|---|---|---|
| Paragraph | `"paragraph"` | `{ text: [Inline[]] }` |
| Heading { n } | `"h{n}"` | `{ text: [Inline[]] }` ，n ∈ 1..5 |
| Callout | `"quote"` | `{ text: [Inline[]], callout: true, borderColor?, icon? }` |
| BulletedList | `"bullet_list"` | `{ text: [Inline[]] }` |
| NumberedList | `"numbered_list"` | `{ text: [Inline[]] }` |
| Todo { checked } | `"task"` | `{ done: bool, text: [Inline[]] }` |
| CodeBlock | `"code"` | `{ language: string, text: string }` |
| Table | `"table"` | children = [TableRow] |
| TableRow | `"table_row"` | children = [TableCell] |
| TableCell | `"table_cell"` | `{ text: [Inline[]] }` |
| Image | `"image"` | `{ url: string, text: string }` |
| Divider | `"divider"` | `{}` |
| BlockQuote | `"quote"` | `{ text: [Inline[]] }` |

**不支持的后端 block 类型（Phase 1 不处理）**：`Attachment`, `Video`, `Mermaid`, `PlantUml`

**MDX 中不支持的组件（Phase 1 报错/跳过）**：`<MathBlock>`（后续按需扩展）

**Inline 结构**（用于 Paragraph / Heading / Quote / Todo / TableCell 等）：

```typescript
// 对应 content.text[] 数组中的元素
interface Inline {
  type: "text" | "mention" | "equation";
  text?: string;           // 文本内容
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  strikeThrough?: boolean;
  // 颜色相关（如果后端支持）
  color?: string;
  backgroundColor?: string;
}
```

**关键优势**：

- CLI 直接构造 Block JSON，**不依赖后端的 markdown parser**
- AI 生成的 MDX 语义由 CLI 翻译成正确的 block 结构
- 行内样式（bold/italic/strike）通过 `Inline[]` 精确控制
- **不支持 = 不支持，不搞降级变通**：MDX 中出现未注册的组件直接报错或跳过并 warning

### CLI 集成点

#### 新增命令

```bash
# 导入 MDX 文件到页面（本地解析 → 直接构造 Block JSON → 插入）
lx block import-mdx --entry-id xxx --file ./doc.mdx [--chunk-size 20]

# 导出页面为 MDX（替代 export --format markdown）
lx block export --entry-id xxx --format mdx --output ./doc.mdx

# 转换预览（不写入，只看结果：MDX → 本地 Block JSON）
lx block convert-mdx --file ./doc.mdx [--output-format json|mdx]

# 校验 MDX 文件是否符合 AI Ingest Spec
lx block validate-mdx --file ./doc.mdx [--strict]
```

**`import-mdx` 内部流程**：

```text
1. 读取 .mdx 文件
2. parse_mdx() → DocIR 树
3. ir_to_descendant() → 完整 Block JSON（descendant 结构）
4. 按 chunk_size 分批调用 block_create_block_descendant 插入
   （复用现有 import_markdown 的分批逻辑，只是数据来源不同）
```

**与旧 `import` 的区别**：

| | `lx block import` (旧) | `lx block import-mdx` (新) |
|---|---|---|
| 输入格式 | Markdown | MDX |
| 转换方式 | 后端 `block_convert_content_to_blocks` | **CLI 本地 `ir_to_descendant()`** |
| 语义保真度 | 丢失 Callout/ColumnList/属性等 | 完全保留（支持范围内一一对应） |
| 适用场景 | 简单文档 / git 工作流 | AI 生成的结构化内容 |

#### 修改现有命令

```bash
# lx block convert-content-to-blocks 新增 content_type
lx block convert-content-to-blocks \
  --content "..." \
  --content-type mdx \        # 新增：触发本地 MDX parser + ir_to_descendant()
                             # 默认仍是 markdown（走后端转换）
```

**注意**：当 `--content-type mdx` 时，不再调用 MCP `block_convert_content_to_blocks`，而是本地完成全部转换后返回 descendant JSON。

### Skill Reference 升级

当前 `skills/lx-block/references/` 只包含**命令参考**。升级后新增：

```text
skills/lx-block/
├── SKILL.md                    # 不变：命令决策树
├── references/
│   ├── block-basic.md          # 不变：原子操作
│   ├── block-advanced.md       # 不变：高级命令
│   └── mdx-reference.md        # 新增：MDX 语义参考 ← 这里是关键
│       ├── # 支持的组件清单
│       ├── # 每个组件的示例（MDX 源码 + 对应 block 结构）
│       ├── # 组件能力对照表（哪些支持 / 哪些不支持）
│       └── # 推荐模板（API 文档、PRD、技术方案等场景）
```

`mdx-reference.md` 的核心价值：**让 AI 知道"这个场景该产出什么结构的 MDX"**，而不只是"用什么命令"。

## Implementation Plan

### Phase 1: DocIR + MDX Parser + Block Adapter (核心基础)

**文件变更**：

```text
crates/lx/src/service/block/
├── ir/
│   ├── mod.rs          # Node / NodeType / BlockAttrs / InlineStyle 定义
│   └── traits.rs       # Parse / Emit trait 抽象
├── mdx/
│   ├── mod.rs          # parse_mdx / emit_mdx 入口
│   ├── parser.rs       # MDX → DocIR 实现
│   ├── emitter.rs      # DocIR → MDX 实现
│   └── validator.rs    # AI Ingest Spec 白名单校验
└── adapter.rs          # DocIR ↔ Block 转换（核心：ir_to_descendant 本地构造完整 Block JSON）
```

**任务清单**：

- [ ] 定义 `ir/mod.rs` 数据模型（Node + NodeType + InlineStyle + BlockAttrs）
- [ ] 定义 **Inline[] 结构**（与 MCP block content.text[] 对齐：text/bold/italic/strike/strikeThrough/underline/color/backgroundColor）
- [ ] 实现 `mdx/parser.rs`：frontmatter + block components + inline `<Mark>`/`<Link>`
- [ ] 实现 `mdx/emitter.rs`：三段式输出 + 缩进规则（遵循 AI Ingest Spec）
- [ ] **实现 `adapter.rs::ir_to_descendant()`**：DocIR → 完整 descendant JSON 结构，**直接兼容 `block_create_block_descendant`**
- [ ] 实现 `adapter.rs::block_to_ir()`：MCP 返回的 Block JSON → DocIR
- [ ] 单元测试：
  - 每个组件的 `parse_mdx → ir_to_descendant` 输出验证（对比 MCP 真实返回格式）
  - `parse_mdx → emit_mdx` round-trip 一致性
  - `block_to_ir → emit_mdx → parse_mdx → ir_to_descendant` 全链路

### Phase 2: CLI 命令集成

**文件变更**：

```text
crates/lx/src/cmd/block/
├── mod.rs              # 新增 import-mdx / convert 子命令
└── mdx_handler.rs      # MDX 相关命令处理逻辑
```

**任务清单**：

- [ ] `lx block import-mdx` 命令
- [ ] `lx block export --format mdx` 支持
- [ ] `lx block convert --content-type mdx` 支持
- [ ] `lx block validate-mdx` 校验命令

### Phase 3: Skill Reference 升级

**文件变更**：

```text
skills/lx-block/references/
└── mdx-reference.md     # 新增：MDX 语义参考文档
```

**任务清单**：

- [ ] 编写组件清单与示例
- [ ] 编写组件能力对照表（支持 / 不支持 / Phase 2 计划）
- [ ] 编写场景模板（至少 3 个典型场景）
- [ ] 更新 SKILL.md 引用新 reference

### Phase 4: 清理旧 markdown 链路（可选）

**不做的事**：

- ❌ 删除 `converter.rs` 中现有的 markdown 函数
- ❌ 修改 `cmd/git/mod.rs` 的 `blocks_to_markdown`（git 工作流保持不变）

**做的事**：

- ✅ 标记 `markdown_to_blocks` 为 `#[deprecated]`（仅建议，不强求）
- ✅ 在 skill 文档中引导新流程优先使用 MDX

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| MDX Parser 复杂度失控 | 开发周期长 | 先实现 80% 常用组件，其余 warning + 跳过 |
| **本地构造的 Block JSON 与后端期望不一致** | 插入失败或数据损坏 | 严格对齐 MCP schema 中 `CreateBlockDescendant` 的 descendant 结构；用现有 MCP 返回的真实数据做反向验证测试 |
| 后端 block 结构变化导致适配失效 | round-trip 不一致 | adapter 层做版本化 + 兼容层；未知字段保留在 raw content |
| AI 生成的 MDX 不符合规范 | 解析失败 | `validate-mdx` 命令前置校验；parser 容错模式（跳过无法解析的部分） |
| 性能（大文档解析慢） | CLI 卡顿 | 流式解析 + chunked processing（复用现有 chunk-size 机制） |
| **Inline[] 结构与后端不完全兼容** | 样式丢失 | 先支持 text/bold/italic/strike 这 4 个核心属性；颜色等高级属性 Phase 2 再补 |

## Open Questions

1. **MDX 文件扩展名**：`.mdx` 还是 `.md`？（建议 `.mdx`，明确区分）
2. **是否需要 schema 校验**：JSON Schema 定义 DocIR 结构？建议 Phase 1 先不做，Phase 2 视需求补
3. **未知组件处理**：遇到不支持的 MDX 组件时直接 warning 并跳过，还是严格报错拒绝？
4. **`lx sh` 虚拟 shell 是否需要内置 MDX 命令**：还是只在 `lx block` 子命令中提供？

## Success Criteria

1. AI 可以产出包含 `Callout`、`ColumnList`、`Table`、`Todo` 的 MDX 文件
2. **`lx block import-mdx` 在 CLI 本地完成 MDX → Block JSON 转换，不经过后端 `block_convert_content_to_blocks`**
3. 本地构造的 Block JSON 能直接传给 `block_create_block_descendant` 成功插入
4. `lx block export --format mdx` 导出的 MDX 能被重新导入，内容一致（round-trip）
5. Skill reference 包含足够的示例和模板，AI 能独立完成常见场景
6. 所有现有功能不受影响（git pull/push、旧 markdown import 照常工作）
