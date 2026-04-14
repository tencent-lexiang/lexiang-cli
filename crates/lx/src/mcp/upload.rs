//! 文件上传功能（预签名 URL 方式）

use super::McpClient;
use anyhow::Result;
use std::path::Path;

/// 上传配置
#[derive(Debug, Clone, Default)]
pub struct UploadConfig {
    /// 文件 ID（更新已有文件时使用）
    pub file_id: Option<String>,
    /// 父节点 entry ID
    pub parent_entry_id: String,
    /// 文件名（新建时可选，自动从路径提取）
    pub file_name: Option<String>,
    /// Content-Type（可选，自动检测）
    pub content_type: Option<String>,
}

impl McpClient {
    /// 上传文件到知识库
    pub async fn upload_file(&self, config: &UploadConfig, file_path: &Path) -> Result<String> {
        let file_content = std::fs::read(file_path)?;
        let file_name = config
            .file_name
            .clone()
            .or_else(|| {
                file_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
            })
            .ok_or_else(|| anyhow::anyhow!("Cannot determine file name"))?;

        let content_type = config
            .content_type
            .clone()
            .unwrap_or_else(|| guess_content_type(&file_name));

        self.upload_bytes(config, &file_content, &file_name, &content_type)
            .await
    }

    /// 上传字节数据到知识库
    pub async fn upload_bytes(
        &self,
        config: &UploadConfig,
        content: &[u8],
        file_name: &str,
        content_type: &str,
    ) -> Result<String> {
        // 1. Apply upload
        let mut args = serde_json::json!({
            "parent_entry_id": config.parent_entry_id,
            "upload_type": "PRE_SIGNED_URL",
            "name": file_name,
            "size": content.len(),
            "mime_type": content_type,
        });
        if let Some(file_id) = &config.file_id {
            args["file_id"] = serde_json::json!(file_id);
        }

        let apply_resp: serde_json::Value = self.call_raw("file_apply_upload", args).await?;
        let (session_id, upload_url) = extract_session(&apply_resp)?;

        // 2. HTTP PUT
        let http = reqwest::Client::new();
        let resp = http
            .put(&upload_url)
            .header("Content-Type", content_type)
            .body(content.to_vec())
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "Upload failed: {} - {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            );
        }

        // 3. Commit upload
        let commit_resp: serde_json::Value = self
            .call_raw(
                "file_commit_upload",
                serde_json::json!({ "session_id": session_id }),
            )
            .await?;

        extract_entry_id(&commit_resp)
    }
}

fn extract_session(r: &serde_json::Value) -> Result<(String, String)> {
    let sid = r
        .pointer("/data/session/session_id")
        .or_else(|| r.pointer("/data/session_id"))
        .or_else(|| r.pointer("/session/session_id"))
        .or_else(|| r.get("session_id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No session_id in response: {}",
                serde_json::to_string_pretty(r).unwrap_or_default()
            )
        })?;

    // upload_url 在 objects[0].upload_url 里
    let url = r
        .pointer("/data/session/objects/0/upload_url")
        .or_else(|| r.pointer("/data/session/upload_url"))
        .or_else(|| r.pointer("/data/upload_url"))
        .or_else(|| r.pointer("/session/objects/0/upload_url"))
        .or_else(|| r.pointer("/session/upload_url"))
        .or_else(|| r.get("upload_url"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No upload_url in response: {}",
                serde_json::to_string_pretty(r).unwrap_or_default()
            )
        })?;

    Ok((sid.to_string(), url.to_string()))
}

fn extract_entry_id(r: &serde_json::Value) -> Result<String> {
    r.pointer("/data/entry/id")
        .or_else(|| r.pointer("/data/id"))
        .or_else(|| r.pointer("/entry/id"))
        .or_else(|| r.get("id"))
        .and_then(|v| v.as_str())
        .map(std::string::ToString::to_string)
        .ok_or_else(|| anyhow::anyhow!("No entry_id in commit response"))
}

fn guess_content_type(name: &str) -> String {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "txt" => "text/plain",
        "md" => "text/markdown",
        "json" => "application/json",
        "xml" => "application/xml",
        "html" | "htm" => "text/html",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "zip" => "application/zip",
        "tar" => "application/x-tar",
        "gz" => "application/gzip",
        _ => "application/octet-stream",
    }
    .to_string()
}
