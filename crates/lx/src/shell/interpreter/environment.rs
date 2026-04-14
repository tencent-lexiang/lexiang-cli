//! Shell 环境: 变量表、当前工作目录、PATH 等

use std::collections::HashMap;

/// Shell 执行环境
#[derive(Debug, Clone)]
pub struct Environment {
    /// 环境变量
    variables: HashMap<String, String>,
    /// 当前工作目录
    cwd: String,
    /// 上一个命令的退出码
    last_exit_code: i32,
}

impl Environment {
    pub fn new(cwd: &str) -> Self {
        let mut variables = HashMap::new();
        variables.insert("HOME".to_string(), "/".to_string());
        variables.insert("PWD".to_string(), cwd.to_string());
        variables.insert("?".to_string(), "0".to_string());

        Self {
            variables,
            cwd: cwd.to_string(),
            last_exit_code: 0,
        }
    }

    /// 获取当前工作目录
    pub fn cwd(&self) -> &str {
        &self.cwd
    }

    /// 设置当前工作目录
    pub fn set_cwd(&mut self, cwd: &str) {
        self.cwd = cwd.to_string();
        self.variables.insert("PWD".to_string(), cwd.to_string());
    }

    /// 获取环境变量
    pub fn get(&self, name: &str) -> Option<&str> {
        if name == "?" {
            return Some(if self.last_exit_code == 0 { "0" } else { "1" });
        }
        self.variables.get(name).map(std::string::String::as_str)
    }

    /// 设置环境变量
    pub fn set(&mut self, name: &str, value: &str) {
        self.variables.insert(name.to_string(), value.to_string());
    }

    /// 设置上一个命令的退出码
    pub fn set_exit_code(&mut self, code: i32) {
        self.last_exit_code = code;
        self.variables.insert("?".to_string(), code.to_string());
    }

    /// 获取上一个命令的退出码
    pub fn last_exit_code(&self) -> i32 {
        self.last_exit_code
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new("/")
    }
}
