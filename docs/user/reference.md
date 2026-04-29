# 配置参考与故障排查

## 配置文件

### 主配置

路径：`~/.lexiang/config.json`

```json
{
  "mcp": {
    "url": "https://mcp.lexiang-app.com/mcp",
    "access_token": null
  }
}
```

### Token

路径：`~/.lexiang/auth/token.json`

```json
{
  "access_token": "xxx",
  "refresh_token": "yyy",
  "expires_at": 1234567890
}
```

有 `refresh_token` 时 CLI 自动刷新。

### Client Session

路径：`~/.lexiang/auth/session.json`

```json
{
  "cookie": "uid=xxx; session=yyy",
  "created_at": 1234567890
}
```

保存客户端登录获得的 Cookie，用于调用需要 Cookie 的乐享内部接口。文件权限为 `0600`。

### Schema

路径：`~/.lexiang/tools/override.json`

由 `lx tools sync` 生成，优先级高于内置 Schema。

## Shell 补全

```bash
# Bash（临时 / 永久）
eval "$(lx completion bash)"
lx completion bash >> ~/.bashrc

# Zsh
eval "$(lx completion zsh)"
lx completion zsh >> ~/.zshcompletions/_lx

# Fish
lx completion fish > ~/.config/fish/completions/lx.fish
```

## 版本与更新

```bash
lx version
lx update check                    # 检查新版本
lx update check --prerelease       # 含预发布版
lx update list                     # 最近发布记录
```

## 实用技巧

### 批量导出

```bash
lx entry list-children --parent-id <ID> -o csv > entries.csv
lx team list -o json | jq '.data.teams[].name'
```

### 与其他工具组合

```bash
# fzf 交互选择
lx search kb --keyword "文档" -o json | jq -r '.data.docs[] | "\(.id)\t\(.title)"' | fzf

# 批量获取
for id in $(cat ids.txt); do
  lx entry describe-ai-parse-content --entry-id "$id" -o json >> all_docs.json
done
```

### 调试日志

```bash
RUST_LOG=debug lx team list
RUST_LOG=trace lx search kb --keyword "test"
```

## 故障排查

| 问题 | 解决方案 |
|------|---------|
| Token 过期 | `lx login` |
| Cookie 登录失效 | 重新运行 `lx login --client` |
| 回调链接粘贴失败 | 确认链接包含 `code=` 参数 |
| 命令缺失 | `lx tools sync` |
| 检查 Token 有效期 | `cat ~/.lexiang/auth/token.json \| jq '.expires_at \| todate'` |
| 检查 Schema 加载 | `cat ~/.lexiang/tools/override.json \| jq '.tools.keys \| length'` |

## FAQ

### 如何获取 entry-id？

从页面 URL 提取：`https://lexiangla.com/pages/xxx` 中的 `xxx`。

### 参数太复杂？

改用 JSON 传参：`lx block update -d '{"block_id":"xxx", ...}'`

### 脚本里怎么用最稳妥？

优先 `-o json` + `jq`：

```bash
TEAM_ID=$(lx team list -o json | jq -r '.data.teams[0].id')
```

### 只想快速浏览？

用 `lx sh`（见 [Shell 文档](./shell.md)）。

### 想离线编辑/批量导入？

用 `lx git`（见 [Git 工作流](./git-workflow.md)）。
