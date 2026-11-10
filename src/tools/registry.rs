use std::sync::{Arc, Mutex, OnceLock};

use crate::tools::tool;
use crate::tools::get_time_tool::GetTimeTool;
use crate::tools::get_local_timezone_tool::GetLocalTimeZoneTool;
use crate::tools::get_env_tool::GetEnvTool;

static TOOLS: OnceLock<Mutex<Vec<Arc<dyn tool::Tool + Send + Sync>>>> = OnceLock::new();

fn init_tools() -> Mutex<Vec<Arc<dyn tool::Tool + Send + Sync>>> {
    let env = Arc::new(GetEnvTool) as Arc<dyn tool::Tool + Send + Sync>;
    let time_tool = Arc::new(GetTimeTool) as Arc<dyn tool::Tool + Send + Sync>;
    let tz_tool = Arc::new(GetLocalTimeZoneTool) as Arc<dyn tool::Tool + Send + Sync>;

    Mutex::new(vec![env, time_tool, tz_tool])
}

/// Get a tool by name.
pub fn get_tool(name: &str) -> Option<Arc<dyn tool::Tool + Send + Sync>> {
    let tools = TOOLS.get_or_init(init_tools);
    tools.lock().unwrap().iter().find(|t| t.name() == name).cloned()
}

/// List all registered tool names.
pub fn list_tool_names() -> Vec<String> {
    let tools = TOOLS.get_or_init(init_tools);
    tools.lock().unwrap().iter().map(|t| t.name().to_string()).collect()
}