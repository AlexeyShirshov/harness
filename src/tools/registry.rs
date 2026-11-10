use std::sync::{Mutex, Arc};
use lazy_static::lazy_static;
use crate::tools::tool::Tool;

lazy_static! {
    static ref TOOLS: Mutex<Vec<Arc<dyn Tool + Send + Sync>>> = Mutex::new(Vec::new());
}

/// Register a new tool.
pub fn register_tool(tool: Arc<dyn Tool + Send + Sync>) {
    TOOLS.lock().unwrap().push(tool);
}

/// Retrieve a tool by name. Returns None if not found.
pub fn get_tool(name: &str) -> Option<Arc<dyn Tool + Send + Sync>> {
    for t in TOOLS.lock().unwrap().iter() {
        if t.name() == name {
            return Some(t.clone());
        }
    }
    None
}

/// Convenience to list all tool names.
pub fn list_tool_names() -> Vec<String> {
    TOOLS
        .lock()
        .unwrap()
        .iter()
        .map(|t| t.name().to_string())
        .collect()
}
