# space & team — 知识库与团队管理

> **前置条件：** 先阅读 [`../SKILL.md`](../SKILL.md) 了解层级关系 Team → Space → Entry。

知识库和团队管理的所有工具。核心价值：帮助用户定位目标知识库、获取 `root_entry_id`（后续操作条目的起点）、管理团队组织结构。

## 🌟 我的知识库（一等公民）

**最快捷的入口：** 用户说"我的知识库"时直接使用，无需先查团队。

```bash
# 获取个人知识库（如果不存在则自动创建）
lx space mine

# 同步创建（等待创建完成后直接返回 space 对象）
lx space mine --sync
```

**返回结构：**

- 正常返回：`{ space: { id, name, root_entry_id, ... } }`
- 异步创建中：`{ task_id: "...", is_creating: true }`（需要稍后重试或轮询）

**典型流程：**

```bash
# Step 1: 获取我的知识库
lx space mine
# → 拿到 space_id 和 root_entry_id

# Step 2: 直接操作（→ lx-entry skill）
lx entry list-children --parent-id root_entry_xxx
```

## 使用场景

### 用户说"帮我看看我的知识库"——从团队开始逐级定位

```bash
# Step 1: 列出可访问的团队
lx team list-teams

# Step 2: 列出团队下的知识库
lx space list-spaces --team-id team_xxx

# Step 3: 获取知识库详情，拿到 root_entry_id
lx space describe-space --space-id sp_xxx

# Step 4: 浏览一级目录
lx entry list-children --parent-id root_entry_xxx
```

### 用户已给出知识库 ID 或链接

```bash
# 从 URL 提取：{domain}/spaces/{space_id} → space_id
lx space describe-space --space-id sp_abc123
# → 拿到 root_entry_id，后续操作条目
```

### 快速找到常用知识库

```bash
# 获取常用团队
lx team list-frequent-teams

# 获取最近访问的知识库
lx space list-recently-spaces
```

### 确认知识库元信息

```bash
lx space describe-space --space-id sp_xxx
# 返回: name, root_entry_id, team_id, entry_count
```

## 关键规则

1. **获取 root_entry_id 是核心目标**：几乎所有条目操作都需要从 `root_entry_id` 开始。如果用户给了知识库 ID，直接 `lx space describe-space` 拿到它。
2. **从哪里开始**：用户给了 `space_id` → 直接 `lx space describe-space`；只知道团队名 → `lx team list-teams` → `lx space list-spaces`；什么都不知道 → `lx space list-recently-spaces` 或 `lx team list-frequent-teams`。
3. **URL 解析**：知识库链接格式 `{domain}/spaces/{space_id}`，从中提取 `space_id`。团队首页 `{domain}/t/{team_id}/spaces`。
4. **不要重复调用**：如果已经拿到了 `root_entry_id`，不要再调一次 `lx space describe-space`。在工作流中保持上下文。

## ⚠️ 副作用与风险

- `lx space list-spaces` 必须传 `--team-id`，不传会报错。如果不知道 team_id，先用 team 相关命令获取。
- `root_entry_id` 不要和 `space_id` 混淆——前者是条目树的根节点 ID，后者是知识库 ID。
- 知识库 URL 格式是 `/spaces/{space_id}`，条目 URL 格式是 `/pages/{entry_id}`，注意区分。

## 详细参数

所有命令的完整参数说明请运行：

```bash
lx space --help
lx team --help
```

## 参考

- [lx-space](../SKILL.md) — 知识库 skill 完整决策树
- [lx-entry](../../lx-entry/SKILL.md) — 拿到 root_entry_id 后的条目操作
- [lx-search](../../lx-search/SKILL.md) — 搜索知识库内容
