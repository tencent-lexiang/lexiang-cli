---
name: lx-search
description: |
  乐享知识库搜索与人员查询。支持关键词精确搜索、语义向量搜索和企业员工信息查询。
  当用户需要搜索文档、查找内容或查询同事信息时使用。
  触发词：搜索、找一下、有没有关于、查找、search、查人、谁是
metadata:
  {"openclaw": {"requires": {"bins": ["lx"]}, "always": false}}
---

# 搜索与人员查询

> **前置条件：** 需要 `lx` CLI 已配置并登录（`lx whoami` 可正常返回）。

---

## 使用场景

### 场景一：关键词精确搜索

**触发条件：**

- 用户给出明确的关键词（如产品名、文档标题、特定术语）
- 用户说"搜索 XX"/"找一下 XX 文档"/"有没有关于 XX 的文档"

**使用工具：** `lx-search`

**SOP：**

1. 提取用户给出的关键词
2. 如用户指定了知识库，先调用 `lx space list` 获取 `space_id`
3. 调用 `lx-search` 传入 `keyword` 和可选的 `space_id`
4. 返回结果中的 `entry_id` 可用于后续获取详细内容

**特殊情况处理：**

- 用户说"标题是 XX" → 添加 `title_only: true` 参数
- 搜索结果过多 → 建议用户添加更精确的关键词或限定知识库
- 无结果 → 建议尝试语义搜索（场景二）

---

### 场景二：语义/向量搜索

**触发条件：**

- 用户描述模糊意图（如"怎么申请资源"、"如何配置数据库"）
- 用户用自然语言提问，没有明确关键词
- 关键词搜索无结果

**使用工具：** `lx-embedding-search`

**SOP：**

1. 将用户的自然语言描述作为 `keyword` 传入
2. 如用户指定了知识库，传入 `space_id`
3. 调用 `lx-embedding-search`
4. 返回结果按语义相似度排序

**特殊情况处理：**

- 语义搜索结果相关性低 → 建议用户换一种描述方式
- 需要获取文档详细内容 → 使用返回的 `entry_id` 调用 `lx-entry` 相关工具

---

### 场景三：企业人员查询

**触发条件：**

- 用户说"查一下 XX 同事"/"谁是 XX"/"找一下 XX 的联系方式"

**使用工具：** `lx-search-staff`（通过 schema 动态注册）或 `lx contact search-staff`

**SOP：**

1. 提取人员姓名或关键词
2. 调用人员搜索工具
3. 返回人员基本信息（姓名、部门、职位等）

**特殊情况处理：**

- 查询失败/无结果 → 告知用户可能是企业通讯录权限限制
- 重名情况 → 列出多个匹配结果让用户确认

---

### 场景四：确认当前身份

**触发条件：**

- 用户问"我是谁"/"当前登录的是谁"
- 需要验证 lx CLI 是否已正确配置

**使用工具：** `lx-whoami`

**SOP：**

1. 直接调用 `lx-whoami`
2. 返回当前登录用户的名称、部门等信息

---

## 搜索后操作指引

### 重要：搜索不是终点

搜索返回的 `entry_id` 只是定位信息，如需继续操作：

| 后续操作 | 切换 Skill |
|---------|-----------|
| 获取文档内容 | `lx-entry` |
| 编辑文档 | `lx-entry` 或 `lx-block` |
| 创建新页面 | `lx-entry` |
| 管理文档块 | `lx-block` |

---

## 参数速查

### lx-search（关键词搜索）

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| keyword | string | 是 | 搜索关键词 |
| type | string | 否 | 类型：all/doc/space/folder/file/page |
| space_id | string | 否 | 限定知识库 |
| title_only | boolean | 否 | 仅搜索标题 |
| limit | number | 否 | 结果数量限制 |

### lx-embedding-search（语义搜索）

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| keyword | string | 是 | 搜索语句 |
| space_id | string | 否 | 限定知识库 |
| limit | number | 否 | 结果数量限制 |

---

## 典型组合流程

### 在指定知识库中搜索

```bash
# 获取知识库列表，确认 space_id
lx space list

# 在指定知识库中搜索
lx search kb-search --keyword "API 设计" --space-id sp_xxx
```

### 语义搜索 + 获取文档内容

```bash
# 语义搜索获取 entry_id
lx search kb-embedding-search --keyword "如何配置数据库连接池"

# 获取文档内容（切换到 lx-entry skill）
lx entry describe-ai-parse-content --entry-id entry_xxx
```

### 查找同事的相关文档

```bash
# 查找人员信息
lx contact search-staff --staff-id "张三"

# 按创建人搜索其文档
lx search kb-search --keyword "项目总结" --type kb_doc
```
