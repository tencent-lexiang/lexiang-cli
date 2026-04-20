---
name: lx-connector
version: 1.0.0
description: "乐享外部数据导入。支持腾讯会议记录导入到知识库。当用户需要从外部系统导入数据到知识库时使用。触发词：腾讯会议、会议记录、导入、外部数据"
metadata:
  requires:
    bins: ["lx"]
---

# 外部数据导入

> **前置条件：** 需要 `lx` CLI 已配置并登录。

## ⚡ 什么时候用这个 skill？

**进入场景：**

- 用户说"导入腾讯会议记录"/"把这个会议存到知识库"

**禁止在本 skill 中执行：**

- **不要编辑页面内容**：用户说"编辑某个页面内容" → **立即切换到 lx-block skill**
- **不要创建页面**：用户说"在知识库里创建页面" → **立即切换到 lx-entry skill**

## ⚡ 怎么选命令？（决策树）

```text
识别场景 →
└── 导入腾讯会议记录?
    └── lx meeting search-tx-meeting-records → lx meeting import-tx-meeting-record
```

## ⚠️ 高风险操作与默认优先路径

**会议导入流程：**

- 必须先搜索（`lx meeting search-tx-meeting-records`）拿到录制信息
- 再导入（`lx meeting import-tx-meeting-record`）
- 导入需要指定目标知识库的 `--parent-entry-id`

**默认优先路径：**

1. 导入目标必须预先确定 → 若用户未指定，需先通过 lx-space skill 定位目标知识库和父节点

## 可用工具

| 命令 | 说明 | 参考 |
|------|------|------|
| `lx meeting search-tx-meeting-records` | 搜索腾讯会议记录 | [meeting.md](references/meeting.md) |
| `lx meeting list-tx-meeting-records` | 列出录制记录 | [meeting.md](references/meeting.md) |
| `lx meeting describe-tx-meeting-record` | 录制详情 | [meeting.md](references/meeting.md) |
| `lx meeting import-tx-meeting-record` | 导入会议记录到知识库 | [meeting.md](references/meeting.md) |
| `lx meeting reload-tx-meeting-record` | 重新加载已导入记录 | [meeting.md](references/meeting.md) |

## 🎯 执行规则

1. **会议导入流程**：必须先搜索（`lx meeting search-tx-meeting-records`）拿到录制信息，再导入（`lx meeting import-tx-meeting-record`）。导入需要指定目标知识库的 `--parent-entry-id`。
2. **导入目标必须预先确定**：所有导入操作都需要 `--parent-entry-id`，若用户未指定，需先通过 lx-space skill 定位目标知识库和父节点。

## 典型组合流程

### 导入腾讯会议记录

```bash
# 搜索会议记录
lx meeting search-tx-meeting-records --meeting-code "123456789"

# 用户确认后导入
lx meeting import-tx-meeting-record \
  --record-file-id rec_file_xxx \
  --parent-entry-id folder_xxx
```
