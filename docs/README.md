# lexiang-cli 文档中心

## 我是谁？想做什么？

| 身份 | 从这里开始 |
|------|-----------|
| **第一次用 lx** | [用户文档 - 快速开始](./user/getting-started.md) |
| **想查某个命令怎么用** | [用户文档 - 命令参考](./user/commands.md) |
| **要参与开发 / 改代码** | [开发者文档 - 开发指南](./dev/development.md) |
| **不知道该在哪层写代码** | [开发者文档 - 模块边界](./dev/module-boundaries.md) |
| **让 AI Agent 会用 lx** | `skills/` 目录下的 SKILL.md |

---

## 文档结构

```text
docs/
├── README.md                  ← 你在这里
│
├── user/                      📖 用户文档（怎么用）
│   ├── getting-started.md     # 安装、登录、前三步
│   ├── commands.md            # 全部命令参考（search/team/space/entry/block/file...）
│   ├── schema.md              # 动态 Schema 同步与管理
│   ├── shell.md               # lx sh 虚拟 Shell 完整指南
│   ├── git-workflow.md        # lx git 本地版本化工作流
│   └── reference.md           # 配置文件、补全、排障、FAQ
│
├── dev/                       🔧 开发者文档（怎么改）
│   ├── development.md         # 项目架构、三层命令、Schema 策略、开发流程
│   └── module-boundaries.md   # 每层职责、依赖方向、常见错误、决策树
│
└── plans/                     📐 设计文档（为什么这样设计）
    └── *.md                   # RFC / ADR

skills/                        🤖 Agent 技能文件（lx skill generate 自动生成）
├── lx-block/SKILL.md          # block 操作
├── lx-smartsheet/SKILL.md     # 智能表格操作
├── lx-entry/SKILL.md          # 条目操作
├── lx-search/SKILL.md         # 搜索
└── ...
```

## 维护规则

| 触发场景 | 更新什么 |
|---------|---------|
| 新增用户功能 | `docs/user/*.md` |
| 新增/重构模块 | `docs/dev/*.md` |
| 重大架构变更前 | 在 `docs/plans/` 写 RFC |
| MCP Tool 增减 | `lx skill update` 重生成 `skills/` |
