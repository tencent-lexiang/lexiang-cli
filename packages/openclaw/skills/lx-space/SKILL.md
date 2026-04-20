---
name: lx-space
description: |
  乐享知识库与团队管理。当用户需要查看、管理知识库（空间）或团队信息时使用。
  触发词：知识库、空间、团队、space、team
metadata:
  {"openclaw": {"requires": {"bins": ["lx"]}, "always": false}}
---

# 知识库与团队管理

> **前置条件：** 需要 `lx` CLI 已配置并登录。

---

## 使用场景

### 场景一：查看我的团队

**触发条件：**

- 用户说"我的团队有哪些"/"我在哪些团队"

**使用工具：** `lx-team-list-teams` / `lx-team-list-frequent-teams`

**SOP：**

1. 调用 `lx-team-list-teams` 获取所有可访问团队
2. 或调用 `lx-team-list-frequent-teams` 获取常用团队

---

### 场景二：查看团队下的知识库

**触发条件：**

- 用户说"XX 团队有哪些知识库"

**使用工具：** `lx-space-list-spaces`

**SOP：**

1. 获取 team_id（通过 `lx-team-list-teams`）
2. 调用 `lx-space-list-spaces { "team_id": "team_xxx" }`

---

### 场景三：快速定位最近使用的知识库

**触发条件：**

- 用户说"最近的知识库"/"最近访问的空间"

**使用工具：** `lx-space-list-recently-spaces`

**SOP：**

1. 直接调用 `lx-space-list-recently-spaces`
2. 返回结果按最近访问时间排序

---

### 场景四：获取知识库详情

**触发条件：**

- 用户说"XX 知识库的详情"/"获取知识库根节点"

**使用工具：** `lx-space-describe-space`

**SOP：**

1. 获取 space_id
2. 调用 `lx-space-describe-space { "space_id": "sp_xxx" }`
3. 从返回结果中获取 `root_entry_id`（后续操作条目必需）

**特殊情况处理：**

- 不知道 space_id → 先用 `lx-space-list-recently-spaces` 或 `lx-space-list-spaces` 获取

---

## 工具速查

| 工具名 | 用途 |
|--------|------|
| `lx-team-list-teams` | 列出所有团队 |
| `lx-team-list-frequent-teams` | 列出常用团队 |
| `lx-team-describe-team` | 获取团队详情 |
| `lx-space-list-spaces` | 列出团队下的知识库 |
| `lx-space-describe-space` | 获取知识库详情（含 root_entry_id） |
| `lx-space-list-recently-spaces` | 获取最近访问的知识库 |

---

## 典型组合流程

### 从团队到知识库到文档

```json
// 1. 获取团队列表
lx-team-list-teams: {}

// 2. 获取该团队下的知识库
lx-space-list-spaces: { "team_id": "team_xxx" }

// 3. 获取知识库 root_entry_id
lx-space-describe-space: { "space_id": "sp_xxx" }

// 4. 遍历文档目录树（切换到 lx-entry skill）
lx-entry-list-children: { "parent_id": "root_xxx" }
```

### 快速定位最近使用的知识库

```json
// 1. 获取最近访问的知识库
lx-space-list-recently-spaces: {}

// 2. 获取详情和 root_entry_id
lx-space-describe-space: { "space_id": "sp_xxx" }
```
