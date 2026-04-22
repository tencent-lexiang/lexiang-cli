# MDX Block 数据格式参考

> **前置条件：** 先阅读 [`../SKILL.md`](../SKILL.md) 了解 block 操作的整体决策树。
>
> 本文档与 **Notion Enhanced Markdown Specification** 对齐。

## 概述

MDX（Markdown + JSX）是 AI 生成结构化内容的**推荐输入格式**。CLI 本地解析 MDX → 构造与后端完全兼容的 Block JSON → 调用 MCP 插入。

### 核心流程

```text
AI 产出 .mdx 文件 → CLI parse_mdx() → DocIR → ir_to_descendant()
→ 完整 Block JSON（与 MCP schema 一致）→ block_create_block_descendant() 直接插入
```

### 格式约定

- **所有组件标签小写**：`<callout>`、`<columns>`、`<column>`、`<details>`、`<table>`
- **属性双引号**：`<callout icon="🚧" color="red">`
- **块级颜色后缀**：`{color="blue"}` 附加在任意块末尾

---

## 支持的 Block 类型（完整清单）

以下所有类型均来自后端 `block_create_block_descendant` 的 `block_type` 枚举，全部支持。

### 文本块（叶子节点，不支持 children）

这些块的直接子块不能包含其他块。

| block_type | MDX 语法 | content 结构 | 说明 |
|-----------|----------|-------------|------|
| `p` | 普通文本 | `{ elements: [TextElement[]] }` | 默认段落 |
| `h1` ~ `h5` | `#` ~ `#####` | `{ elements: [TextElement[]] }` | 1-5级标题 |
| `code` | \`\`\`language\ncode\n\`\`\` | `{ language, text: "..." }` | 代码块 |
| `divider` | `---` | `{}` | 分割线 |
| `image` | `![alt](url)` | `{ file_id?, caption?, width?, height?, align? }` | 图片 |
| `quote` | `> text {color}` | `{ text: { elements[] } }` | 引用块 |

### 容器块（支持 children 嵌套子块）

| block_type | MDX 语法 | content 结构 | 说明 |
|-----------|----------|-------------|------|
| `callout` | `<callout icon? color?>...children...</callout>` | `{ color?, icon?, callout: true }` | 高亮提示框，内容在子块中 |
| `toggle` | `<details color?><summary>text</summary>\nchildren\n</details>` | `{}` | 折叠块 |
| `task` | `- [x] 任务名` 或 `<task checked name/>` | `{ done, name? }` | 任务（含完成状态） |
| `bulleted_list` | `- item` | `{ text: { elements[] } }` | 无序列表 |
| `numbered_list` | `1. item` | `{ text: { elements[] } }` | 有序列表 |
| `column_list` | `<columns>\n<column>...</column>\n</columns>` | `{}` | 分栏容器 |
| `column` | `<column>...children...</column>` | `{ width_ratio? }` | 分栏列 |
| `table` | GFM 表格语法或 XML `<table>/<tr>/<td>` | `{ column_size?, row_size? }` | 表格容器 |
| `table_cell` | 表格单元格内容 | `{ align?, background_color?, col_span?, row_span? }` | 表格单元格 |
| `mermaid` | `<mermaid>\ncode\n</mermaid>` | `{ content: "..." }` | Mermaid 图表 |
| `plantuml` | `<plantuml>\ncode\n</plantuml>` | `{ content: "..." }` | PlantUml 图表 |

---

## 详细组件说明

### Callout（高亮提示框）

**关键：Callout 是容器类型，实际内容存储在 `children` 子块中，不是内联文本。**

```mdx
<callout icon="🚧" color="red">
## 注意事项

这是 Callout 内部的一个标题段落。

- 可以包含列表
- 也可以包含代码块

```rust
fn example() {}
```

</callout>
```

| 属性 | 类型 | 说明 |
|------|------|------|
| `icon` | String | Emoji 图标 |
| `color` | String | Notion 颜色名（`red`, `blue`, `green` 等）或背景色（`red_bg` 等）|

### Toggle（折叠块）

Notion 格式使用 HTML `<details>` + `<summary>`：

```mdx
<details color="blue">
<summary>点击展开查看详情</summary>

这里的内容默认折叠，用户点击后展开显示。
</details>
```

| 属性 | 类型 | 说明 |
|------|------|------|
| `color` | String | 可选的 Notion 颜色名 |

### Quote（引用块）

原生 Markdown 引用语法，支持块级颜色：

```mdx
> 这是一个引用消息 {color="blue"}

> 多行引用
> 第二行
```

### Task（任务）

推荐使用 GFM 语法：

```mdx
- [x] 完成接口开发
- [ ] 编写单元测试
- [ ] 代码审查
```

或 MDX 组件形式（仅流级）：

```mdx
<task checked={true} name="完成接口开发"/>
```

| 属性 | 类型 | 必填 | 说明 |
|------|------|:----:|------|
| `name` | String | 否 | 任务名称 |
| `done` | Boolean | 是 | 是否完成（`true` / `1`）|

### ColumnList / Column（分栏布局）

**Column 和 Callout 一样是容器类型**，分栏内容存在 children 子块中。

```mdx
<columns>
<column>
### 左侧

左侧内容。
</column>
<column>
### 右侧

右侧内容。
</column>
</columns>
```

### Table（表格）

支持两种格式：

**GFM 语法（自动解析为 table/table_row/table_cell）：**

```mdx
| 姓名 | 部门 | 状态 |
| ---- | ---- | ---- |
| 张三 | 开发 | 进行中 |
| 李四 | 产品 | 已完成 |
```

**XML 格式（Notion 对齐）：**

```mdx
<table>
<tr>
<td>A</td>
<td>B</td>
</tr>
</table>
```

### Mermaid / PlantUML 图表

**一等公民类型，有独立的 content 字段存储图表代码。**

```mdx
<mermaid>
graph LR
    A --> B --> C
</mermaid>

<plantuml>
@startuml
Alice -> Bob: Hello
@enduml
</plantuml>
```

---

## 块级颜色（Block Color）

**所有块都支持 Notion 颜色属性**，以 `{color="Color"}` 后缀形式追加：

```mdx
# 标题 {color="red"}

这段文字有蓝色背景 {color="blue_bg"}

> 引用文字 {color="gray"}
```

**有效颜色值：**

| 文字色 | 背景色 |
|--------|--------|
| `default` | `default_background` |
| `gray`, `brown`, `orange`, `yellow`, `green`, `blue`, `purple`, `pink`, `red` | `gray_bg`, `brown_bg`, `orange_bg`, `yellow_bg`, `green_bg`, `blue_bg`, `purple_bg`, `pink_bg`, `red_bg` |

也可使用十六进制：`{color="#FF5500"}`

---

## 行内样式（TextElement / TextStyle）

所有文本块（p, h1-h5, bulleted_list, numbered_list 等）的内容都遵循此结构：

```typescript
// 行内元素
interface TextElement {
  text_run?: {
    content: string;
    text_style?: TextStyle;
  };
  mention_staff?: { staff_id: string };    // @人
  mention_entry?: { entry_id: string };     // #文档
  mention_date?: { date: string; time?: string }; // 日期
}

interface TextStyle {
  bold?: boolean;
  italic?: boolean;
  strikethrough?: boolean;
  underline?: boolean;
  link?: string;
  text_color?: string;
  background_color?: string;
  inline_code?: boolean;
}
```

### 行内样式 MDX 语法

| 样式 | MDX 语法 | 输出 |
|------|---------|------|
| 加粗 | `**text**` 或 `<Mark bold>text</Mark>` | `text_style.bold = true` |
| 斜体 | `*text*` 或 `<Mark italic>text</Mark>` | `text_style.italic = true` |
| 删除线 | `~~text~~` | `text_style.strikethrough = true` |
| 下划线 | `<span underline="true">text</span>` | `text_style.underline = true` |
| 文字颜色 | `<span color="red">text</span>` | `text_style.text_color = "red"` |
| 链接 | `[text](url)` | `text_style.link = url` |
| 行内代码 | `` `code` `` | `text_style.inline_code = true` |

---

## 嵌套规则总结

```text
Document (根)
├── p, h1-h5, divider, image, code, quote          ← 叶子节点
├── bulleted_list, numbered_list, task              ← 叶子节点
├── callout                                        ← 容器：任意块
│   ├── p, h1-h5, code, list, task, toggle, table...
├── columns (column_list)                           ← 容器：仅 column
│   └── column                                     ← 容器：任意块
│       ├── p, h1-h5, code, list, task, callout...
├── details (toggle)                                ← 容器：任意块
│   ├── summary + children blocks
└── table                                          ← 容器：仅 table_row
    └── table_row                                   ← 容器：仅 table_cell
        └── table_cell                              ← 容器：任意块
            └── p, h1-h5, code, list...
```

**叶子节点不支持 children：**
h1-h5, code, image, divider, quote, bulleted_list, numbered_list, task, mermaid, plantuml

---

## 完整示例

```mdx
# 产品概述

本版本重点改进**性能**和*用户体验*。

<callout icon="💡" color="blue">
## 设计原则

1. 用户优先
2. 向后兼容
3. 渐进式增强
</callout>

<details>
<summary>核心功能</summary>

- [x] Callout 组件重构
- [x] 性能优化（首屏 < 1s）
- [ ] 国际化支持
</details>

> 重要提示：请仔细阅读以下计划 {color="orange"}

<!-- markdownlint-disable MD033 -->
<columns>
<column>
### 开发计划

```typescript
interface User {
  id: string;
  name: string;
}
```

</column>
<column>
### 时间线

| 阶段 | 时间 | 负责人 |
| ---- | ---- | ---- |
| Design | W1-W2 | Alice |
| Dev | W3-W4 | Bob |

<mermaid>
gantt
    dateFormat YYYY-MM-DD
    title 开发计划
    section 设计
    UI设计 :a1, 2025-04-01, 7d
    section 开发
    前端开发 :a2, after a1, 14d
</mermaid>
</column>
</columns>
<!-- markdownlint-enable MD033 -->
```

---

## 组件能力完整对照表

| 组件 / 特性 | MDX 语法 | 支持 |
|------------|----------|:----:|
| **文本块** |||
| Paragraph (`p`) | 普通文本 | ✅ |
| Heading (`h1`~`h5`) | `#` ~ `#####` | ✅ |
| Code (`code`) | \`\`\`language\ncode\n\`\`\` | ✅ |
| Divider (`divider`) | `---` | ✅ |
| Image (`image`) | `![alt](url)` | ✅ |
| Quote (`quote`) | `> text {color}` | ✅ |
| **结构化块** |||
| **Callout** | `<callout icon color>...</callout>` | ✅ |
| **Toggle** | `<details><summary>...</summary></details>` | ✅ |
| **Task** | `- [x] name` or `<task checked name/>` | ✅ |
| BulletedList | `- item` | ✅ |
| NumberedList | `1. item` | ✅ |
| **布局块** |||
| **ColumnList** | `<columns><column>...</column></columns>` | ✅ |
| **Column** | `<column>...</column>` | ✅ |
| **Table** | GFM `\|-\|` or `<table>/<tr>/<td>` | ✅ |
| **TableCell** | 单元格内容（可带 `<td color>`） | ✅ |
| **图表** |||
| **Mermaid** | `<mermaid>code</mermaid>` | ✅ |
| **PlantUml** | `<plantuml>code</plantuml>` | ✅ |
| **行内样式** |||
| Bold | `**text**` | ✅ |
| Italic | `*text*` | ✅ |
| Strikethrough | `~~text~~` | ✅ |
| Underline | `<span underline="true">text</span>` | ✅ |
| Link | `[text](url)` | ✅ |
| Inline Code | `` `code` `` | ✅ |
| Text Color | `<span color="red">text</span>` | ✅ |
| **@mention (@人)** | （通过 mention_staff） | ✅ |
| **#mention (#文档)** | （通过 mention_entry） | ✅ |
| **块级样式** |||
| Block Color | `{color="Color"}` 后缀 | ✅ |

---

## 格式规则速查

1. **所有组件标签小写**：`<callout>`、`<columns>`、`<column>`、`<details>`、`<table>`、`<mermaid>`、`<plantuml>`
2. **属性用双引号**：`<callout icon="🚧" color="red">`
3. **Callout/Column 是容器** — 内容写在内部子块中，不是内联文本
4. **Task 用 GFM `- [x]` 语法更简洁**
5. **Toggle 用 `<details><summary>` HTML 语义标签**
6. **表格必须带 header 分隔行**
7. **Mermaid/PlantUml 使用独立小写标签**：`<mermaid>`, `<plantuml>`
8. **块级颜色统一用 `{color="Color"}` 后缀**，不是组件属性

---

## 参考

- [lx-block SKILL.md](../SKILL.md) — block 操作决策树
- [block-basic.md](block-basic.md) — 原子命令
- [block-advanced.md](block-advanced.md) — 高级命令
- [Notion Enhanced Markdown Spec](notion://docs/enhanced-markdown-spec) — Notion MCP resource 定义
