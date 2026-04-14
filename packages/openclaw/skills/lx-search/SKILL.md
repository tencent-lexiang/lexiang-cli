# lx-search Skill

---

name: lx-search
description: |
  在乐享知识库中搜索内容，支持关键词搜索和语义搜索

---

**当以下情况时使用此 Skill**:

- 需要在乐享中搜索文档或内容
- 需要按关键词查找相关文档
- 需要模糊查询或语义匹配
- 用户提到"搜索"、"找一下"、"有没有关于..."

## 工具一：lx-search（关键词搜索）

精确匹配关键词

### 参数（关键词搜索）

- **keyword** (string, required): 搜索关键词
- **type** (string): 搜索类型：all / doc / space / folder / file / page 等
- **space_id** (string): 限定知识库
- **team_id** (string): 限定团队
- **sort_by** (string): 排序：created_at / -created_at / edited_at / -edited_at
- **title_only** (boolean): 只搜索标题
- **limit** (number): 结果数量
- **page_token** (string): 翻页 token

### 示例（关键词搜索）

```json
lx-search: { "keyword": "项目计划" }
```

## 工具二：lx-embedding-search（语义搜索）

基于向量的语义相似度匹配，适用于模糊查询

### 参数（语义搜索）

- **keyword** (string, required): 搜索语句
- **space_id** (string): 限定知识库
- **team_id** (string): 限定团队
- **parent_id** (string): 限定父节点
- **limit** (number): 结果数量

### 示例（语义搜索）

```json
lx-embedding-search: { "keyword": "如何申请服务器资源" }
```

## 选择建议

| 场景               | 推荐工具            |
|--------------------|---------------------|
| 精确查找已知关键词 | lx-search           |
| 模糊查询、语义匹配 | lx-embedding-search |
