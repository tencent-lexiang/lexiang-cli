---
name: lx-url-route
description: |
  识别乐享相关 URL 并路由到对应的工具调用。当用户发送了链接但不明确要做什么时，用此 Skill 判断链接类型并调用合适的命令。
  触发词：链接、URL、帮我看看这个、lexiangla.com、lexiang.tencent.com、mp.weixin.qq.com
metadata:
  {"openclaw": {"requires": {"bins": ["lx"]}, "always": false}}
---

# URL 路由

> **前置条件：** 需要 `lx` CLI 已配置并登录。

---

## 使用场景

### 场景一：用户只发了页面链接

**触发条件：**

- 用户只发了一个 `/pages/{entry_id}` 链接，没有明确说要做什么

**处理方式：**

1. 提取 `entry_id`
2. 默认查看内容 → 调用 `lx-entry-describe-ai-parse-content { "entry_id": "entry_xxx" }`
3. 如用户后续明确意图：
   - 编辑内容 → 切换到 `lx-block` skill
   - 查看评论 → 切换到 `lx-connector` skill

---

### 场景二：用户发了知识库链接

**触发条件：**

- 用户只发了一个 `/spaces/{space_id}` 链接

**处理方式：**

1. 提取 `space_id`
2. 调用 `lx-space-describe-space { "space_id": "sp_xxx" }` 获取详情
3. 获取 `root_entry_id` 后，根据用户意图决定：
   - 浏览目录 → 切换到 `lx-entry` skill
   - 创建页面 → 切换到 `lx-entry` skill

---

### 场景三：用户发了团队链接

**触发条件：**

- 用户只发了一个 `/t/{team_id}/spaces` 链接

**处理方式：**

1. 提取 `team_id`
2. 调用 `lx-space-list-spaces { "team_id": "team_xxx" }` 列出该团队的知识库
3. 让用户选择目标知识库

---

### 场景四：用户发了公众号文章链接

**触发条件：**

- 用户发了 `mp.weixin.qq.com/*` 链接

**处理方式：**

1. 确认用户是否要导入到知识库
2. 如确认，先获取 `space_id` 和 `parent_entry_id`
3. 调用 `lx-file-create-hyperlink { "url": "...", "space_id": "...", "parent_entry_id": "..." }`

---

### 场景五：Token 配置页面

**触发条件：**

- 用户发了 `lexiang.tencent.com/ai/claw` 链接

**处理方式：**

- 提示用户去该页面获取 Access Token 进行配置
- 不调用任何命令

---

## URL 类型与路由

| URL 模式 | 提取字段 | 默认动作 | 用户意图明确时的去向 |
|----------|----------|----------|---------------------|
| `/pages/{entry_id}` | `entry_id` | `lx-entry-describe-ai-parse-content` | 编辑 → `lx-block`；评论 → `lx-connector` |
| `/spaces/{space_id}` | `space_id` | `lx-space-describe-space` | 浏览/创建 → `lx-space` → `lx-entry` |
| `/t/{team_id}/spaces` | `team_id` | `lx-space-list-spaces` | 选定知识库后继续 |
| `mp.weixin.qq.com/*` | 原始 URL | 先确认意图，再 `lx-file-create-hyperlink` | 导入后编辑 → `lx-entry` / `lx-block` |
| `lexiang.tencent.com/ai/claw` | 无 | 提示获取 Token | 不调用命令 |
| `mcp.lexiang-app.com/*` | 无 | 内部端点 | 不展示给用户 |

---

## 执行规则

1. **先判断要不要路由**：只有"用户给了 URL，但意图不明确"时才用本 skill
2. **页面链接默认看内容**：默认动作是 `describe-ai-parse-content`
3. **公众号文章导入必须先补齐目标位置**：缺 `space_id` / `parent_entry_id` 时不能直接导入
4. **不暴露内部域名**：`mcp.lexiang-app.com` 只用于内部服务识别
5. **路由完成就退出**：拿到目标 ID 或确定下游命令后，切换到对应 skill
