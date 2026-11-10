use serde::{Deserialize, Serialize};
use crate::tools::registry::get_tool;
use reqwest::blocking::Client;
use std::io::BufRead;
use std::str::FromStr;

fn generate_system_prompt() -> String {
    let mut desc = String::from("You have the following tools available:\n");
    for name in crate::tools::registry::list_tool_names() {
        if let Some(tool) = crate::tools::registry::get_tool(&name) {
            desc.push_str(&format!("- **{}**: {}\n", tool.name(), tool.description()));
        }
    }
    desc.push_str(
        "\nWhen the user asks for local time, call get_time and get_local_timezone, \
         then combine the UTC time with the returned offset and answer with an ISO 8601 \
         local timestamp.\n",
    );
    desc
}

/// Build the Ollama `tools` payload. Without this the model only sees the
/// tool names in the system prompt and will not emit structured tool calls.
fn tool_definitions() -> Vec<serde_json::Value> {
    crate::tools::registry::list_tool_names()
        .into_iter()
        .filter_map(|name| crate::tools::registry::get_tool(&name))
        .map(|tool| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": tool.name(),
                    "description": tool.description(),
                    "parameters": {
                        "type": "object",
                        "properties": {}
                    }
                }
            })
        })
        .collect()
}

/// Return the current local time as an ISO 8601 string by combining the UTC
/// time from `get_time` with the offset from `get_local_timezone`.
fn current_local_time() -> Result<String, Box<dyn std::error::Error>> {
    let utc_tool = get_tool("get_time").ok_or("get_time tool is not registered")?;
    let tz_tool =
        get_tool("get_local_timezone").ok_or("get_local_timezone tool is not registered")?;

    let utc_str = utc_tool.run("", "")?;
    let tz_str = tz_tool.run("", "")?;

    let utc = chrono::DateTime::parse_from_rfc3339(utc_str.trim())?;
    let offset = chrono::FixedOffset::from_str(tz_str.trim())?;

    Ok(utc.with_timezone(&offset).to_rfc3339())
}


/// Message structure
#[derive(Debug, Clone, Deserialize, Serialize)] // Added Serialize for JSON serialization
pub struct Message {
    pub role: String,
    #[serde(default)]
    pub content: String,
}

/// Chat request to Ollama API
#[derive(Debug, Deserialize, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub stream: bool,
    pub tools: Vec<serde_json::Value>,
}

struct ChatApplication {
    client: Client,
}

impl ChatApplication {
    fn new() -> Self {
        Self {
            client: Client::builder().timeout(std::time::Duration::from_secs(60)).build().expect("Failed to build client"),
        }
    }

    /// Generates response to Ollama
    pub fn generate(&self, model: &str, prompt: &str) -> Result<Vec<Message>, Box<dyn std::error::Error>> {
        println!("Sending request to Ollama API...");
        
        let response = self.client.post("http://localhost:11434/api/generate")
            .json(&serde_json::json!({
                "model": model,
                "prompt": prompt,
                "stream": false
            }))
            .send()?;

        if !response.status().is_success() {
            return Err(format!("HTTP Error: {}", response.status()).into());
        }

        let text = response.text()?;
        
        println!("Response from server:\n{}", text);
        
        if let Ok(result) = serde_json::from_str::<serde_json::Value>(&text) {
            match result.get("response")
                .and_then(|r| r.as_object())
                .and_then(|r| r.get("message"))
                .and_then(|m| m.as_object()) {
                
                Some(msg_obj) => {
                    let content = msg_obj.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string();
                    let role = msg_obj.get("role").and_then(|r| r.as_str()).unwrap_or("assistant");

                    Ok(vec![
                        Message{role: "user".to_string(), content: prompt.to_string()},
                        Message{role: role.to_string(), content},
                    ])
                }
                
                None => {
                    eprintln!("⚠ Response does not contain response.message field");
                    Ok(vec![Message{role: "user".to_string(), content: prompt.to_string()}])
                }
            }
        } else {
            Err(format!("Failed to parse JSON response: {}", text).into())
        }
    }

    /// Sends chat request via /api/chat
    pub fn chat_call(&self, model: &str, prompt: &str) -> Result<Vec<Message>, Box<dyn std::error::Error>> {
        println!("Sending chat request to /api/chat...");
        
        let messages = vec![
            Message{ role: "system".to_string(), content: generate_system_prompt() },
            Message{ role: "user".to_string(), content: prompt.to_string() },
        ];

        let request = ChatRequest {
            model: model.to_string(),
            messages,
            stream: false,
            tools: tool_definitions(),
        };

        let response = self.client.post("http://localhost:11434/api/chat")
            .json(&request)
            .send()?;

        if !response.status().is_success() {
            return Err(format!("HTTP Error: {}", response.status()).into());
        }

        let text = response.text()?;
        
        println!("Chat response:\n{}", text);

        // Parse chat response using serde_json::Value to extract message
        if let Ok(result) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(msg_obj) = result.get("message").and_then(|m| m.as_object()) {
                let role = msg_obj.get("role").and_then(|r| r.as_str()).unwrap_or("assistant");
                // Build assistant content
                let mut assistant_content = String::new();
                let mut tool_results = Vec::new();
                let mut local_time_requested = false;

                if let Some(tool_calls) = msg_obj.get("tool_calls").and_then(|t| t.as_array()) {
                    for tool_call in tool_calls {
                        if let Some(func) = tool_call.get("function") {
                            if let Some(name) = func.get("name").and_then(|n| n.as_str()) {
                                if name == "get_time" || name == "get_local_timezone" {
                                    local_time_requested = true;
                                }
                                if let Some(tool) = get_tool(name) {
                                    match tool.run("", "") {
                                        Ok(res) => tool_results.push(res),
                                        Err(err) => {
                                            tool_results.push(format!("**Tool error**: {}", err))
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                let prompt_lower = prompt.to_lowercase();
                let prompt_wants_local_time = prompt_lower.contains("local time")
                    || prompt_lower.contains("местн")
                    || prompt_lower.contains("локальн");

                if local_time_requested || prompt_wants_local_time {
                    match current_local_time() {
                        Ok(local) => assistant_content = format!("Local time: {}", local),
                        Err(err) => assistant_content = format!("**Tool error**: {}", err),
                    }
                } else if !tool_results.is_empty() {
                    assistant_content = tool_results.join("\n");
                }

                if assistant_content.is_empty() {
                    assistant_content = msg_obj.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string();
                }
                Ok(vec![
                    Message{role: "user".to_string(), content: prompt.to_string()},
                    Message{role: role.to_string(), content: assistant_content},
                ])
            } else {
                eprintln!("⚠ Chat response does not contain message field");
                Err("Chat response missing message".into())
            }
        } else {
            Err(format!("Failed to parse chat JSON response: {}", text).into())
        }
    }

    ///Makes a quick chat request using model from list and user input
    pub fn quick_chat(&self, _prompt: &str) -> Result<Vec<Message>, Box<dyn std::error::Error>> {
        // Get prompt from user input
        println!("\n📝 Введите ваш запрос:");
        
        let stdin = std::io::stdin();
        let mut stdin_handle = stdin.lock();
        let mut user_input = String::new();
        stdin_handle.read_line(&mut user_input)?;
        
        let user_prompt = user_input.trim();
        
        if user_prompt.is_empty() {
            println!("❌ Запрос не был выполнен (пустой ввод)");
            return Ok(vec![]);
        }
        
        println!("✅ Ваш запрос: {}", user_prompt);
        println!("🔍 Fetching model list from Ollama...");
        
        let models_response = self.client.get("http://localhost:11434/api/tags").send()?;
        
        if models_response.status().is_success() {
            let json_text = models_response.text()?;
            
            // Parse available models and use the first one
            let models_value: serde_json::Value = serde_json::from_str(&json_text)?;
            if let Some(models_arr) = models_value.get("models").and_then(|m| m.as_array()) {
                if !models_arr.is_empty() {
                    // Prefer the specific 20b-48k model if available
                let target_name = models_arr
                    .iter()
                    .find(|m| {
                        m.get("name")
                            .and_then(|n| n.as_str())
                            .map(|s| s == "gpt-oss:20b-48k")
                            .unwrap_or(false)
                    })
                    .and_then(|m| m.get("name").and_then(|n| n.as_str()))
                    .unwrap_or_else(|| {
                        // Default to first model in the list
                        models_arr[0]
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("gpt-oss:20b-48k")
                    });
                    Ok(self.chat_call(target_name, user_prompt)?)
                } else {
                    // Fallback to default model if no models available
                    println!("⚠ Нет доступных моделей, используем по умолчанию...");
                    Ok(self.chat_call("gpt-oss:20b-48k", user_prompt)?)
                }
            } else {
                Ok(self.chat_call("gpt-oss:20b-48k", user_prompt)?)
            }
        } else {
            Err(format!("Connection error to Ollama: {:?}", models_response.status()).into())
        }
    }

    /// Gets list of available models from Ollama
    pub fn list_models(&self) -> Result<Vec<LocalModel>, Box<dyn std::error::Error>> {
        let response = self.client.get("http://localhost:11434/api/tags").send()?;

        if response.status().is_success() {
            let json_text = response.text()?;
            
            // Simply parse and extract model names from JSON response
            match serde_json::from_str::<Vec<LocalModel>>(&json_text) {
                Ok(models) => Ok(models),
                Err(_) => {
                    // If direct parsing fails, try manual extraction
                    let models_value: Result<serde_json::Value, _> = serde_json::from_str(&json_text);
                    match models_value {
                        Ok(value) => {
                            if let Some(models_val) = value.get("models") {
                                if let Some(arr) = models_val.as_array() {
                                    let mut result = Vec::new();
                                    for m in arr.iter() {
                                        if let Some(name_str) = m.as_object()
                                            .and_then(|obj| obj.get("name").and_then(|n| n.as_str())) {
                                            result.push(LocalModel{name: name_str.to_string()});
                                        }
                                    }
                                    Ok(result)
                                } else {
                                    Ok(vec![])
                                }
                            } else {
                                Ok(vec![])
                            }
                        },
                        Err(_) => Err("Invalid JSON response from Ollama".into()),
                    }
                }
            }
        } else {
            Err(format!("API error: {:?}", response.status()).into())
        }
    }
}

/// Local model with name from Ollama list
#[derive(Debug, Clone, Deserialize)] // Added Deserialize for JSON parsing
pub struct LocalModel {
    pub name: String,
}

/// Entry point for the chat application
pub fn run_chat_app() -> Result<(), Box<dyn std::error::Error>> {
    println!("🦙 Ollama Local Connection 🦙");
    println!();

    let app = ChatApplication::new();

    // Получение prompt от пользователя  
    // println!("\n📝 Введите ваш запрос:");
    
    // Для тестов используем hardcoded prompt
    // Но функция quick_chat ожидает &str, поэтому передаем строку напрямую
    
    match app.quick_chat("Привет, кто ты?") {
        Ok(responses) => {
            println!();
            println!("💬 Dialogue:");
            for msg in &responses {
                let role_prefix = if msg.role == "user" { "👤" } else { "🤖" };
                println!("  {} {}: {}", role_prefix, msg.role.to_uppercase(), msg.content);
            }
        },
        Err(e) => {
            eprintln!("❌ Error: {}", e);
            return Err(e.into());
        },
    }

    Ok(())
}
