---
name: lx-git
description: |
  Git 风格的知识库本地工作流。支持克隆知识库到本地目录，用 git-like 命令管理版本（add/commit/push/pull/diff/log/reset/revert），以及多 worktree 管理。
  触发词：克隆、clone、push、pull、同步、推送、拉取、版本、提交、commit、worktree、工作区
metadata:
  {"openclaw": {"requires": {"bins": ["lx"]}, "always": false}}
---

# Git 风格知识库工作流

> **前置条件：** 需要 `lx` CLI 已配置并登录。

---

## 使用场景

### 场景一：克隆知识库到本地

**触发条件：**

- 用户说"克隆知识库"/"把知识库拉到本地"

**使用工具：** `lx-git-clone`

**SOP：**

1. 获取 space_id（通过 `lx-space-list-spaces` 或 `lx-space-list-recently-spaces`）
2. 调用 `lx-git-clone { "space_id": "sp_xxx", "path": "./my-kb" }`
3. 进入克隆的目录进行后续操作

---

### 场景二：查看本地修改

**触发条件：**

- 用户说"看看改了什么"/"本地有哪些变更"

**使用工具：** `lx-git-status` / `lx-git-diff`

**SOP：**

1. 在 worktree 目录中调用
2. `lx-git-status` 查看变更文件列表
3. `lx-git-diff` 查看具体变更内容

---

### 场景三：提交本地变更

**触发条件：**

- 用户说"提交修改"/"保存本地变更"

**使用工具：** `lx-git-add` / `lx-git-commit`

**SOP：**

1. `lx-git-add { "path": "." }` 暂存所有变更
2. `lx-git-commit { "message": "更新内容" }` 提交

---

### 场景四：推送本地变更到远程

**触发条件：**

- 用户说"推送"/"push"/"同步到远程"

**使用工具：** `lx-git-push`

**SOP：**

1. 确保所有变更已提交（`lx-git-commit`）
2. 调用 `lx-git-push`
3. 如需强制推送 → `lx-git-push { "force": true }`（需谨慎）

**特殊情况处理：**

- 推送前建议先 `dry-run` → 部分实现支持预览
- 有冲突 → 先 `lx-git-pull` 拉取远程更新

---

### 场景五：拉取远程更新

**触发条件：**

- 用户说"拉取更新"/"pull"/"同步远程"

**使用工具：** `lx-git-pull`

**SOP：**

1. 在 worktree 目录中调用
2. `lx-git-pull` 拉取远程最新内容

---

### 场景六：查看提交历史

**触发条件：**

- 用户说"查看历史"/"提交记录"

**使用工具：** `lx-git-log`

**SOP：**

1. 调用 `lx-git-log`
2. 可指定数量限制 → `lx-git-log { "limit": 10 }`

---

### 场景七：版本回退

**触发条件：**

- 用户说"回退版本"/"撤销修改"

**使用工具：** `lx-git-reset` / `lx-git-revert`

**SOP：**

- 本地回退（未推送）→ `lx-git-reset { "commit": "abc123" }`
- 远程回退（已推送）→ `lx-git-revert { "commit": "abc123" }`（危险操作，需谨慎）

**特殊情况处理：**

- 远程回退前建议先用 `--dry-run` 预览
- 确认用户意图后再执行

---

### 场景八：多 worktree 管理

**触发条件：**

- 用户说"创建新工作区"/"管理多个 worktree"

**使用工具：** `lx-worktree-add` / `lx-worktree-list` / `lx-worktree-remove`

**SOP：**

1. `lx-worktree-add { "space_id": "sp_xxx", "path": "./worktree2" }` 创建新 worktree
2. `lx-worktree-list` 查看所有 worktree
3. `lx-worktree-remove { "path": "./worktree2" }` 删除 worktree

---

## 工具速查

| 工具名 | 用途 |
|--------|------|
| `lx-git-clone` | 克隆远程知识库 |
| `lx-git-add` | 暂存文件 |
| `lx-git-commit` | 提交本地变更 |
| `lx-git-status` | 查看工作树状态 |
| `lx-git-diff` | 查看变更差异 |
| `lx-git-log` | 查看提交历史 |
| `lx-git-pull` | 拉取远程更新 |
| `lx-git-push` | 推送到远程 |
| `lx-git-reset` | 重置本地 HEAD |
| `lx-git-revert` | 回退远程版本 |
| `lx-worktree-add` | 创建 worktree |
| `lx-worktree-list` | 列出 worktree |
| `lx-worktree-remove` | 删除 worktree |

---

## 典型组合流程

### 首次克隆并编辑

```json
// 1. 克隆知识库
lx-git-clone: { "space_id": "sp_xxx", "path": "./my-kb" }

// 2. 查看本地文件（在 worktree 目录中）
// ... 编辑文件 ...

// 3. 查看变更
lx-git-status: {}
lx-git-diff: {}

// 4. 暂存 + 提交 + 推送
lx-git-add: { "path": "." }
lx-git-commit: { "message": "更新了项目计划" }
lx-git-push: {}
```

### 拉取远程更新

```json
lx-git-pull: {}
```

### 回退远程版本

```json
// 1. 查看历史
lx-git-log: { "limit": 10 }

// 2. 回退（危险操作，谨慎执行）
lx-git-revert: { "commit": "abc1234" }
```
