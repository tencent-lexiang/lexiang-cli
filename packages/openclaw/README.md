# @tencent-lexiang/openclaw-lexiang"

OpenClaw 插件，将 `lx` CLI 包装为 AI 工具。

## 特性

- **Onboarding 支持**：运行 `openclaw onboard` 自动安装 CLI 和配置 Token
- **Schema 自动生成**：基于 MCP schema 自动生成所有工具，无需手动维护
- **自动二进制管理**：自动从 GitHub Releases 下载对应平台的 `lx` 二进制
- **跨平台支持**：macOS (arm64/x64)、Linux (x64)、Windows (x64)

## 安装

```bash
# OpenClaw 插件安装
openclaw plugins install @lexiang/openclaw-plugin

# 运行 onboard 配置
openclaw onboard
```

## 配置

在 OpenClaw 设置中配置：

- **Access Token**: 乐享 API Token（从 <https://lexiang.tencent.com/ai/claw> 获取）
- **Binary Path**: 自定义 lx 二进制路径（可选）
- **Auto Generate Tools**: 是否基于 schema 自动生成工具（默认 true）

或设置环境变量：

```bash
export LEXIANG_ACCESS_TOKEN=your_token
```

## 工具生成

插件会自动从 `lx` CLI 读取 MCP schema，动态生成所有可用工具。

```bash
# 同步最新 schema
lx tools sync

# 查看可用工具
lx tools list
```

同步后重启 OpenClaw 即可使用新工具。

## 核心工具（Fallback）

当 schema 不可用时，会注册以下核心工具：

| 工具        | 描述                             |
|-------------|----------------------------------|
| `lx-status` | 检查 CLI 状态、安装、同步 schema |
| `lx-search` | 关键词搜索                       |
| `lx-whoami` | 当前用户信息                     |

## 二进制查找顺序

1. 配置的 `binaryPath`
2. 系统 PATH 中的 `lx`
3. 插件 `bin/` 目录下的预编译二进制
4. `~/.lexiang/bin/lx`（自动下载位置）
5. 从 GitHub Releases 下载

## 开发

```bash
cd openclaw

# 安装依赖
pnpm install

# 生成 cli-config.json（从 git remote 读取仓库地址）
pnpm run generate:cli-config

# 构建
pnpm run build

# 开发模式
pnpm run dev
```

## 发布 npm 包

本包通过 GitHub Actions 自动发布到 **GitHub Packages**。无需配置额外的 npm token，直接使用 Actions 自动提供的 `GITHUB_TOKEN` 即可。

发布流程：

1. 更新 `package.json` 中的版本号。

2. 创建并推送一个以 `npm-v` 开头的 tag（推荐格式 `npm-vX.Y.Z`），或在 GitHub 上发布一个 Release：

   ```bash
   git tag npm-v0.1.1
   git push origin npm-v0.1.1
   ```

3. GitHub Actions 会自动运行测试、构建并执行 `npm publish`，将包发布到 GitHub Packages（`https://npm.pkg.github.com`）。

> **注意**：安装该包的用户需要在本地 `.npmrc` 中配置 `@lexiang:registry=https://npm.pkg.github.com` 并提供个人 GitHub token（`read:packages` 权限）。

## 发布预编译二进制

在 GitHub Releases 中上传以下格式的文件：

```text
lx-aarch64-apple-darwin.tar.gz    # macOS Apple Silicon
lx-x86_64-apple-darwin.tar.gz     # macOS Intel
lx-x86_64-unknown-linux-gnu.tar.gz # Linux
lx-x86_64-pc-windows-msvc.tar.gz  # Windows
```

每个 tar.gz 包含一个 `lx` (或 `lx.exe`) 二进制文件。
