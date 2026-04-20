---
name: lx-comment
version: 1.0.0
description: "乐享页面评论管理。当用户需要查看文档评论时使用。触发词：评论、comment、查看评论、文档评论"
metadata:
  requires:
    bins: ["lx"]
---

# 评论管理

> **前置条件：** 需要 `lx` CLI 已配置并登录。

## ⚡ 什么时候用这个 skill？

**进入场景：**

- 用户说"查看这个页面的评论"
- 用户说"文档有哪些评论"

**禁止在本 skill 中执行：**

- **不要编辑页面内容**：用户说"编辑某个页面内容" → **立即切换到 lx-block skill**
- **不要创建页面**：用户说"在知识库里创建页面" → **立即切换到 lx-entry skill**

## ⚡ 怎么选命令？（决策树）

```text
识别场景 →
└── 查看/管理页面评论?
    └── lx comment list-comments / lx comment describe-comment
```

## 可用工具

| 命令 | 说明 | 参考 |
|------|------|------|
| `lx comment list-comments` | 获取页面评论列表 | [comment.md](references/comment.md) |
| `lx comment describe-comment` | 获取评论详情 | [comment.md](references/comment.md) |

## 🎯 执行规则

1. **评论内容特殊格式**：`lx comment describe-comment` 返回的 `content` 不是普通 HTML，需要特殊解析。

## 典型组合流程

### 查看页面评论

```bash
# 获取评论列表
lx comment list-comments --target-type kb_entry --target-id entry_xxx

# 查看评论详情
lx comment describe-comment --comment-id comment_xxx
```
