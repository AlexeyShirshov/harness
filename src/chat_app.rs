use serde::{Deserialize, Serialize};
use reqwest::blocking::Client;
use std::io::BufRead;

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
}

struct ChatApplication {
    client: Client,
}

impl ChatApplication {
    fn new() -> Self {
        Self {
            client: Client::new(),
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
        
        let messages = vec![Message {
            role: "user".to_string(),
            content: prompt.to_string(),
        }];

        let request = ChatRequest {
            model: model.to_string(),
            messages,
            stream: false,
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
            match result.get("message").and_then(|m| m.as_object()) {
                Some(msg_obj) => {
                    let content = msg_obj.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string();
                    let role = msg_obj.get("role").and_then(|r| r.as_str()).unwrap_or("assistant");

                    Ok(vec![
                        Message{role: "user".to_string(), content: prompt.to_string()},
                        Message{role: role.to_string(), content},
                    ])
                }
                None => {
                    eprintln!("⚠ Chat response does not contain message field");
                    Err("Chat response missing message".into())
                }
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
                    // Use first model from the list directly
                    let name_obj = models_arr[0].get("name")
                        .and_then(|n| n.as_str());
                    let first_model_name = name_obj.unwrap_or("qwen3.5:9b-128k");
                    
                    Ok(self.chat_call(first_model_name, user_prompt)?)
                } else {
                    // Fallback to default model if no models available
                    println!("⚠ Нет доступных моделей, используем по умолчанию...");
                    Ok(self.chat_call("qwen3.5:9b-128k", user_prompt)?)
                }
            } else {
                Ok(self.chat_call("qwen3.5:9b-128k", user_prompt)?)
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
    println!("\n📝 Введите ваш запрос:");
    
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

    // List models
    println!();
    println!("📋 Available models from Ollama:");
    if let Ok(models) = app.list_models() {
        for model in &models {
            println!("  - {}", model.name);
        }
    } else {
        eprintln!("  Failed to get model list");
    }

    Ok(())
}
