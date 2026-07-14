# 开发指南

本文档面向 lexiang-cli 贡献者。架构总览见 [模块边界](./module-boundaries.md)。

## 项目结构

```text
lexiang-cli/
├── crates/lx/src/          # 主程序源码
│   ├── main.rs              # 入口：参数解析、命令分发
│   ├── cmd/                 # CLI 命令层（用户交互入口）
│   │   ├── cli.rs           # Clap 命令定义
│   │   ├── block/           # Block 静态子命令
│   │   ├── local_file.rs    # 本地文件上传/内容导入增强命令
│   │   ├── dynamic/         # 动态 MCP 命令（schema 驱动）
│   │   ├── tools/           # Schema 管理命令
│   │   ├── mcp/             # MCP 通用操作
│   │   ├── git/             # Git 风格 workspace 操作
│   │   ├── shell/           # Shell REPL 入口
│   │   ├── skill/           # Skill 生成/安装
│   │   └── output/          # 输出格式化
│   ├── serve/               # JSON-RPC stdio 服务层（编辑器插件后端）
│   │   ├── mod.rs           # ServeState、ServeContext、rpc_method 宏
│   │   ├── transport.rs     # stdin/stdout 读写循环
│   │   ├── handler.rs       # 路由：inventory 表 → MCP fallback
│   │   └── methods/         # 各领域 RPC handler（auth/space/entry...）
│   ├── mcp/                 # MCP 协议客户端
│   │   ├── client.rs        # McpClient（高层）
│   │   ├── caller.rs        # McpCaller trait + RealMcpCaller
│   │   ├── transport.rs     # HttpTransport（底层 HTTP）
│   │   ├── protocol.rs      # 数据结构定义
│   │   ├── schema/          # Schema 管理（运行时/嵌入/生成器）
│   │   └── upload.rs        # 文件上传
│   ├── service/             # 业务逻辑层
│   │   └── block/           # Block 操作、MDX↔Blocks 转换
│   ├── shell/               # 虚拟 Shell 引擎
│   │   ├── bash.rs          # 主入口
│   │   ├── parser/          # 词法+语法解析
│   │   ├── interpreter/     # 执行引擎（管道/重定向）
│   │   ├── commands/        # 内置命令实现
│   │   └── fs/              # 虚拟文件系统抽象
│   ├── auth/                # OAuth 认证 / Token 管理
│   ├── config/              # 配置加载 (~/.lexiang/config.json)
│   ├── daemon/              # 守护进程管理
│   └── vfs/                 # 虚拟文件系统（FUSE）
├── schemas/                 # 编译时嵌入的 Schema 源文件
│   ├── lexiang.json         # listed tools schema
│   └── unlisted.json        # unlisted tools 配置
├── skills/                  # Agent Skill 文件（模板/参考）
├── packages/                # JS/TS 相关包
└── docs/                    # 文档
    ├── user/                # 用户文档
    ├── dev/                 # 开发者文档（本目录）
    └── plans/               # 设计文档（RFC/ADR）
```

## 四层命令架构

```text
┌─────────────────────────────────────┐
│  Layer 1: 静态 Clap 命令            │
│  lx login / serve / sh / git / ...  │
├─────────────────────────────────────┤
│  Layer 2: 静态 Block 子命令         │
│  lx block ls / get / create / ...   │  ← 优先级高于 Layer 3
├─────────────────────────────────────┤
│  Layer 3: 本地文件增强子命令         │
│  lx file upload / entry import ...   │  ← 只认领两个本地命令族
├─────────────────────────────────────┤
│  Layer 4: 动态 MCP 命令             │
│  lx team list / space describe / .. │  ← 从 schema 自动生成
└─────────────────────────────────────┘
```

分发逻辑在 `main.rs`：

1. `lx block <subcmd>` → `cmd/block/mod.rs`（静态，优先）
2. `lx file upload` / `lx entry import <format>` → `cmd/local_file.rs`
3. `lx <namespace> <tool>` → `cmd/dynamic/mod.rs`（动态 MCP）
4. `lx <clap 子命令>` → `Cli::parse()` → 各 cmd 模块

## MCP Resource 与 DSL

服务端通过 MCP Resource 发布版本化 DSL，CLI 只提供发现和读取，不复制或解析正文：

```bash
lx mcp resource list --format json
lx mcp resource read <lexiang://...>
```

`McpClient::list_resources()` 负责跟随 `nextCursor` 聚合结果，
`McpClient::read_resource()` 保留 text/blob 和 MIME 元数据。动态命令生成器会从 Tool
description 中提取 `lexiang://` URI 并写入长帮助；如果新工具依赖 Resource，服务端的
Tool description 应显式声明该 URI。

## Schema 四层合并

优先级：**embedded + unlisted < override < custom**

| 层 | 来源 | 文件 | 说明 |
|----|------|------|------|
| 1 | 编译时嵌入 | `schemas/lexiang.json` | listed tools |
| 2 | 编译时嵌入 | `schemas/unlisted.json` | unlisted tools（配置即代码） |
| 3 | 运行时同步 | `~/.lexiang/tools/override.json` | `lx tools sync` 生成 |
| 4 | 用户自定义 | `~/.lexiang/tools/custom.json` | 手动编辑 |

启动时：override 存在则用它；否则 fallback 到 embedded。

## 开发流程

### 添加新的静态 Block 子命令

1. 在 `cmd/block/mod.rs` 添加 clap 子命令和 handler
2. 在 `is_static_subcommand()` 中注册名称（防止被动态覆盖）
3. handler 中调用 `service::block::BlockService` 做业务逻辑
4. 需要 MDX 转换时调用 `service::block::converter`

### 添加新的 JSON-RPC Handler（serve 层）

```rust
// 在 serve/methods/ 下任意文件
use crate::serve::{JsonRpcResult, ServeContext, rpc_method};

async fn handle_my_api(ctx: &ServeContext, params: Value) -> JsonRpcResult {
    let data = ctx.mcp_call("my_tool", params).await?;
    Ok(data)
}

inventory::submit! { rpc_method!("my/domain/api", handle_my_api) }
```

未注册的方法会自动 fallback 到 MCP tool call（`handler.rs` 的 dynamic proxy）。

### 更新内置 Schema

```bash
# Listed: 从 MCP Server 全量拉取
cargo run -- tools sync-embedded

# Unlisted: 在 schemas/unlisted.json 加 tool_name 后
cargo run -- tools sync-unlisted

# 重新编译嵌入
cargo build
```

`build.rs` 监听 `schemas/*.json` 变更，修改后自动重新编译。

### 添加新的输出格式

1. `mcp/schema/generator.rs`: 添加 format 参数
2. `cmd/dynamic/mod.rs`: 添加格式处理分支
3. `cmd/output/`: 实现格式化函数

## 测试

```bash
cargo test                           # 所有测试
cargo test mcp::schema::types       # 特定模块
cargo build --release && ./target/release/lx --help  # 构建验证
```

## 调试

```bash
RUST_LOG=debug cargo run -- team list
cat ~/.lexiang/auth/token.json
cat ~/.lexiang/tools/override.json | jq '.tools | keys | length'
ls ~/.lexiang/skills/
```
