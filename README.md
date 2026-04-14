# lexiang-cli - 乐享知识库工具集

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

乐享知识库工具集，包含 **3 个独立产品**：

| 产品 | 说明 | 安装方式 | 更新方式 |
|------|------|----------|----------|
| **lx CLI** | 命令行工具，虚拟 Shell / Git 版本化管理 / 动态命令 | 安装脚本 / cargo / Release | 自动检查（24h） |
| **VS Code 扩展** | VS Code 知识库浏览与编辑插件 | Release 下载 .vsix | 自动检查（4h） |
| **OpenClaw 插件** | OpenClaw 平台的知识库集成插件 | npm / openclaw CLI | openclaw plugins update |

> 三个产品共享同一仓库，通过不同的 Release Tag 独立发布：`cli-v*`、`vscode-v*`、`openclaw-v*`。

---

## 📦 安装

### 1. lx CLI

Rust 编写的命令行工具，支持在线操作和本地工作区两种模式。

#### 无 Cargo 环境（推荐）

```bash
curl -fsSL https://raw.githubusercontent.com/tencent-lexiang/lexiang-cli/main/scripts/install.sh | sh
```

自定义安装目录：

```bash
curl -fsSL https://raw.githubusercontent.com/tencent-lexiang/lexiang-cli/main/scripts/install.sh | sh -s -- --dir /usr/local/bin
```

脚本会自动：

- 检测当前平台并选择对应二进制
- 通过 `releases/latest/download` 下载最新版本
- 校验 `SHA256SUMS.txt`
- 安装到 `~/.local/bin`（或你指定的目录）

#### Rust 生态安装

```bash
cargo install --git https://github.com/tencent-lexiang/lexiang-cli --locked
```

#### 从 Release 下载

直接从 [GitHub Releases](https://github.com/tencent-lexiang/lexiang-cli/releases) 下载对应平台二进制，支持 macOS (arm64/x86_64/universal)、Linux (x86_64/arm64/musl)、Windows (x86_64)。

#### 本地源码安装

```bash
git clone https://github.com/tencent-lexiang/lexiang-cli.git
cd lexiang-cli
cargo install --path crates/lx
```

#### 更新检查

```bash
lx update check     # 检查是否有新版本
lx update list      # 列出最近发布版本
```

CLI 每隔 24 小时自动静默检查更新，有新版本时提示。

### 2. VS Code 扩展

在 VS Code 中浏览和管理乐享知识库，支持文档查看、知识库挂载、AI 对话集成等。

从 [GitHub Releases](https://github.com/tencent-lexiang/lexiang-cli/releases) 下载 `.vsix` 文件（标签以 `vscode-v` 开头），然后：

```bash
code --install-extension lefs-vscode-*.vsix
```

或在 VS Code 中：Extensions → `...` → Install from VSIX...

扩展启动后每 4 小时自动从 GitHub Release 检查更新，有新版本时提示一键更新。

### 3. OpenClaw 插件

为 OpenClaw 平台提供乐享知识库集成能力。

```bash
# 安装
openclaw plugins install @tencent-lexiang/openclaw-lexiang

# 交互式配置（自动安装 lx CLI + 配置 Token）
openclaw onboard
```

也可以从 [npm](https://www.npmjs.com/package/@tencent-lexiang/openclaw-lexiang) 直接安装：

```bash
npm install -g @tencent-lexiang/openclaw-lexiang
```

更新：

```bash
openclaw plugins update @tencent-lexiang/openclaw-lexiang
```

## 🚀 快速开始

```bash
lx login           # OAuth 登录
lx --help          # 查看所有命令
lx tools sync      # 同步最新工具定义
```

## 🐚 Just-Bash 虚拟 Shell

`lx sh` 提供面向知识库的虚拟 Shell，用 Bash 风格命令浏览和搜索知识库，无需记忆 API 参数。

### 启动方式

```bash
# Worktree 模式（推荐）：先 clone 再进入 Shell
lx git clone <SPACE_ID> ./my-kb && cd my-kb && lx sh

# MCP 远程模式：直接连接远端知识库
lx sh --space <SPACE_ID>

# 单次执行并退出
lx sh -e "ls /kb"
lx sh --space <SPACE_ID> -e "tree -L 2 /kb"
```

### 虚拟文件系统

```text
/
├── kb/        # 知识库挂载点（只读）
└── tmp/       # 临时可写区域
```

- `/kb`：知识库内容（Worktree 模式映射本地磁盘，MCP 模式实时从远端加载）
- `/tmp`：临时可写区域，供 `sort`、`uniq`、重定向等使用
- 默认工作目录 `/kb`

### 内置命令

| 命令         | 说明                    | 命令            | 说明              |
|--------------|-------------------------|-----------------|-------------------|
| `ls`         | 列出目录                | `cat`           | 查看文件内容      |
| `grep`       | 搜索文本                | `find`          | 查找文件          |
| `tree`       | 目录树                  | `head` / `tail` | 查看头部/尾部     |
| `wc`         | 统计行数/单词/字符      | `sort` / `uniq` | 排序/去重         |
| `pwd` / `cd` | 路径导航                | `echo`          | 输出文本          |
| `stat`       | 查看文件信息            | `xargs`         | 参数传递          |
| `fzf`        | 模糊筛选                | `search`        | 知识库关键词搜索  |
| `git`        | Git 操作（仅 Worktree） | `mcp`           | 透传调用 MCP Tool |

### 现代 CLI 兼容

自动兼容 `rg`、`eza`、`fd`、`bat` 等现代工具：`rg "p" /kb` → `grep -rn "p" /kb`、`eza /kb` → `ls -la /kb`、`fd ".md" /kb` → `find /kb -name "*.md"`、`bat /kb/README.md` → `cat /kb/README.md`

### 只读保护

`rm`、`mv`、`cp`、`mkdir`、`touch`、`chmod` 等写操作返回"只读文件系统"错误，防止误操作。

## 📁 Git 版本化管理

`lx git` 提供类 Git 工作流，支持离线编辑、批量同步和版本回退。本地工作区创建 `.lxworktree` 目录，存储文档与远端的映射关系和本地提交历史。

### 命令

| 命令                             | 说明                               |
|----------------------------------|------------------------------------|
| `lx git clone <space_id> <path>` | 克隆知识库到本地                   |
| `lx git status`                  | 查看本地变更状态                   |
| `lx git add`                     | 暂存文件变更                       |
| `lx git commit -m "msg"`         | 提交到本地仓库                     |
| `lx git push`                    | 推送本地变更到乐享知识库           |
| `lx git pull`                    | 拉取乐享知识库最新内容             |
| `lx git log`                     | 查看本地提交历史                   |
| `lx git diff`                    | 查看本地变更详情                   |
| `lx git diff --remote`           | 对比本地与远端差异                 |
| `lx git reset`                   | 本地版本回退                       |
| `lx git revert <commit>`         | 回退知识库到历史版本（推送后生效） |
| `lx worktree list`               | 列出所有工作区                     |
| `lx worktree remove <path>`      | 删除工作区                         |

### 支持的文件类型

| 类型   | 本地文件      | 拉取            | 推送         | 版本回退 |
|--------|---------------|-----------------|--------------|----------|
| 页面   | `.md`         | ✅ 转为 Markdown | ✅ 覆盖内容   | ✅        |
| 文件   | PDF/DOCX/XLSX | ✅ 下载原文件    | ✅ 预签名上传 | ✅        |
| 文件夹 | 目录          | ✅ 创建目录结构  | ✅ 自动创建   | -        |

## 🔧 动态命令系统

CLI 自动从 MCP Schema 生成命令，新功能上线后只需 `lx tools sync` 即可同步。

| 命名空间  | 说明         | 命令数 |
|-----------|--------------|--------|
| `team`    | 团队接口     | 3      |
| `space`   | 知识库接口   | 3      |
| `entry`   | 知识增删改查 | 10     |
| `block`   | 在线文档     | 10     |
| `file`    | 知识文件     | 5      |
| `search`  | 搜索         | 2      |
| `ppt`     | PPT 服务     | 6      |
| `meeting` | 腾讯会议     | 5      |
| `comment` | 知识评论     | 2      |
| `contact` | 联系人       | 2      |
| `iwiki`   | iWiki        | 1      |

工具管理命令：`lx tools sync`、`lx tools categories`、`lx tools list --category <name>`、`lx tools skill`

所有命令都有帮助信息：`lx <namespace> <command> --help`

## 🎨 Shell 补全

```bash
eval "$(lx completion bash)"   # Bash
eval "$(lx completion zsh)"    # Zsh
lx completion fish > ~/.config/fish/completions/lx.fish  # Fish
```

## ⚙️ 配置

| 文件   | 位置                             | 说明                                 |
|--------|----------------------------------|--------------------------------------|
| 主配置 | `~/.lexiang/config.json`         | MCP 地址等                           |
| Token  | `~/.lexiang/auth/token.json`     | OAuth token，支持自动刷新            |
| Schema | `~/.lexiang/tools/override.json` | `lx tools sync` 生成，优先级高于内置 |

## 🐛 故障排查

| 问题         | 解决方案                      |
|--------------|-------------------------------|
| Token 过期   | `lx login`                    |
| 命令未找到   | `lx tools sync`               |
| 查看详细日志 | `RUST_LOG=debug lx <command>` |
| 查看请求参数 | `RUST_LOG=trace lx <command>` |

## 📖 文档

- [使用指南](docs/USAGE.md) - 完整的使用文档
- [API 文档](schemas/) - MCP Schema 定义

## 📝 许可证

MIT - 详见 [LICENSE](LICENSE)

## 🙏 致谢

- [clap](https://github.com/clap-rs/clap) - 命令行参数解析
- [tokio](https://github.com/tokio-rs/tokio) - 异步运行时
- [gix](https://github.com/Byron/gitoxide) - Git 实现
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP 客户端
