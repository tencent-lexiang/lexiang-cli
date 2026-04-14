//! GitHub Release 更新检查器

use anyhow::{Context, Result};
use serde::Deserialize;
use std::cmp::Ordering;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use crate::datadir::DataDir;
use crate::version;

/// GitHub Release 信息
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GitHubRelease {
    tag_name: String,
    name: Option<String>,
    html_url: String,
    published_at: String,
    prerelease: bool,
    draft: bool,
    body: Option<String>,
    assets: Vec<GitHubAsset>,
}

/// GitHub Release Asset
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

/// 检查更新的结果
#[derive(Debug, Clone)]
pub struct CheckResult {
    /// 当前版本
    pub current_version: String,
    /// 最新版本
    pub latest_version: String,
    /// 是否有更新
    pub has_update: bool,
    /// Release 页面 URL
    pub release_url: String,
    /// Release 说明
    pub release_notes: Option<String>,
    /// 当前平台的下载 URL（如果有）
    pub download_url: Option<String>,
}

/// 更新检查配置
#[derive(Debug, Clone)]
pub struct UpdateConfig {
    /// GitHub 仓库 owner
    pub owner: String,
    /// GitHub 仓库名
    pub repo: String,
    /// 是否包含预发布版本
    pub include_prerelease: bool,
    /// 检查间隔（秒）
    pub check_interval: Duration,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            owner: "nicholasniu".to_string(),
            repo: "lexiang-cli".to_string(),
            include_prerelease: false,
            check_interval: Duration::from_secs(24 * 60 * 60), // 24 小时
        }
    }
}

/// 更新检查器
pub struct UpdateChecker {
    config: UpdateConfig,
    client: reqwest::Client,
}

impl UpdateChecker {
    /// 创建新的更新检查器
    pub fn new() -> Self {
        Self::with_config(UpdateConfig::default())
    }

    /// 使用自定义配置创建更新检查器
    pub fn with_config(config: UpdateConfig) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(format!(
                "{}/{}",
                env!("CARGO_PKG_NAME"),
                version::current_version()
            ))
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self { config, client }
    }

    /// 获取当前版本
    pub fn current_version() -> &'static str {
        version::current_version()
    }

    /// 检查是否有新版本
    pub async fn check(&self) -> Result<CheckResult> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            self.config.owner, self.config.repo
        );

        let response = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .context("Failed to fetch release info")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "GitHub API error: {} {}",
                response.status().as_u16(),
                response.status().as_str()
            );
        }

        let release: GitHubRelease = response
            .json()
            .await
            .context("Failed to parse release info")?;

        // 跳过预发布和草稿
        if release.draft || (!self.config.include_prerelease && release.prerelease) {
            return Ok(CheckResult {
                current_version: Self::current_version().to_string(),
                latest_version: Self::current_version().to_string(),
                has_update: false,
                release_url: String::new(),
                release_notes: None,
                download_url: None,
            });
        }

        let latest_version = release.tag_name.trim_start_matches('v').to_string();
        let current_version = Self::current_version().to_string();
        let has_update = compare_versions(&current_version, &latest_version) == Ordering::Less;

        // 查找当前平台的下载 URL
        let download_url = find_platform_asset(&release.assets);

        Ok(CheckResult {
            current_version,
            latest_version,
            has_update,
            release_url: release.html_url,
            release_notes: release.body,
            download_url,
        })
    }

    /// 检查是否应该进行更新检查（基于时间间隔）
    pub fn should_check(&self) -> bool {
        let last_check = self.load_last_check_time();
        match last_check {
            Some(time) => SystemTime::now()
                .duration_since(time)
                .map(|d| d >= self.config.check_interval)
                .unwrap_or(true),
            None => true,
        }
    }

    /// 保存检查时间
    pub fn save_check_time(&self) -> Result<()> {
        let path = Self::check_time_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        std::fs::write(&path, timestamp.to_string())?;
        Ok(())
    }

    /// 加载上次检查时间
    fn load_last_check_time(&self) -> Option<SystemTime> {
        let path = Self::check_time_path();
        let content = std::fs::read_to_string(&path).ok()?;
        let timestamp: u64 = content.trim().parse().ok()?;
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(timestamp))
    }

    /// 检查时间文件路径
    fn check_time_path() -> PathBuf {
        DataDir::default().path().join("update_check_time")
    }

    /// 获取所有 releases（用于列出历史版本）
    pub async fn list_releases(&self, limit: usize) -> Result<Vec<CheckResult>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases?per_page={}",
            self.config.owner, self.config.repo, limit
        );

        let response = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .context("Failed to fetch releases")?;

        if !response.status().is_success() {
            anyhow::bail!("GitHub API error: {}", response.status());
        }

        let releases: Vec<GitHubRelease> = response.json().await?;
        let current = Self::current_version();

        Ok(releases
            .into_iter()
            .filter(|r| !r.draft && (self.config.include_prerelease || !r.prerelease))
            .map(|r| {
                let version = r.tag_name.trim_start_matches('v').to_string();
                let has_update = compare_versions(current, &version) == Ordering::Less;
                CheckResult {
                    current_version: current.to_string(),
                    latest_version: version,
                    has_update,
                    release_url: r.html_url,
                    release_notes: r.body,
                    download_url: find_platform_asset(&r.assets),
                }
            })
            .collect())
    }
}

impl Default for UpdateChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// 比较两个版本号
fn compare_versions(current: &str, latest: &str) -> Ordering {
    let parse_version = |v: &str| -> Vec<u64> {
        let stable_part = v.split_once('-').map_or(v, |(main, _)| main);
        stable_part
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect()
    };

    let current_parts = parse_version(current);
    let latest_parts = parse_version(latest);

    for (c, l) in current_parts.iter().zip(latest_parts.iter()) {
        match c.cmp(l) {
            Ordering::Equal => continue,
            other => return other,
        }
    }

    current_parts.len().cmp(&latest_parts.len())
}

/// 查找当前平台对应的下载资源
fn find_platform_asset(assets: &[GitHubAsset]) -> Option<String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    // 构建可能的文件名模式
    let patterns: Vec<String> = match (os, arch) {
        ("macos", "aarch64") => vec![
            "darwin-arm64".to_string(),
            "darwin-aarch64".to_string(),
            "macos-arm64".to_string(),
            "macos-aarch64".to_string(),
            "apple-darwin".to_string(),
        ],
        ("macos", "x86_64") => vec![
            "darwin-x86_64".to_string(),
            "darwin-amd64".to_string(),
            "macos-x86_64".to_string(),
            "macos-amd64".to_string(),
            "apple-darwin".to_string(),
        ],
        ("linux", "x86_64") => vec![
            "linux-x86_64".to_string(),
            "linux-amd64".to_string(),
            "linux-gnu".to_string(),
        ],
        ("linux", "aarch64") => vec!["linux-arm64".to_string(), "linux-aarch64".to_string()],
        ("windows", "x86_64") => vec![
            "windows-x86_64".to_string(),
            "windows-amd64".to_string(),
            "win64".to_string(),
            ".exe".to_string(),
        ],
        _ => vec![],
    };

    for asset in assets {
        let name_lower = asset.name.to_lowercase();
        for pattern in &patterns {
            if name_lower.contains(pattern) {
                return Some(asset.browser_download_url.clone());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_versions() {
        assert_eq!(compare_versions("1.0.0", "1.0.1"), Ordering::Less);
        assert_eq!(compare_versions("1.0.1", "1.0.0"), Ordering::Greater);
        assert_eq!(compare_versions("1.0.0", "1.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.0.0", "2.0.0"), Ordering::Less);
        assert_eq!(compare_versions("1.9.0", "1.10.0"), Ordering::Less);
        assert_eq!(compare_versions("0.1.0", "0.1.0"), Ordering::Equal);
    }

    #[test]
    fn test_compare_versions_with_prerelease() {
        // 预发布版本只比较主版本号部分
        assert_eq!(compare_versions("1.0.0-beta.1", "1.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.0.0-alpha", "1.0.1"), Ordering::Less);
    }
}
