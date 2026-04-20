---
name: lx-comment
description: |
  乐享页面评论管理。当用户需要查看文档评论时使用。
  触发词：评论、comment、查看评论、文档评论
metadata:
  {"openclaw": {"requires": {"bins": ["lx"]}, "always": false}}
---

# 评论管理

> **前置条件：** 需要 `lx` CLI 已配置并登录。

---

## 使用场景

### 场景一：查看页面评论列表

**触发条件：**

- 用户说"查看这个页面的评论"/"文档有哪些评论"

**使用工具：** `lx-comment-list-comments`

**SOP：**

1. 获取 entry_id（通过搜索或浏览）
2. 调用 `lx-comment-list-comments { "target_type": "kb_entry", "target_id": "entry_xxx" }`
3. 返回评论列表，包含评论 ID、作者、时间等信息

**特殊情况处理：**

- 评论为空 → 提示该页面暂无评论
- 需要分页 → 使用 `page_token` 参数翻页

---

### 场景二：查看评论详情

**触发条件：**

- 用户说"看看这条评论的详细内容"

**使用工具：** `lx-comment-describe-comment`

**SOP：**

1. 获取 comment_id（从列表中获取）
2. 调用 `lx-comment-describe-comment { "comment_id": "comment_xxx" }`

**特殊情况处理：**

- 评论内容格式特殊 → 返回的 `content` 不是普通 HTML，需要按实际格式解析

---

## 工具速查

| 工具名 | 用途 |
|--------|------|
| `lx-comment-list-comments` | 获取页面评论列表 |
| `lx-comment-describe-comment` | 获取评论详情 |

---

## 典型组合流程

### 查看页面评论

```json
// 1. 获取评论列表
lx-comment-list-comments: { "target_type": "kb_entry", "target_id": "entry_xxx" }

// 2. 查看评论详情
lx-comment-describe-comment: { "comment_id": "comment_xxx" }
```
