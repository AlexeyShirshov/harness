pub trait Tool: Send + Sync {
    /// Name of the tool – used by the model to refer to it.
    fn name(&self) -> &'static str;
    /// Short description for the system prompt.
    fn description(&self) -> &'static str;
    /// Execute the tool.
    /// `input` – JSON string of *arguments* supplied by the model.
    /// `params` – optional JSON string of additional parameters.
    fn run(&self, input: &str, params: &str) -> Result<String, ToolError>;
}

/// Simple error type used by tools.
#[derive(Debug)]
pub struct ToolError(pub String);

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for ToolError {}
