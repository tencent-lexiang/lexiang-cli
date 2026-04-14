pub mod block;
pub mod cli;
pub mod dynamic;
pub mod git;
pub mod mcp;
pub mod output;
pub mod sh;
pub mod skill;
pub mod tools;
pub mod ui;
pub mod update;
pub mod utils;

pub use block::try_handle_block_command;
pub use cli::{Cli, Commands, McpCommands, SkillCommands, ToolsCommands, UpdateCommands};
pub use dynamic::{handle_dynamic_command, print_help_with_dynamic_commands};
pub use git::handle_git_command;
pub use mcp::{call_tool, list_tools};
#[allow(unused_imports)]
pub use sh::{build_shell, exec_command, start_repl};
pub use skill::{handle_generate, handle_install, handle_status, handle_uninstall, handle_update};
pub use tools::{
    handle_categories, handle_list, handle_schema, handle_sync, handle_sync_embedded,
    handle_sync_unlisted, handle_version,
};
pub use update::{
    auto_check_update, handle_check as handle_update_check, handle_list as handle_update_list,
};

use crate::mcp::schema::McpSchemaCollection;

pub fn load_schema() -> Option<McpSchemaCollection> {
    use crate::mcp::schema::embedded::load_embedded_collection;
    use crate::mcp::schema::RuntimeSchemaManager;

    let runtime = RuntimeSchemaManager::new();
    let mut schema = runtime.load().ok().flatten();

    // 如果运行时 schema 不存在，使用 embedded schema
    if schema.is_none() {
        return load_embedded_collection();
    }

    // 运行时 schema 中的 tool 可能缺少 inputSchema（为 null），
    // 用 embedded schema 补充缺失的参数定义
    if let Some(ref mut collection) = schema {
        if let Some(embedded) = load_embedded_collection() {
            for (name, tool) in collection.tools.iter_mut() {
                if tool.input_schema.is_none() {
                    if let Some(embedded_tool) = embedded.tools.get(name) {
                        tool.input_schema = embedded_tool.input_schema.clone();
                    }
                }
            }
        }
    }

    schema
}
