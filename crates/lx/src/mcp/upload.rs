//! 文件上传功能（预签名 URL 方式）

use super::McpClient;
use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
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
    /// 知识条目语义扩展（如 html、zip、mp4）
    pub extension: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UploadTarget {
    session_id: String,
    upload_url: String,
    headers: HeaderMap,
}

impl McpClient {
    /// 上传文件到知识库，兼容旧调用方只获取 entry ID 的行为
    pub async fn upload_file(&self, config: &UploadConfig, file_path: &Path) -> Result<String> {
        let commit = self.upload_file_result(config, file_path).await?;
        extract_entry_id(&commit)
    }

    /// 上传文件并返回完整的 commit 响应
    pub async fn upload_file_result(
        &self,
        config: &UploadConfig,
        file_path: &Path,
    ) -> Result<serde_json::Value> {
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

        self.upload_bytes_result(config, &file_content, &file_name, &content_type)
            .await
    }

    /// 上传字节数据到知识库，兼容旧调用方只获取 entry ID 的行为
    #[allow(dead_code)]
    pub async fn upload_bytes(
        &self,
        config: &UploadConfig,
        content: &[u8],
        file_name: &str,
        content_type: &str,
    ) -> Result<String> {
        let commit = self
            .upload_bytes_result(config, content, file_name, content_type)
            .await?;
        extract_entry_id(&commit)
    }

    /// 上传字节数据并返回完整的 commit 响应
    pub async fn upload_bytes_result(
        &self,
        config: &UploadConfig,
        content: &[u8],
        file_name: &str,
        content_type: &str,
    ) -> Result<serde_json::Value> {
        // 1. Apply upload
        let args = build_apply_args(config, file_name, content_type, content.len());
        let apply_resp: serde_json::Value = self.call_raw("file_apply_upload", args).await?;
        let target = extract_upload_target(&apply_resp, content_type)?;

        // 2. HTTP PUT。失败时通过 ? 直接返回，绝不执行 commit。
        put_upload(&target, content).await?;

        // 3. Commit upload。普通文件、HTML、视频和音频都只回传 session_id。
        self.call_raw(
            "file_commit_upload",
            serde_json::json!({ "session_id": target.session_id }),
        )
        .await
    }
}

fn build_apply_args(
    config: &UploadConfig,
    file_name: &str,
    content_type: &str,
    size: usize,
) -> serde_json::Value {
    let mut args = serde_json::json!({
        "parent_entry_id": config.parent_entry_id,
        "upload_type": "PRE_SIGNED_URL",
        "name": file_name,
        "size": size,
        "mime_type": content_type,
    });
    if let Some(file_id) = &config.file_id {
        args["file_id"] = serde_json::json!(file_id);
    }
    if let Some(extension) = &config.extension {
        args["extension"] = serde_json::json!(extension.trim_start_matches('.'));
    }
    args
}

fn extract_upload_target(
    r: &serde_json::Value,
    fallback_content_type: &str,
) -> Result<UploadTarget> {
    if let Some(session) = r
        .pointer("/data/client_upload_session")
        .or_else(|| r.get("client_upload_session"))
    {
        return extract_client_upload_target(session, fallback_content_type);
    }

    let session = r
        .pointer("/data/session")
        .or_else(|| r.get("session"))
        .unwrap_or(r);
    let session_id = session
        .get("session_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No session_id in response: {}",
                serde_json::to_string_pretty(r).unwrap_or_default()
            )
        })?;
    let upload_url = session
        .pointer("/objects/0/upload_url")
        .or_else(|| session.get("upload_url"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No upload_url in response: {}",
                serde_json::to_string_pretty(r).unwrap_or_default()
            )
        })?;

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_str(fallback_content_type)?);

    Ok(UploadTarget {
        session_id: session_id.to_string(),
        upload_url: upload_url.to_string(),
        headers,
    })
}

fn extract_client_upload_target(
    session: &serde_json::Value,
    fallback_content_type: &str,
) -> Result<UploadTarget> {
    let session_id = session
        .get("session_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No session_id in client upload response"))?;
    let object = session
        .get("object")
        .ok_or_else(|| anyhow::anyhow!("No object in client upload response"))?;
    let upload_url = object
        .get("upload_url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No upload_url in client upload response"))?;

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_str(fallback_content_type)?);
    append_header_object(&mut headers, object.get("headers"), false)?;
    append_header_object(&mut headers, object.get("auth"), true)?;

    Ok(UploadTarget {
        session_id: session_id.to_string(),
        upload_url: upload_url.to_string(),
        headers,
    })
}

fn append_header_object(
    headers: &mut HeaderMap,
    object: Option<&serde_json::Value>,
    auth: bool,
) -> Result<()> {
    let Some(values) = object.and_then(serde_json::Value::as_object) else {
        return Ok(());
    };

    for (name, value) in values {
        let Some(value) = value.as_str() else {
            continue;
        };
        let normalized = if auth {
            auth_header_name(name)
        } else {
            Some(name.as_str())
        };
        let Some(normalized) = normalized else {
            continue;
        };
        let name = HeaderName::from_bytes(normalized.as_bytes())
            .with_context(|| format!("Invalid upload header name: {normalized}"))?;
        let value = HeaderValue::from_str(value)
            .with_context(|| format!("Invalid upload header value for {normalized}"))?;
        headers.insert(name, value);
    }
    Ok(())
}

fn auth_header_name(name: &str) -> Option<&str> {
    if name.eq_ignore_ascii_case("Authorization") {
        Some("authorization")
    } else if name.eq_ignore_ascii_case("XCosSecurityToken")
        || name.eq_ignore_ascii_case("x-cos-security-token")
    {
        Some("x-cos-security-token")
    } else if name.contains('-') {
        Some(name)
    } else {
        None
    }
}

async fn put_upload(target: &UploadTarget, content: &[u8]) -> Result<()> {
    let response = reqwest::Client::new()
        .put(&target.upload_url)
        .headers(target.headers.clone())
        .body(content.to_vec())
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Upload failed: {} - {}",
            response.status(),
            response.text().await.unwrap_or_default()
        );
    }
    Ok(())
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

pub fn guess_content_type(name: &str) -> String {
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
        "md" | "markdown" => "text/markdown",
        "csv" => "text/csv",
        "json" => "application/json",
        "xml" => "application/xml",
        "html" | "htm" => "text/html",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "wav" => "audio/wav",
        "ogg" | "oga" => "audio/ogg",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "mpeg" | "mpg" => "video/mpeg",
        "zip" => "application/zip",
        "tar" => "application/x-tar",
        "gz" => "application/gzip",
        "7z" => "application/x-7z-compressed",
        "rar" => "application/vnd.rar",
        _ => "application/octet-stream",
    }
    .to_string()
}

pub fn semantic_extension(name: &str) -> Option<String> {
    Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_lowercase)
        .filter(|ext| !ext.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::routing::{post, put};
    use axum::{Json, Router};
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct MockState {
        upload_url: String,
        tool_calls: Arc<Mutex<Vec<String>>>,
    }

    async fn mock_mcp(
        State(state): State<MockState>,
        Json(request): Json<serde_json::Value>,
    ) -> Json<serde_json::Value> {
        let tool_name = request
            .pointer("/params/name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        state.tool_calls.lock().unwrap().push(tool_name.clone());

        let payload = if tool_name == "file_apply_upload" {
            serde_json::json!({
                "data": {
                    "session": {
                        "session_id": "session-1",
                        "objects": [{"upload_url": state.upload_url}]
                    }
                }
            })
        } else {
            serde_json::json!({"data": {"entry": {"id": "entry-1"}}})
        };
        Json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": request["id"],
            "result": {
                "content": [{"type": "text", "text": payload.to_string()}]
            }
        }))
    }

    async fn reject_upload() -> (StatusCode, &'static str) {
        (StatusCode::INTERNAL_SERVER_ERROR, "mock upload failure")
    }

    #[test]
    fn build_apply_args_includes_semantic_extension() {
        let config = UploadConfig {
            file_id: Some("file-1".to_string()),
            parent_entry_id: "entry-1".to_string(),
            file_name: None,
            content_type: None,
            extension: Some(".html".to_string()),
        };

        let args = build_apply_args(&config, "site.zip", "application/zip", 42);

        assert_eq!(args["extension"], "html");
        assert_eq!(args["file_id"], "file-1");
        assert_eq!(args["size"], 42);
        assert_eq!(args["upload_type"], "PRE_SIGNED_URL");
    }

    #[test]
    fn extracts_ordinary_upload_session() {
        let response = serde_json::json!({
            "data": {
                "session": {
                    "session_id": "session-1",
                    "objects": [{"upload_url": "https://upload.example/file"}]
                }
            }
        });

        let target = extract_upload_target(&response, "application/pdf").unwrap();

        assert_eq!(target.session_id, "session-1");
        assert_eq!(target.upload_url, "https://upload.example/file");
        assert_eq!(target.headers[CONTENT_TYPE], "application/pdf");
    }

    #[test]
    fn html_bundle_keeps_zip_transport_and_html_semantics() {
        let config = UploadConfig {
            parent_entry_id: "parent-1".to_string(),
            extension: Some("html".to_string()),
            ..UploadConfig::default()
        };
        let response = serde_json::json!({
            "data": {
                "session": {
                    "session_id": "html-session",
                    "objects": [{"upload_url": "https://upload.example/site"}]
                }
            }
        });

        let args = build_apply_args(&config, "site.zip", "application/zip", 512);
        let target = extract_upload_target(&response, "application/zip").unwrap();

        assert_eq!(args["mime_type"], "application/zip");
        assert_eq!(args["extension"], "html");
        assert_eq!(target.session_id, "html-session");
        assert_eq!(target.headers[CONTENT_TYPE], "application/zip");
    }

    #[test]
    fn extracts_media_session_and_upload_headers() {
        let response = serde_json::json!({
            "data": {
                "client_upload_session": {
                    "session_id": "media-session",
                    "object": {
                        "upload_url": "https://upload.example/video",
                        "headers": {
                            "Content-Type": "application/octet-stream",
                            "Content-Disposition": "attachment; filename=video.mp4"
                        },
                        "auth": {
                            "Authorization": "signed-value",
                            "XCosSecurityToken": "temporary-token",
                            "URL": "not-an-http-header"
                        }
                    }
                }
            }
        });

        let target = extract_upload_target(&response, "video/mp4").unwrap();

        assert_eq!(target.session_id, "media-session");
        assert_eq!(target.headers[CONTENT_TYPE], "application/octet-stream");
        assert_eq!(target.headers["authorization"], "signed-value");
        assert_eq!(target.headers["x-cos-security-token"], "temporary-token");
        assert!(!target.headers.contains_key("url"));
    }

    #[test]
    fn rejects_apply_response_without_upload_url() {
        let response = serde_json::json!({
            "data": {"session": {"session_id": "session-1", "objects": []}}
        });

        let error = extract_upload_target(&response, "application/pdf").unwrap_err();

        assert!(error.to_string().contains("No upload_url"));
    }

    #[test]
    fn infers_common_media_and_semantic_extensions() {
        assert_eq!(guess_content_type("video.mov"), "video/quicktime");
        assert_eq!(guess_content_type("voice.m4a"), "audio/mp4");
        assert_eq!(semantic_extension("SITE.HTML").as_deref(), Some("html"));
    }

    #[tokio::test]
    async fn put_failure_does_not_commit_upload_session() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let state = MockState {
            upload_url: format!("http://{address}/upload"),
            tool_calls: Arc::clone(&calls),
        };
        let app = Router::new()
            .route("/mcp", post(mock_mcp))
            .route("/upload", put(reject_upload))
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = McpClient::new(format!("http://{address}/mcp"), None).unwrap();
        let config = UploadConfig {
            parent_entry_id: "parent-1".to_string(),
            extension: Some("pdf".to_string()),
            ..UploadConfig::default()
        };
        let error = client
            .upload_bytes_result(&config, b"content", "report.pdf", "application/pdf")
            .await
            .unwrap_err();

        assert!(error.to_string().contains("500 Internal Server Error"));
        assert_eq!(calls.lock().unwrap().as_slice(), ["file_apply_upload"]);
        server.abort();
    }
}
