use std::io::{stdin, Write};
use chrono::Local;

use crate::tools::registry;

use reqwest::blocking::Client;
use serde_json::Value;
use serde_json::json;

pub fn run_chat_app() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    println!("Ollama Local Connection\n");
    println!("Fetching models from Ollama API (/api/tags)...");

    let models = match client.get("http://localhost:11434/api/tags").send() {
        Ok(res) if res.status().is_success() => {
            let text = res.text().unwrap_or_default();
            if let Ok(Value::Object(map)) = serde_json::from_str(&text) {
                if let Some(Value::Array(models_arr)) = map.get("models") {
                    let vec: Vec<String> = models_arr
                        .iter()
                        .filter_map(|m| m.as_object())
                        .filter_map(|obj| obj.get("name").and_then(|n| n.as_str()))
                        .map(str::to_string)
                        .collect();
                    if !vec.is_empty() {
                        vec
                    } else {
                        default_models()
                    }
                } else {
                    default_models()
                }
            } else {
                default_models()
            }
        }
        _ => default_models(),
    };

    let idx = show_models(&models)?;
    let model_name = models[idx].clone();
    run_chat_loop(&client, &model_name)?;

    Ok(())
}

fn default_models() -> Vec<String> {
    vec![
        "gpt-oss:20b-base".into(),
        "qwen2.5-coder:14b-24k".into(),
        "gpt-oss:20b-48k".into(),
        "qwen3.5:9b-128k".into(),
    ]
}

fn show_models(models: &[String]) -> Result<usize, Box<dyn std::error::Error>> {
    println!("=== Available models ===\n");
    for (i, m) in models.iter().enumerate() {
        println!("{:2}). {}", i + 1, m);
    }

    print!("Choose model number or Enter for first: ");
    std::io::stdout().flush().unwrap();

    let mut choice = String::new();
    stdin().read_line(&mut choice)?;

    let idx = if choice.trim().is_empty() {
        0
    } else if let Ok(n) = choice.trim().parse::<usize>() {
        if n >= 1 && n <= models.len() {
            n - 1
        } else {
            0
        }
    } else {
        0
    };

    println!("\nSelected model: {}", models[idx]);
    println!();

    Ok(idx)
}

fn run_chat_loop(client: &Client, model_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        print!(">>> ");
        std::io::stdout().flush().unwrap();

        let mut user_inp = String::new();
        stdin().read_line(&mut user_inp)?;
        let prompt = user_inp.trim();
        if prompt.to_lowercase().contains("сколько") && prompt.to_lowercase().contains("время") {
            let now = Local::now().format("%H:%M:%S").to_string();
            println!("Текущее время: {}", now);
            continue;
        }
        if prompt.eq_ignore_ascii_case("exit") || prompt.eq_ignore_ascii_case("выход") {
            println!("\nGood bye!");
            break;
        }

        let resp = chat_with_tools(client, model_name, prompt);
        println!("{}", resp);
        println!("==========");
    }
    Ok(())
}

fn chat_with_tools(client: &Client, model: &str, prompt: &str) -> String {
    // Build tool list from registry
    let tool_names = registry::list_tool_names();
    let tools_json: Vec<serde_json::Value> = tool_names
        .iter()
        .filter_map(|name| {
            registry::get_tool(name).map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name(),
                        "description": t.description(),
                        "parameters": { "type": "object", "properties": {}, "required": [] }
                    }
                })
            })
        })
        .collect();

    let mut messages = vec![json!({"role": "user", "content": prompt})];

    for _ in 0..5 {
        let body = json!({
            "model": model,
            "messages": messages,
            "tools": tools_json,
            "stream": false,
        });

        let resp = match client
            .post("http://localhost:11434/api/chat")
            .json(&body)
            .send()
        {
            Ok(r) if r.status().is_success() => r.text().unwrap_or_default(),
            Ok(r) => return format!("API Error: {}", r.status()),
            Err(_) => return format!("🤖 {} MODE (offline)", model),
        };

        let v: serde_json::Value = serde_json::from_str(&resp).unwrap_or_default();
        let msg = v.get("message").cloned().unwrap_or_default();

        // If model requested tool calls, execute them
        if let Some(calls) = msg.get("tool_calls").and_then(|c| c.as_array()) {
            messages.push(msg.clone());
            for call in calls {
                let name = call
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("");
                let args = call
                    .get("function")
                    .and_then(|f| f.get("arguments"))
                    .cloned()
                    .unwrap_or(json!({}));

                let result = if let Some(tool) = registry::get_tool(name) {
                    let input = args.to_string();
                    match tool.run(&input, "") {
                        Ok(r) => r,
                        Err(e) => format!("Error: {}", e),
                    }
                } else {
                    format!("Unknown tool: {}", name)
                };

                messages.push(json!({"role": "tool", "content": result}));
            }
            continue;
        }

        // Otherwise return the content
        if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
            return content.to_string();
        }
        return resp;
    }

    "Tool loop exceeded".to_string()
}
