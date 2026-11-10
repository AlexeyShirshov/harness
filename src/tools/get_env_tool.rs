use super::tool::{Tool, ToolError};

/// Instrument for getting environment variables.
pub struct GetEnvTool;

impl Tool for GetEnvTool {
    fn name(&self) -> &'static str {
        "get_env"
    }

    fn description(&self) -> &'static str {
        "Returns all system environment variables as key-value pairs."
    }

    fn run(&self, _input: &str, _params: &str) -> Result<String, ToolError> {
        // Получаем переменные окружения - синхронно без async
        let env_vars = std::env::vars().collect::<Vec<_>>();

        if env_vars.is_empty() {
            return Ok("# Environment Variables\n(No environment variables found)\n".to_string());
        }

        // Форматируем в понятном виде
        let output = env_vars.iter()
            .map(|(k, v)| format!("{}:\t{}\n", k, 
                if v.len() > 200 {
                    format!("{}...", &v[0..197])
                } else {
                    v.clone()
                }
            ))
            .collect::<Vec<_>>()
            .join("");

        Ok(format!("# Environment Variables\n\n{}", output))
    }
}
