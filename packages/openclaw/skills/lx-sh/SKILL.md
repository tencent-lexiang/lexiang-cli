---
name: lx-sh
description: |
  虚拟 Shell 引擎，用 UNIX 命令浏览和探索乐享知识库。支持 ls/cat/grep/find/tree 等命令、管道、重定向、变量替换。
  触发词：shell、sh、浏览知识库、grep、ls、cat、搜索文件、虚拟shell
metadata:
  {"openclaw": {"requires": {"bins": ["lx"]}, "always": false}}
---

# 虚拟 Shell

> **前置条件：** 需要 `lx` CLI 已配置并登录。

---

## 使用场景

### 场景一：交互式浏览知识库

**触发条件：**

- 用户说"浏览知识库"/"看看知识库结构"

**使用工具：** `lx-sh`

**SOP：**

1. 启动 Shell → `lx-sh { "space_id": "sp_xxx" }`（MCP 远程模式）
2. 在 Shell 中使用 `ls`、`cat`、`tree` 等命令浏览
3. 或使用单次执行模式 → `lx-sh { "exec": "ls -la /kb", "space_id": "sp_xxx" }`

**特殊情况处理：**

- 已有本地 worktree → 在 worktree 目录中启动 `lx-sh`，`/kb` 映射到本地磁盘
- 不想 clone → 使用 `--space` 参数指定远程知识库

---

### 场景二：在知识库中搜索内容

**触发条件：**

- 用户说"搜索文件内容"/"grep 知识库"

**使用工具：** `lx-sh`

**SOP：**

1. 单次执行 → `lx-sh { "exec": "grep -r '关键词' /kb | head -10", "space_id": "sp_xxx" }`
2. 或使用内置 search → `lx-sh { "exec": "search 关键词", "space_id": "sp_xxx" }`

---

### 场景三：查看文档内容

**触发条件：**

- 用户说"查看 XX 文档内容"

**使用工具：** `lx-sh`

**SOP：**

1. 调用 `lx-sh { "exec": "cat /kb/项目文档/README.md", "space_id": "sp_xxx" }`

---

### 场景四：分析文档结构

**触发条件：**

- 用户说"统计文档数量"/"看看知识库有多大"

**使用工具：** `lx-sh`

**SOP：**

1. 统计文档数 → `lx-sh { "exec": "find /kb -name '*.md' | wc -l", "space_id": "sp_xxx" }`
2. 查看目录树 → `lx-sh { "exec": "tree /kb --depth 2", "space_id": "sp_xxx" }`

---

## 工具参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `exec` | string | 否 | 要执行的命令 |
| `space_id` | string | 否 | MCP 远程模式，指定知识库 |
| `path` | string | 否 | 指定 worktree 路径 |

---

## Shell 内置命令

| 分类 | 命令 |
|------|------|
| 文件浏览 | `ls`, `cat`, `tree`, `stat` |
| 搜索 | `grep`, `find`, `fzf` |
| 文本处理 | `head`, `tail`, `wc`, `sort`, `uniq`, `awk`, `cut`, `tr` |
| 导航 | `cd`, `pwd`, `echo` |
| 桥接 | `search`, `mcp`, `git` |

---

## 执行规则

1. **只读文件系统**：`/kb` 是只读的，`rm`/`mv`/`cp`/`mkdir`/`touch` 等写命令会被拦截
2. **管道和重定向**：完整支持 `|` 管道和 `>` 重定向
3. **别名系统**：`rg` → `grep -rn`、`eza` → `ls`、`fd` → `find`、`bat` → `cat`
4. **`/tmp` 可写**：可以写入临时数据
5. **非交互模式**：优先使用 `exec` 参数单次执行，避免启动 REPL

---

## 典型组合流程

### 浏览远程知识库结构

```json
// 直接连接远程
lx-sh: { "exec": "tree /kb --depth 2", "space_id": "sp_xxx" }

// 查看具体文件
lx-sh: { "exec": "cat /kb/项目文档/README.md", "space_id": "sp_xxx" }
```

### 在知识库中搜索内容

```json
// 使用 grep
lx-sh: { "exec": "grep -r 'OAuth' /kb | head -10", "space_id": "sp_xxx" }

// 使用内置 search
lx-sh: { "exec": "search OAuth 认证", "space_id": "sp_xxx" }
```

### 分析文档内容

```json
// 统计行数
lx-sh: { "exec": "wc -l /kb/**/*.md", "space_id": "sp_xxx" }

// 查找包含 TODO 的文件
lx-sh: { "exec": "grep -rl 'TODO' /kb", "space_id": "sp_xxx" }
```
