---
name: lx-smartsheet
version: 1.1.0
description: "乐享智能表格管理。当用户需要创建或读取智能表格、修改 schema、增删改查记录或管理视图时使用。触发词：智能表格、smartsheet、字段、记录、视图、CREATE TABLE、ALTER TABLE"
metadata:
  requires:
    bins: ["lx"]
---

# 智能表格

> 前置条件：`lx` CLI 已配置并登录。除创建阶段外，操作时始终保留页面 `entry_id` 和智能表格 `smartsheet_id`。

## 统一命名空间

服务端新增接口虽然暂时归在 `knowledge.block` category，CLI 已统一提升到 `lx smartsheet`：

| 命令 | 用途 |
|------|------|
| `lx smartsheet create` | 用 `CREATE TABLE` DDL 创建智能表格 |
| `lx smartsheet fetch` | 获取 schema、视图和摘要 |
| `lx smartsheet list-records` | 分页读取记录 |
| `lx smartsheet update-records` | 批量部分更新记录，单次最多 50 条 |
| `lx smartsheet update-schema` | 用 `ALTER TABLE` DDL 修改 schema |
| `lx smartsheet update-view` | 更新视图 |

同一 namespace 还提供字段、记录和视图的详细操作，包括 `create-field`、`create-records`、`delete-records`、`list-fields`、`list-views` 等。原有 `lx block smartsheet-*` 命令暂时保留兼容。

## 工作流

```text
1. 发现并读取服务端 block-view-dsl Resource
2. list 或 fetch，确认 entry_id、smartsheet_id 和当前 schema
3. 在统一的 lx smartsheet namespace 中选择高层或详细命令
4. 用 -d JSON 传递 schema、statements、records、field 或 view
5. 写入后重新 fetch/list-records 验证
```

不要根据通用 SQL 或旧示例猜测智能表格语法。创建/修改 schema、写记录或更新视图前先执行：

```bash
lx mcp resource list --format json
lx mcp resource read lexiang://docs/block-view-dsl/v0
```

若列表返回了更新版本的 `block-view-dsl` URI，应读取并使用列表中的当前 URI。

具体命令与示例见 [smartsheet.md](references/smartsheet.md)。

## 安全规则

1. 不猜测字段 ID、记录 ID 或视图 ID；先 fetch/list 获取。
2. 对象和数组参数统一使用 `-d '<JSON>'`，避免被 CLI 拆成字符串数组。
3. 更新记录是部分更新，只提交目标字段；单次最多 50 条。
4. 修改 schema 前先 fetch；`update-schema` 提交完整 `ALTER TABLE` 语句。
5. 页面正文中的智能表格块布局属于 `lx-block`，表格数据/schema 属于本 skill。
6. `--help` 中出现 Resource preflight 时，必须先执行其中的读取命令。
