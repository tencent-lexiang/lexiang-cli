# 快速开始

lx 是乐享知识库的命令行工具，支持在线操作和本地工作区两种模式。

## 安装

### 从源码安装

```bash
git clone <repo-url>
cd lexiang-cli
cargo install --path .
```

### 验证安装

```bash
lx version
```

## 登录

### 客户端登录（推荐）

通过乐享客户端重定向获取 Cookie，可调用依赖 Cookie 的内部接口：

```bash
lx login --client
```

CLI 会显示登录链接，在浏览器中完成登录后，将回调链接 `lexiang://auth-callback?code=...&state=...` 粘贴到终端。

### OAuth 登录

```bash
lx login
```

浏览器会自动打开 OAuth 登录页面。Token 保存在 `~/.lexiang/auth/token.json`。

也可以直接用 token 登录：

```bash
lx login --token "your_access_token"
```

## 前三步

### 1. 看看有哪些命令

```bash
lx --help
```

输出分两部分：

- **静态命令**：写在代码里的（login、serve、sh、git...）
- **动态命令**：从 MCP Schema 自动生成的（team、space、entry、block...）

### 2. 列出团队和知识库

```bash
# 可访问的团队
lx team list

# 某团队下的知识库
lx space list --team-id <TEAM_ID>

# 知识库详情（拿到 root_entry_id）
lx space describe --space-id <SPACE_ID>
```

### 3. 搜索或浏览

```bash
# 全局搜索
lx search kb --keyword "项目文档"

# 或用虚拟 shell（类 Bash 风格）
lx sh --space <SPACE_ID> -e "tree -L 2 /kb"
```

## 下一步

- [命令参考](./commands.md) — 所有 namespace 和 tool 的完整列表
- [虚拟 Shell](./shell.md) — `lx sh` 的完整能力说明
- [Git 工作流](./git-workflow.md) — 本地克隆、编辑、版本管理
- [配置与排障](./reference.md) — 配置文件、输出格式、FAQ
