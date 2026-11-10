use chrono::Local;
use super::tool::{Tool, ToolError};

/// Tool that returns the local time zone offset as a string.
/// Example output: "+02:00" or "-04:00".
pub struct GetLocalTimeZoneTool;

impl Tool for GetLocalTimeZoneTool {
    fn name(&self) -> &'static str {
        "get_local_timezone"
    }

    fn description(&self) -> &'static str {
        "Return the local time zone offset as ISO 8601 string."
    }

    fn run(&self, _input: &str, _params: &str) -> Result<String, ToolError> {
        let now = Local::now();
        // Use the offset part of the local datetime
        let offset_str = now.offset().to_string();
        Ok(offset_str)
    }
}

unsafe impl Sync for GetLocalTimeZoneTool {}
