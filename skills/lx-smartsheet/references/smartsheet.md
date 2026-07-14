# 智能表格命令参考

## 获取当前 DSL 和数据契约

智能表格的 schema DDL、记录字段和值类型、视图配置由服务端 Resource 定义。执行写操作前先读取，不要把普通 SQL 语法当成这里的完整契约：

```bash
lx mcp resource list --format json
lx mcp resource read lexiang://docs/block-view-dsl/v0
```

如果列表中的 `block-view-dsl` URI 版本不同，以列表返回值为准。

## 发现与读取

```bash
lx smartsheet list --entry-id <ENTRY_ID>
lx smartsheet fetch --entry-id <ENTRY_ID> --smartsheet-id <SMARTSHEET_ID>
lx smartsheet list-records -d '{
  "entry_id": "<ENTRY_ID>",
  "smartsheet_id": "<SMARTSHEET_ID>",
  "page_size": 50
}'
```

继续分页时，把上次响应的 `page_token` 原样传回。

## 创建与修改 schema

`schema` 是完整 `CREATE TABLE` SQL DDL。创建位置由 `target_type` 以及对应的 parent 参数决定；执行前以命令帮助确认服务端当前字段：

```bash
lx smartsheet create --help
lx smartsheet create -d '{
  "target_type": "kb_entry",
  "parent_entry_id": "<PARENT_ENTRY_ID>",
  "title": "项目跟踪",
  "schema": "CREATE TABLE ..."
}'
```

创建页面内嵌表格时使用 `target_type=block`，并传入页面 `entry_id`；可选 `parent_block_id` 和 `index` 控制插入位置。

修改时提交一条或多条完整 `ALTER TABLE` 语句：

```bash
lx smartsheet update-schema -d '{
  "entry_id": "<ENTRY_ID>",
  "smartsheet_id": "<SMARTSHEET_ID>",
  "statements": ["ALTER TABLE ..."]
}'
```

DDL 的具体字段类型与语法以 `block-view-dsl` Resource、`smartsheet fetch` 返回的 schema 和 `--help` 为准。

## 更新记录和视图

复杂 payload 使用 JSON；先 fetch/list 获取真实 ID，不要自行构造：

```bash
lx smartsheet update-records -d '{
  "entry_id": "<ENTRY_ID>",
  "smartsheet_id": "<SMARTSHEET_ID>",
  "records": [
    {"record_id": "<RECORD_ID>", "fields": {"<FIELD_ID>": "新值"}}
  ]
}'

lx smartsheet update-view -d '{
  "entry_id": "<ENTRY_ID>",
  "smartsheet_id": "<SMARTSHEET_ID>",
  "view_id": "<VIEW_ID>",
  "view": {"...": "以 fetch/list-views 返回结构为准"}
}'
```

若高层命令不能表达所需操作，查看详细命名空间：

```bash
lx smartsheet --help
lx smartsheet create-records --help
lx smartsheet create-field --help
lx smartsheet delete-view --help
```
