use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub output: OutputConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(default = "default_mcp_url")]
    pub url: String,
    #[serde(default)]
    pub access_token: Option<String>,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            url: default_mcp_url(),
            access_token: None,
        }
    }
}

fn default_mcp_url() -> String {
    "https://mcp.lexiang-app.com/mcp".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OutputConfig {
    #[serde(default)]
    pub format: Option<String>,
}

impl Config {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("lx")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        let mut config = if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: Config = serde_json::from_str(&content)?;
            config
        } else {
            Self::default()
        };

        // 动态命令和本地增强命令会在主 Clap 解析之前加载配置，
        // 因此环境 token 必须在这里注入，不能只依赖 Cli::parse()。
        apply_access_token_override(&mut config, std::env::var("LX_ACCESS_TOKEN").ok());

        // NOTE: 不在此处加载 TokenStore 中的 token。
        // Config::load() 是同步的，无法执行 async 的 token 刷新操作。
        // 所有需要 access_token 的地方统一使用 auth::get_access_token() 获取，
        // 该函数会自动处理过期检查和 refresh_token 刷新。

        Ok(config)
    }

    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir)?;

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(Self::config_path(), content)?;
        Ok(())
    }
}

fn apply_access_token_override(config: &mut Config, token: Option<String>) {
    if let Some(token) = token.filter(|value| !value.trim().is_empty()) {
        config.mcp.access_token = Some(token);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_token_overrides_file_configuration() {
        let mut config = Config::default();
        config.mcp.access_token = Some("file-token".to_string());

        apply_access_token_override(&mut config, Some("environment-token".to_string()));

        assert_eq!(
            config.mcp.access_token.as_deref(),
            Some("environment-token")
        );
    }

    #[test]
    fn empty_environment_token_does_not_clear_configuration() {
        let mut config = Config::default();
        config.mcp.access_token = Some("file-token".to_string());

        apply_access_token_override(&mut config, Some("   ".to_string()));

        assert_eq!(config.mcp.access_token.as_deref(), Some("file-token"));
    }
}
