---
name: lx-connector
description: |
  乐享外部数据导入。支持腾讯会议记录导入到知识库。
  当用户需要从外部系统导入数据到知识库时使用。
  触发词：腾讯会议、会议记录、导入、外部数据
metadata:
  {"openclaw": {"requires": {"bins": ["lx"]}, "always": false}}
---

# 外部数据导入

> **前置条件：** 需要 `lx` CLI 已配置并登录。

---

## 使用场景

### 场景一：导入腾讯会议记录

**触发条件：**

- 用户说"导入腾讯会议记录"/"把这个会议存到知识库"

**使用工具：** `lx-meeting-search-tx-meeting-records` / `lx-meeting-import-tx-meeting-record`

**SOP：**

1. 搜索会议记录 → `lx-meeting-search-tx-meeting-records { "meeting_code": "123456789" }`
2. 获取目标 parent_entry_id（通过 `lx-space-describe-space` 或 `lx-entry-list-children`）
3. 导入会议 → `lx-meeting-import-tx-meeting-record { "record_file_id": "rec_xxx", "parent_entry_id": "folder_xxx" }`

**特殊情况处理：**

- 未指定目标位置 → 先引导用户选择知识库和父节点
- 会议记录不存在 → 提示用户检查会议号
- 需要重新加载 → 使用 `lx-meeting-reload-tx-meeting-record`

---

## 工具速查

| 工具名 | 用途 |
|--------|------|
| `lx-meeting-search-tx-meeting-records` | 搜索腾讯会议记录 |
| `lx-meeting-list-tx-meeting-records` | 列出录制记录 |
| `lx-meeting-describe-tx-meeting-record` | 录制详情 |
| `lx-meeting-import-tx-meeting-record` | 导入会议记录到知识库 |
| `lx-meeting-reload-tx-meeting-record` | 重新加载已导入记录 |

---

## 典型组合流程

### 导入腾讯会议记录

```json
// 1. 搜索会议记录
lx-meeting-search-tx-meeting-records: { "meeting_code": "123456789" }

// 2. 获取目标位置（切换到 lx-space skill）
lx-space-describe-space: { "space_id": "sp_xxx" }

// 3. 导入会议
lx-meeting-import-tx-meeting-record: { "record_file_id": "rec_xxx", "parent_entry_id": "folder_xxx" }
```
