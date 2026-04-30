---
name: lx-space
version: 1.1.0
description: "乐享知识库与团队管理。当用户需要查看、管理知识库（空间）或团队信息时使用。触发词：知识库、空间、团队、我的知识库、space、team"
metadata:
  requires:
    bins: ["lx"]
---

# 知识库与团队管理

> **前置条件：** 需要 `lx` CLI 已配置并登录。

## ⚡ 什么时候用这个 skill？

**进入场景：**

- 用户说"我的知识库"/"帮我打开我的知识库" → **直接用 `lx space mine`**
- 用户说"我的知识库有哪些"/"帮我看看 XX 团队的知识库"
- 用户说"找一下 YY 知识库"/"最近的知识库"
- 用户说"这个知识库的根节点是什么"

**禁止在本 skill 中执行：**

- **不要在本 skill 中创建页面**：用户说"创建页面" → **立即切换到 lx-entry skill**
- **不要在本 skill 中编辑页面**：用户说"编辑某个页面" → **立即切换到 lx-block skill**
- **不要在本 skill 中搜索内容**：用户说"搜索知识库内容" → **立即切换到 lx-search skill**

## ⚡ 怎么选命令？（决策树）

```text
识别场景 →
├── "我的知识库"/"帮我打开我的知识库"? → lx space mine（一等公民，最优先）
├── 不知道在哪个团队? → lx team list-teams 或 lx team list-frequent-teams
├── 知道团队，找知识库? → lx space list-spaces --team-id <ID>
├── 知道知识库 ID? → lx space describe-space --space-id <ID>
├── 快速定位最近用的知识库? → lx space list-recently-spaces
└── 需要获取知识库根节点（后续操作条目）? → lx space describe-space → 取 root_entry_id
```

## ⚠️ 高风险操作与默认优先路径

**"我的知识库"是一等公民工具：**

- 用户提到"我的知识库"时，**直接用 `lx space mine`**，这是最快的路径
- 该命令会自动获取（或创建）用户的个人知识库，返回 `space_id` 和 `root_entry_id`
- 如果返回 `is_creating: true`，说明知识库正在异步创建，需要用 `task_id` 轮询

**获取 root_entry_id 是关键：**

- 后续操作条目（创建页面、浏览目录树）都需要先通过 `lx space describe-space` 拿到 `root_entry_id`
- 这是最重要的前置步骤

**默认优先路径：**

1. 用户说"我的知识库" → 直接用 `lx space mine`
2. 用户说"最近的知识库" → 直接用 `lx space list-recently-spaces`，不要走 team → space 全链路
3. 需要创建页面 → 先获取 `root_entry_id`，再切换到 lx-entry skill

**层级关系：**

- 团队(Team) → 知识库(Space) → 条目(Entry)
- 知识库必须属于某个团队
- 个人知识库也属于一个特殊的个人团队

## 可用工具

| 命令 | 说明 | 参考 |
|------|------|------|
| `lx space mine` | 🌟 获取我的知识库（自动创建） | [space-team.md](references/space-team.md) |
| `lx team list-teams` | 列出用户可访问的所有团队 | [space-team.md](references/space-team.md) |
| `lx team list-frequent-teams` | 获取常用团队（按频率排序） | [space-team.md](references/space-team.md) |
| `lx team describe-team` | 获取团队详情 | [space-team.md](references/space-team.md) |
| `lx space list-spaces` | 列出团队下的知识库 | [space-team.md](references/space-team.md) |
| `lx space describe-space` | 获取知识库详情（含 root_entry_id） | [space-team.md](references/space-team.md) |
| `lx space list-recently-spaces` | 获取最近访问的知识库 | [space-team.md](references/space-team.md) |

## 🎯 执行规则

1. **"我的知识库"优先**：用户提到"我的知识库"时直接用 `lx space mine`，不需要先查团队或列表。
2. **层级关系**：团队(Team) → 知识库(Space) → 条目(Entry)。知识库必须属于某个团队。
3. **获取 root_entry_id 是关键**：后续操作条目（创建页面、浏览目录树）都需要先通过 `lx space describe-space` 拿到 `root_entry_id`。
4. **快速路径优先**：用户说"最近的知识库"时直接用 `lx space list-recently-spaces`，不要走 team → space 全链路。
5. **团队首页链接**：`{domain}/t/{team_id}/spaces`
6. **知识库访问链接**：`{domain}/spaces/{space_id}`

## 典型组合流程

### 🌟 快速进入我的知识库

```bash
# 一键获取个人知识库（自动创建）
lx space mine

# 返回 space_id 和 root_entry_id，可直接后续操作
lx entry list-children --parent-id root_entry_xxx
```

### 从团队到知识库到文档

```bash
# 获取团队列表，让用户选择目标团队
lx team list-teams

# 获取该团队下的知识库列表
lx space list-spaces --team-id team_xxx

# 获取知识库 root_entry_id
lx space describe-space --space-id sp_xxx

# 遍历文档目录树（→ lx-entry skill）
lx entry list-children --parent-id root_entry_xxx
```

### 快速定位最近使用的知识库

```bash
# 获取最近访问的知识库
lx space list-recently-spaces

# 获取详情和 root_entry_id
lx space describe-space --space-id sp_xxx
```
