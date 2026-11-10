use chrono::Utc;
use super::tool::{Tool, ToolError};

/// Tool that returns the current UTC time as an ISO‑8601 string.
pub struct GetTimeTool;

impl Tool for GetTimeTool {
    fn name(&self) -> &'static str {
        "get_time"
    }

    fn description(&self) -> &'static str {
        "Return the current UTC time as ISO 8601 string."
    }

    fn run(&self, _input: &str, _params: &str) -> Result<String, ToolError> {
        let now = Utc::now().to_rfc3339();
        Ok(now)
    }
}

unsafe impl Sync for GetTimeTool {}
