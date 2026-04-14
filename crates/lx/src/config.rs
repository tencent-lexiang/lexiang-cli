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
        let config = if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: Config = serde_json::from_str(&content)?;
            config
        } else {
            Self::default()
        };

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
