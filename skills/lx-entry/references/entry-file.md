# entry file — 文件管理与上传

> **前置条件：** 先阅读 [`../SKILL.md`](../SKILL.md) 了解条目管理的整体决策树。

`lx file upload` 会自动完成 MCP apply、HTTP PUT 和 MCP commit，支持普通文件、图片、单 HTML、HTML bundle、视频和音频。

## 上传新文件

```bash
lx file upload ./report.pdf --parent-entry-id folder_xxx
lx file upload ./demo.mp4 --parent-entry-id folder_xxx
lx file upload ./voice.mp3 --parent-entry-id folder_xxx
```

CLI 自动读取文件名、大小、MIME 和语义扩展。视频/音频仍走相同公共命令，存储后端由服务端选择。

## HTML 的四种输入方式

```bash
# 1. 导入为可编辑在线页面
lx entry import html ./index.html --parent-id folder_xxx

# 2. HTML 物料目录自动压缩并上传为 bundle
lx entry import html ./site --dir --parent-id folder_xxx

# 3. 上传单 HTML 文件条目
lx file upload ./index.html --parent-entry-id folder_xxx

# 4. 上传预制 HTML bundle；ZIP 根应包含 index.html
lx file upload ./site.zip --parent-entry-id folder_xxx --html-bundle
```

目录模式递归包含普通文件，不额外包一层目录；拒绝符号链接，并要求目录根存在 `index.html`。默认远端文件名为 `<目录名>.zip`，可用 `--name` 覆盖。

普通 ZIP 不加 `--html-bundle`：

```bash
lx file upload ./archive.zip --parent-entry-id folder_xxx
```

HTML 单文件和 bundle 受服务端 10 MiB 上限约束。

## 更新已有文件

```bash
# target_id 即 file_id
lx entry describe-entry --entry-id file_entry_xxx

lx file upload ./report-v2.pdf \
  --parent-entry-id file_entry_xxx \
  --file-id file_xxx
```

更新时 `--parent-entry-id` 必须是当前文件条目自己的 entry ID，不是父文件夹。

## 高级覆盖

只有自动推断不正确时才显式覆盖：

```bash
lx file upload ./artifact.bin \
  --parent-entry-id folder_xxx \
  --name artifact.pdf \
  --mime-type application/pdf \
  --extension pdf
```

`extension` 表示知识条目业务语义，不是存储后端。不要传 `filesystem`、VOD session key 或 `vod_file_id`。

## 底层三步流程（调试用）

```bash
lx file apply-upload \
  --parent-entry-id folder_xxx \
  --name report.pdf \
  --size 12345 \
  --mime-type application/pdf \
  --extension pdf \
  --upload-type PRE_SIGNED_URL

curl -X PUT "{upload_url}" --data-binary @./report.pdf
lx file commit-upload --session-id sess_xxx
```

手工路径缺一步都会失败；媒体会话还必须携带 apply 返回的 headers/auth，所以优先使用 `lx file upload`。

## 其他文件操作

```bash
lx file download-file --file-id file_xxx
lx file list-revisions --file-id file_xxx
lx file revert-file --file-id file_xxx --revision-id rev_xxx
lx file create-hyperlink --url "https://..." --parent-entry-id folder_xxx
```

版本恢复是破坏性操作，执行前必须确认。

## 详细参数

```bash
lx file upload --help
lx file apply-upload --help
lx file commit-upload --help
```
