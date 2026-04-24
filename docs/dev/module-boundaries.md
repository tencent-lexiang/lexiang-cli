# lexiang-cli 模块业务边界说明

> **目的**：明确每个模块的职责范围和边界，避免在错误的层次实现功能。
> **核心原则**：上层可以调用下层，下层不能反向依赖；同层模块之间尽量解耦。

---

## 整体架构（从上到下）

```text
┌─────────────────────────────────────────────────────────┐
│  main.rs — 入口：参数解析、命令分发、日志初始化            │
├──────────┬──────────────┬───────────────────────────────┤
│  cmd/    │  serve/      │  shell/                       │
│ CLI命令层 │ JSON-RPC服务层 │ 虚拟Shell引擎                 │
├──────────┴──────────────┴───────────────────────────────┤
│  mcp/ — MCP 协议客户端（HTTP transport + 协议编解码）     │
├─────────────────────────────────────────────────────────┤
│  service/ — 业务逻辑层（block 操作、MDX 转换等）          │
├──────────┬──────────────┬───────────────────────────────┤
│  auth/   │  config/     │  vfs/  daemon/ worktree/ ...  │
│ 认证     │ 配置管理     │ 基础设施                       │
└─────────────────────────────────────────────────────────┘
```

---

## 1. `main.rs` — 程序入口

### 职责（main.rs）

- 初始化日志系统
- 解析命令行参数，**分发到正确的处理函数**
- 命令优先级：`block 静态命令` > `动态 MCP 命令` > `clap 静态子命令`

### 分发逻辑

```text
1. --help → 显示帮助（含动态命令）
2. lx block <subcmd> → cmd/block/mod.rs（静态实现，优先）
3. lx <namespace> <tool> → cmd/dynamic/mod.rs（动态 MCP 调用）
4. lx < clap 子命令> → Cli::parse() → 各 cmd 模块
5. 无参数 → 默认帮助
```

### 边界（main.rs）

- **只做路由，不做业务逻辑**
- **不直接调用 mcp::McpClient**（通过 cmd 层间接调用）
- **不包含任何 UI 格式化代码**

---

## 2. `cmd/` — CLI 命令层

这是用户通过终端交互的**唯一入口层**。每个子目录对应一类命令。

### 2.1 `cmd/cli.rs` — Clap 命令定义

| 内容 | 说明 |
|------|------|
| `Cli` struct | 顶层命令定义 |
| `Commands` enum | 所有静态子命令（mcp, tools, skill, git, worktree, login, serve, sh...） |
| `McpCommands` | `lx mcp list/call` |
| `ToolsCommands` | `lx tools sync/list/schema/sync-embedded` |
| `SkillCommands` | `lx skill generate/install/uninstall/update/status` |
| `WorktreeCommands` / `GitCommands` | 工作区和 Git 风格操作 |

**边界**：纯定义，不含实现逻辑。

### 2.2 `cmd/block/mod.rs` — **Block 静态命令（重点）**

#### 职责（cmd）

将 `block_*` 系列 MCP tool 封装为**一级 CLI 子命令**，提供：

- 友好的参数解析（`--block-id`, `--file`, `--format` 等）
- 本地增强能力（MDX 自动检测/转换、格式化输出、树形展示）
- 错误信息友好化

#### 已实现的静态子命令

| 命令 | 对应 MCP Tool | 特殊处理 |
|------|--------------|---------|
| `ls` | `block_list_block_children` | 树形缩进展示 |
| `get` | `block_describe_block` | 支持 `--format mdx` 输出 MDX |
| `create` | `block_create_block_descendant` | **自动 MDX→blocks 转换** |
| `update` | `block_update_block` | 支持 text 快速更新或 MDX 全量更新 |
| `delete` | `block_delete_block` | 直接代理 |
| `move` | `block_move_blocks` | 直接代理 |
| `convert` | （本地） | MDX ↔ JSON 双向转换 |
| `export/import/tree/table-*` | （高级） | 批量导出、表格操作等 |

#### 边界（重要！）

- **是 CLI 命令层，不是 JSON-RPC handler**
- **调用 service::block 做业务逻辑，不直接调 mcp::McpClient**
- 输出格式：面向人类（table、tree、color），不是 JSON-RPC response
- **不要在这里实现 `serve/methods/*` 的内容**

#### 何时添加新的静态子命令？

当某个 MCP tool：

1. 使用频率高（用户经常手动调用）
2. 需要本地增强（MDX 转换、批量操作、格式化输出）
3. 参数复杂，CLI 比 JSON 更易用

否则走 `cmd/dynamic` 动态命令即可。

### 2.3 `cmd/dynamic/mod.rs` — 动态 MCP 命令

#### 职责（dynamic）

根据 `schemas/lexiang.json` 中的 tool schema，**自动生成** CLI 子命令并转发到 MCP Server。

#### 工作流程

```text
lx space list_spaces → 解析 namespace="space", subcommand="list_spaces"
    → 查找对应的 MCP tool name: "space_list_spaces"
    → 构建参数 JSON → 调用 McpClient.call_tool() → 格式化输出
```

#### 边界（dynamic）

- **只做"透传 + 格式化"，不做业务逻辑增强**
- 不修改、不转换输入输出数据（除了格式化显示）
- 被 static 实现覆盖时自动绕过（main.rs 中的优先级判断）

### 2.4 `cmd/tools/mod.rs` — Tool Schema 管理

| 函数 | 职责 |
|------|------|
| `handle_sync` | 从 MCP Server 同步 schema 到本地 |
| `handle_categories` | 列出 tool 分类 |
| `handle_list` | 列出某分类下的工具 |
| `handle_schema` | 输出完整 schema JSON（给编辑器集成用） |
| `handle_sync_embedded` | 开发用：同步 schema 到 `schemas/lexiang.json`（编译嵌入） |
| `handle_sync_unlisted` | 同步未列出的 tool schema |

**边界**：只管 schema 元数据，不管 tool 的实际调用。

### 2.5 `cmd/mcp/mod.rs` — MCP 通用操作

| 函数 | 职责 |
|------|------|
| `list_tools` | 列出 MCP Server 所有可用工具 |
| `call_tool` | 通用工具调用（原始 JSON 参数） |

**边界**：最底层的 MCP 调用封装，供其他 cmd 模块复用。

### 2.6 其他 cmd 模块

| 模块 | 职责 | 边界 |
|------|------|------|
| `cmd/git` | Git 风格的 workspace 操作（clone/add/commit/push/pull...） | 不依赖 shell 模块 |
| `cmd/shell` | Shell REPL 入口（build_shell/exec_command/start_repl） | 只组装，不实现命令 |
| `cmd/skill` | AI Agent skill 文件的生成/安装/卸载 | 文件系统操作为主 |
| `cmd/update` | 版本检查与更新 | GitHub API 调用 |
| `cmd/output` | 通用输出格式化（table/csv/markdown/json） | 纯展示逻辑，无业务 |
| `cmd/ui` | 交互式 UI 组件 | 终端交互 |
| `cmd/utils` | cmd 层通用工具函数 | 不依赖 business logic |

---

## 3. `serve/` — JSON-RPC 2.0 服务层 ⚠️

### 职责（serve）

提供 **stdio JSON-RPC 2.0 server**，让编辑器（VS Code、Neovim、JetBrains）通过 stdin/stdout 调用 lexiang-cli 能力。

### 架构

```text
编辑器 ──stdin──▶ transport.rs (读请求)
                      ▼
                handler.rs (路由分发)
                      ▼
         ┌──────────┴──────────┐
    methods/*.rs          fallback
   (静态注册handler)    (MCP动态代理)
         │                    │
         ▼                    ▼
   业务逻辑            ctx.mcp_call()
```

### 核心文件

| 文件 | 职责 |
|------|------|
| `mod.rs` | ServeState、ServeContext、RpcMethod 注册宏、error codes、run_serve 入口 |
| `transport.rs` | stdio 读写循环（stdin 读请求 → dispatch → stdout 写响应） |
| `handler.rs` | 方法路由：inventory 表匹配 → 未命中则 fallback 到 MCP tool call |
| `protocol.rs` | JSON-RPC 2.0 数据结构（Request/Response/Error/Notification） |

### `serve/methods/` — 静态 RPC Method Handler

| 文件 | 注册的方法 | 说明 |
|------|-----------|------|
| `auth.rs` | `auth/startOAuth`, `auth/completeOAuth` | OAuth 认证流程 |
| `contact.rs` | `contact/whoami` | 用户信息 |
| `entry.rs` | 条目相关方法 | 知识条目 CRUD |
| `file.rs` | 文件上传下载 | 文件操作 |
| `lifecycle.rs` | `initialize`, `exit` | 生命周期 |
| `quota.rs` | 配额查询 | 存储配额 |
| `search.rs` | 搜索相关 | 全文/向量搜索 |
| `space.rs` | 空间相关 | 知识库列表/详情 |
| `team.rs` | 团队相关 | 团队列表/详情 |

**如何添加新 handler**：

```rust
// 在 serve/methods/ 下任意文件
use crate::serve::{JsonRpcResult, ServeContext, rpc_method};

async fn handle_my_method(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    Ok(serde_json::json!({ "result": "..." }))
}

inventory::submit! { rpc_method!("my/domain/method", handle_my_method) }
```

### Dynamic Fallback（关键机制）

handler.rs 中，如果 inventory 表中没有匹配的 method，会自动尝试转为 MCP tool 调用：

```text
"space/list" → MCP tool "space_list"
"entry/content" → MCP tool "entry_content"
```

这意味着**新增 MCP tool 后无需改代码即可在 serve 中使用**。

### 边界（重要！⚠️）

| 可以做 | 不可以做 |
|--------|---------|
| 接收 JSON-RPC 请求，返回 JSON-RPC 响应 | 实现 CLI 命令行的业务逻辑 |
| 调用 MCP Server 获取数据 | 直接操作文件系统（除必要的缓存外） |
| 数据适配/聚合（合并多个 MCP 调用结果） | 包含 CLI 格式的用户交互（prompt、确认） |
| 通过 `ctx.mcp_call()` 调用 MCP | 直接引用 `cmd/` 层的函数 |

### 与 `cmd/` 层的本质区别

| 维度 | `cmd/` | `serve/` |
|------|--------|----------|
| 交互方式 | 人类敲命令（终端 TTY） | 编辑器发 JSON-RPC（stdio） |
| 输出格式 | table、tree、color、human-friendly | JSON（strict JSON-RPC 2.0） |
| 错误处理 | 友好提示 + exit code | JsonRpcError with code |
| 适用场景 | 开发者手工操作、CI/CD 脚本 | VS Code/Neovim 插件后端 |

---

## 4. `mcp/` — MCP 协议客户端层

### 职责（mcp）

封装与 Lexiang MCP Server 的 HTTP 通信，提供类型安全的调用接口。

| 文件 | 职责 |
|------|------|
| `client.rs` | `McpClient`：高层客户端，`list_tools()` / `call_tool()` / `call_raw<T>()` |
| `caller.rs` | `McpCaller` trait + `RealMcpCaller`：抽象接口，方便测试和注入 |
| `transport.rs` | `HttpTransport`：底层 HTTP 通信（reqwest），认证 header 注入 |
| `protocol.rs` | MCP 协议数据结构定义（ToolSchema、ToolsListResult、ToolCallResult...） |
| `schema/` | Schema 管理：运行时加载、编译时嵌入、命令生成器（CommandGenerator） |
| `upload.rs` | 文件上传配置和凭证申请 |

### 边界（mcp）

- **纯粹的通信层，不含业务逻辑**
- 不知道"block"、"entry"、"space"是什么，只知道"tool name + args → result"
- 可被 `cmd/`、`serve/`、`service/`、`shell/` 各层引用
- **不依赖 `cmd/` 或 `serve/`**

---

## 5. `service/` — 业务逻辑层

### 职责（service）

封装特定领域的业务逻辑，对 MCP 的底层调用做**组合、转换、增强**。

### 5.1 `service/block/` — Block 文档操作（当前核心）

```text
service/block/
├── mod.rs          # BlockService（门面），统一入口
├── types.rs        # 数据结构定义（Block、BlockType、BlockChildren...）
├── adapter.rs      # MCP 适配器：BlockService → McpCaller trait 的桥接
├── converter.rs    # MDX ↔ Blocks 转换器（双向）
├── document.rs     # 文档级操作（全量获取、导入导出）
├── reader.rs       # 流式读取（大文档分页）
├── table.rs        # 表格 block 专用操作
├── mdx/            # MDX 编解码
│   ├── mod.rs      # MDX 模块入口
│   ├── parser.rs   # MDX → IR（中间表示）
│   └── emitter.rs  # IR → MDX
└── ir/             # 中间表示（Intermediate Representation）
    └── mod.rs      # IR 数据结构定义
```

#### 核心设计：MDX ↔ Blocks 双向转换

```text
MDX 字符串 ──parser──▶ IR（中间表示）──emitter──▶ MDX 字符串
                    │
                    ▼
              Blocks JSON（MCP API 格式）
```

- **parser**：MDX → IR（支持 heading/paragraph/text/bullet_list/ordered_list/thematic_break/divider/code_block 等）
- **IR**：与具体格式无关的树形结构（`IRNode` / `IRNodeKind`）
- **emitter**：IR → MDX 字符串
- **converter**：IR ↔ Blocks JSON 双向转换

#### `BlockService` 门面模式

```rust
impl BlockService {
    // 基础 CRUD（委托给 adapter → McpCaller → MCP Server）
    pub async fn get_block(&self, id: &str) -> Result<Block>
    pub async fn list_children(&self, id: &str, recursive: bool) -> Result<...>
    pub async fn create_child(&self, parent_id: &str, blocks: Vec<BlockInput>) -> Result<...>
    pub async fn update_block(&self, id: &str, updates: &BlockUpdates) -> Result<()>
    pub async fn delete_block(&self, id: &str) -> Result<()>
    pub async fn move_blocks(&self, ...) -> Result<()>

    // 增强能力（MDX 转换、文档级操作）
    pub async fn get_as_mdx(&self, id: &str) -> Result<String>   // get + convert to MDX
    pub async fn create_from_mdx(&self, parent_id: &str, mdx: &str) -> Result<()>  // parse+create
    pub async fn export_document(&self, entry_id: &str) -> Result<...>
}
```

#### 边界（service）

- **知道什么是 block，但不知道 CLI 参数怎么传**
- 通过 `McpCaller` trait 解耦网络层（可替换为 mock 测试）
- **不包含任何 CLI 输出格式化代码**
- **不包含 JSON-RPC 相关代码**

---

## 6. `shell/` — 虚拟 Shell 引擎

### 职责（Shell）

实现一个类 Bash 的 Shell，让 AI Agent 用 UNIX 命令风格操作知识库。

```text
shell/
├── bash.rs         # Bash 主入口（REPL / 单次执行）
├── parser/         # 词法分析 + 语法解析（Lexer → AST → Parser）
├── interpreter/    # 解释执行引擎（管道、重定向、变量替换、条件）
├── commands/       # 内置命令实现（ls/cat/grep/find/tree/cd/pwd/mkdir/touch/rm/echo/wc/head/tail/sort/uniq/diff/stat/git-help）
└── fs/             # 虚拟文件系统抽象（IFileSystem trait → InMemoryFs/OverlayFs/MountableFs）
```

### 边界（Shell）

- **独立运行时**：有自己的 parser/interpreter/fs，不依赖 cmd/clap
- 通过 `McpCaller` trait 与 MCP 交互
- 内置命令的实现调用 `fs/` 抽象层，不直接调 MCP
- **不被 `cmd/` 或 `serve/` 引用**（只有 main.rs → cmd/shell → shell/）

---

## 7. 基础设施模块

| 模块 | 职责 | 被谁使用 |
|------|------|---------|
| `auth/` | OAuth 登录、token 管理、refresh | main, serve, cmd |
| `config/` | 配置文件加载（~/.lexiang/config.toml）、环境变量 | 全局 |
| `vfs/` | 虚拟文件系统（FUSE） | daemon |
| `daemon/` | 守护进程管理（start/stop/status） | main |
| `worktree/` | 本地工作区管理（类似 git worktree） | cmd/git |
| `datadir/` | 数据目录管理（~/.lexiang/） | auth, config |
| `update/` | 自更新（GitHub releases 检查） | cmd/update |
| `skill/` | Skill 模板渲染 | cmd/skill |
| `version/` | 版本号管理 | main |

---

## 8. 关键规则总结

### 规则 1：静态优先于动态 ✅

如果某个 tool 有了 `cmd/block/` 的静态实现，`cmd/dynamic/` 会自动跳过它。
**扩展方式**：需要本地增强 → 加静态子命令；简单透传 → 用动态命令。

### 规则 2：`serve/` ≠ `cmd/` ❌

- `serve/methods/` 是给编辑器用的 JSON-RPC handler
- `cmd/` 是给人用的 CLI 命令
- **永远不要把 CLI 业务逻辑写到 `serve/methods/` 里**

### 规则 3：依赖方向（单向）

```text
main → cmd → service → mcp (client/caller)
           ↓
        serve → mcp
           ↓
        shell → mcp
```

- 下层不能反向依赖上层
- `mcp` 是最底层通信库，可被所有层引用

### 规则 4：`service/` 是业务逻辑的唯一归属

- MDX 转换、blocks 组装、文档级操作 → `service/block/`
- `cmd/block/` 只做"解析参数 → 调 service → 格式化输出"
- `serve/methods/` 只做"解析 JSON params → 调 MCP/service → 返回 JSON"

### 规则 5：新增功能的决策树

```text
需要新功能？
├── 给编辑器插件用？ → serve/methods/ 新建 handler
├── 给终端用户用？
│   ├── 需要本地增强（转换/批量/格式化）？ → cmd/<domain>/ 新建静态子命令
│   ├── 简单调用 MCP tool？ → 走 dynamic 自动生成
│   └── 是全新的命令领域？ → cli.rs 加 Commands 枚举 + cmd/<new>/ 模块
├── 给 AI Shell 用？ → shell/commands/ 新建内置命令
└── 纯业务逻辑？ → service/ 新建或扩展现有模块
```
