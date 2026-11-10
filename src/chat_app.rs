use std::io::{stdin, Write};
use std::time::Duration;

use crate::tools::registry;

use reqwest::blocking::{Client, RequestBuilder, Response};
use serde_json::Value;
use serde_json::json;

/// How many times a chat request is attempted before giving up.
const CHAT_ATTEMPTS: u32 = 7;

pub fn run_chat_app() -> Result<(), Box<dyn std::error::Error>> {
    let http_client = Client::new();

    println!("Ollama Local Connection\n");
    println!("Fetching models from Ollama API (/api/tags)...");

    let models = match http_client.get("http://localhost:11434/api/tags").send() {
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
    run_chat_loop(&http_client, &models, idx)?;

    Ok(())
}

fn default_models() -> Vec<String> {
    vec![
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

const SYSTEM_PROMPT: &str = "Ты локальный ассистент с доступом к инструментам. \
Помни предыдущие сообщения диалога и учитывай их в ответах.";

/// Tool output is trimmed before it goes into the history.
const MAX_TOOL_OUTPUT_CHARS: usize = 50_000;

fn clamp_tool_output(output: String) -> String {
    match output.char_indices().nth(MAX_TOOL_OUTPUT_CHARS) {
        None => output,
        Some((cut, _)) => {
            let omitted = output[cut..].chars().count();
            let mut clamped = output[..cut].to_string();
            clamped.push_str(&format!(
                "\n…[вывод обрезан: ещё {} символов из {}]",
                omitted,
                omitted + MAX_TOOL_OUTPUT_CHARS
            ));
            clamped
        }
    }
}

/// Context size in binary kilobytes: 131072 -> "128K".
fn k_tokens(tokens: usize) -> String {
    format!("{}K", (tokens as f64 / 1024.0).round() as usize)
}

/// One assistant reply plus how many tokens the conversation occupied afterwards.
struct Reply {
    text: String,
    used_tokens: usize,
}

/// Read `num_ctx` from the model's parameters (`/api/show`).
/// Returns `None` when the model does not set it explicitly.
fn fetch_context_limit(client: &Client, model: &str) -> Option<usize> {
    let resp = client
        .post("http://localhost:11434/api/show")
        .json(&json!({ "model": model }))
        .send()
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let v: Value = serde_json::from_str(&resp.text().ok()?).ok()?;
    v.get("parameters")?
        .as_str()?
        .lines()
        .find(|line| line.trim_start().starts_with("num_ctx"))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|n| n.parse().ok())
}

fn print_prompt(used_tokens: usize, ctx_limit: Option<usize>) {
    match ctx_limit {
        Some(limit) if limit > 0 => print!(
            "[{}/{} tok, {:.1}%] >>> ",
            used_tokens,
            k_tokens(limit),
            used_tokens as f64 * 100.0 / limit as f64
        ),
        _ => print!("[{} tok] >>> ", used_tokens),
    }
    std::io::stdout().flush().unwrap();
}

const COMMANDS: [&str; 9] = [
    "help", "exit", "quit", "clear", "tokens", "compact", "model", "tools", "system",
];

enum Resolved {
    Command(&'static str),
    Ambiguous(Vec<&'static str>),
    Unknown,
}

/// Accept any unique prefix of a command: `q` -> quit, `co` -> compact.
fn resolve_command(input: &str) -> Resolved {
    let input = input.to_lowercase();

    if let Some(name) = COMMANDS.iter().find(|name| **name == input).copied() {
        return Resolved::Command(name);
    }

    let candidates: Vec<&'static str> = COMMANDS
        .iter()
        .copied()
        .filter(|name| name.starts_with(&input))
        .collect();

    match candidates.len() {
        1 => Resolved::Command(candidates[0]),
        0 => Resolved::Unknown,
        _ => Resolved::Ambiguous(candidates),
    }
}

fn print_help() {
    println!(
        "Команды:
  /help          — эта справка
  /exit, /quit   — выход
  /clear         — очистить историю диалога
  /tokens        — размер контекста в токенах
  /compact       — сжать историю в резюме
  /model         — сменить модель (история сохраняется)
  /tools         — список доступных инструментов
  /system [txt]  — показать или задать системный промпт

Любую команду можно сократить до уникального префикса: /q, /co, /cl, /t (неоднозначно: /tok, /too)."
    );
}

fn print_tokens(messages: &[Value], model: &str, used_tokens: usize, ctx_limit: Option<usize>) {
    match ctx_limit {
        Some(limit) if limit > 0 => println!(
            "Контекст: {} / {} ({} токенов), {:.1}%",
            used_tokens,
            k_tokens(limit),
            limit,
            used_tokens as f64 * 100.0 / limit as f64
        ),
        _ => println!("Контекст: {} токенов (лимит модели не задан)", used_tokens),
    }
    println!(
        "Сообщений в истории: {} (включая системный промпт)",
        messages.len()
    );
    println!("Модель: {}", model);
}

fn print_tools() {
    let names = registry::list_tool_names();
    if names.is_empty() {
        println!("Инструменты не зарегистрированы.");
        return;
    }

    println!("=== Инструменты ===");
    for name in names {
        if let Some(tool) = registry::get_tool(&name) {
            println!("  {:<22} {}", tool.name(), tool.description());
        }
    }
}

const COMPACT_INSTRUCTION: &str = "Ниже история текущего диалога между пользователем и ассистентом.
Составь список фактов, которые нужно помнить, чтобы продолжить этот же диалог.
Правила:
- каждая строка начинается с «- » и формулируется как утверждение;
- обязательно укажи, как зовут пользователя и как зовут ассистента, если это звучало в диалоге; ассистент — это ты;
- имена, числа, даты, суммы, пути, названия проектов и принятые решения сохраняй дословно;
- перечисли открытые вопросы и незавершённые задачи;
- не пересказывай приветствия, любезности и эмоции;
- ничего не выдумывай и не добавляй от себя;
- пиши на языке диалога, выведи только список, без вступления.";

const SUMMARY_PREFIX: &str = "Факты из предыдущей части этого же диалога (считай их верными и не переспрашивай):";

enum Compact {
    Done {
        before: usize,
        after: usize,
        summary: String,
    },
    Nothing,
    Failed(String),
}

fn post_chat(client: &Client, body: &Value) -> Result<String, ChatError> {
    let resp = send_with_retry(client.post("http://localhost:11434/api/chat").json(body))
        .map_err(|e| ChatError::Transport(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(ChatError::Http(resp.status()));
    }
    Ok(resp.text().unwrap_or_default())
}

enum ChatError {
    Http(reqwest::StatusCode),
    Transport(String),
}

impl std::fmt::Display for ChatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChatError::Http(status) => write!(f, "API Error: {}", status),
            ChatError::Transport(e) => write!(f, "{}", e),
        }
    }
}

/// Retry a request on timeout with exponential backoff: 1, 2, 4, 8, ... seconds.
fn send_with_retry(builder: RequestBuilder) -> Result<Response, reqwest::Error> {
    let mut delay = Duration::from_secs(1);
    let mut attempt = 1;

    loop {
        let Some(request) = builder.try_clone() else {
            return builder.send();
        };

        match request.send() {
            Ok(resp) => return Ok(resp),
            Err(e) if e.is_timeout() && attempt < CHAT_ATTEMPTS => {
                eprintln!(
                    "⏳ таймаут, повтор через {} с (попытка {}/{})",
                    delay.as_secs(),
                    attempt + 1,
                    CHAT_ATTEMPTS
                );
                std::thread::sleep(delay);
                delay *= 2;
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

fn history_chars(messages: &[Value]) -> usize {
    messages
        .iter()
        .map(|m| m.to_string().chars().count())
        .sum()
}

/// Replace everything before the last user turn with a model-written summary.
/// The history is left untouched if the summarization request fails.
fn compact_history(
    client: &Client,
    model: &str,
    messages: &mut Vec<Value>,
    used_tokens: usize,
) -> Compact {
    let Some(keep_from) = messages
        .iter()
        .rposition(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
    else {
        return Compact::Nothing;
    };
    if keep_from <= 1 {
        return Compact::Nothing;
    }

    let to_summarize = &messages[1..keep_from];
    // Nothing but a previous summary: compacting it again would only lose detail
    if to_summarize.len() == 1
        && to_summarize[0]
            .get("content")
            .and_then(|c| c.as_str())
            .is_some_and(|c| c.starts_with(SUMMARY_PREFIX))
    {
        return Compact::Nothing;
    }
    let chars_before = history_chars(to_summarize) + COMPACT_INSTRUCTION.chars().count();

    let mut request_messages = to_summarize.to_vec();
    request_messages.push(json!({"role": "user", "content": COMPACT_INSTRUCTION}));

    let mut body = json!({
        "model": model,
        "messages": request_messages,
        "stream": false,
        "think": false,
        "options": { "temperature": 0.2, "num_predict": 700 },
    });

    let resp = match post_chat(client, &body) {
        Ok(text) => text,
        // Some models reject the `think` flag: retry without it
        Err(ChatError::Http(_)) => {
            if let Value::Object(map) = &mut body {
                map.remove("think");
            }
            match post_chat(client, &body) {
                Ok(text) => text,
                Err(e) => return Compact::Failed(e.to_string()),
            }
        }
        Err(e) => return Compact::Failed(e.to_string()),
    };

    let v: Value = serde_json::from_str(&resp).unwrap_or_default();
    let summary = v
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if summary.is_empty() {
        return Compact::Failed("модель вернула пустое резюме".to_string());
    }

    let prompt_tokens = v
        .get("prompt_eval_count")
        .and_then(|n| n.as_u64())
        .unwrap_or(0) as usize;
    let before = if used_tokens > 0 {
        used_tokens
    } else {
        prompt_tokens
    };
    let tokens_per_char = if chars_before > 0 && prompt_tokens > 0 {
        prompt_tokens as f64 / chars_before as f64
    } else {
        0.25
    };

    let mut compacted = vec![
        messages[0].clone(),
        json!({
            "role": "system",
            "content": format!("{}\n{}", SUMMARY_PREFIX, summary),
        }),
    ];
    compacted.extend_from_slice(&messages[keep_from..]);

    let after = (history_chars(&compacted) as f64 * tokens_per_char).ceil() as usize;
    *messages = compacted;

    Compact::Done {
        before,
        after,
        summary,
    }
}

fn run_chat_loop(
    client: &Client,
    models: &[String],
    start_idx: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    // Build tool list from registry once and reuse it for every request
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

    // Full conversation history, kept for the whole session
    let mut messages: Vec<Value> = vec![json!({"role": "system", "content": SYSTEM_PROMPT})];

    let mut model_name = models[start_idx].clone();
    let mut ctx_limit = fetch_context_limit(client, &model_name);
    let mut used_tokens = 0usize;

    println!("Введите /help для списка команд.\n");

    loop {
        print_prompt(used_tokens, ctx_limit);

        let mut user_inp = String::new();
        stdin().read_line(&mut user_inp)?;
        let input = user_inp.trim();
        if input.is_empty() {
            continue;
        }

        // Slash commands
        if let Some(rest) = input.strip_prefix('/') {
            let (cmd, arg) = rest
                .split_once(char::is_whitespace)
                .unwrap_or((rest, ""));
            let cmd = cmd.trim();
            if cmd.is_empty() {
                print_help();
                continue;
            }

            match resolve_command(cmd) {
                Resolved::Command("help") => print_help(),
                Resolved::Command("exit" | "quit") => {
                    println!("\nGood bye!");
                    break;
                }
                Resolved::Command("clear") => {
                    messages.truncate(1);
                    used_tokens = 0;
                    println!("История очищена.");
                }
                Resolved::Command("tokens") => {
                    print_tokens(&messages, &model_name, used_tokens, ctx_limit)
                }
                Resolved::Command("compact") => {
                    if let Some(limit) = ctx_limit {
                        if limit > 0 && used_tokens * 100 >= limit * 90 {
                            println!(
                                "⚠ История почти в лимите ({} из {}), сводка может быть неполной.",
                                used_tokens,
                                k_tokens(limit)
                            );
                        }
                    }
                    match compact_history(client, &model_name, &mut messages, used_tokens) {
                        Compact::Done {
                            before,
                            after,
                            summary,
                        } => {
                            used_tokens = after;
                            println!(
                                "История сжата: было {} токенов, стало ~{} (оценка).",
                                before, after
                            );
                            println!("Резюме:\n{}", summary);
                            println!("Сообщений в истории: {}.", messages.len());
                        }
                        Compact::Nothing => {
                            println!("Сжимать нечего: вся история — это текущий ход.")
                        }
                        Compact::Failed(e) => println!("Не удалось сжать историю: {}", e),
                    }
                }
                Resolved::Command("tools") => print_tools(),
                Resolved::Command("model") => match show_models(models) {
                    Ok(idx) => {
                        model_name = models[idx].clone();
                        ctx_limit = fetch_context_limit(client, &model_name);
                        used_tokens = 0;
                        println!("Счётчик контекста обновится после следующего запроса.");
                    }
                    Err(e) => println!("Не удалось выбрать модель: {}", e),
                },
                Resolved::Command("system") => {
                    let arg = arg.trim();
                    if arg.is_empty() {
                        let current = messages[0]
                            .get("content")
                            .and_then(|c| c.as_str())
                            .unwrap_or("");
                        println!("Текущий системный промпт:\n{}", current);
                    } else {
                        messages[0] = json!({"role": "system", "content": arg});
                        println!("Системный промпт обновлён.");
                    }
                }
                Resolved::Ambiguous(options) => {
                    let list = options
                        .iter()
                        .map(|name| format!("/{}", name))
                        .collect::<Vec<_>>()
                        .join(", ");
                    println!("Неоднозначная команда /{}: {}", cmd, list);
                }
                Resolved::Unknown => println!("Неизвестная команда: /{}. Введите /help.", cmd),
                Resolved::Command(_) => unreachable!("all commands are handled above"),
            }
            continue;
        }

        messages.push(json!({"role": "user", "content": input}));

        let reply = chat_with_tools(client, &model_name, &mut messages, &tools_json);
        println!("{}", reply.text);
        println!("==========");
        if reply.used_tokens > 0 {
            used_tokens = reply.used_tokens;
        }
    }
    Ok(())
}

fn chat_with_tools(
    client: &Client,
    model: &str,
    messages: &mut Vec<Value>,
    tools_json: &[Value],
) -> Reply {
    let mut used_tokens = 0usize;

    for _ in 0..5 {
        let body = json!({
            "model": model,
            "messages": &messages[..],
            "tools": tools_json,
            "stream": false,
        });

        let resp = match post_chat(client, &body) {
            Ok(text) => text,
            Err(ChatError::Http(status)) => {
                return Reply {
                    text: format!("API Error: {}", status),
                    used_tokens,
                }
            }
            Err(ChatError::Transport(e)) => {
                return Reply {
                    text: format!("🤖 {} MODE (offline): {}", model, e),
                    used_tokens,
                }
            }
        };

        let v: serde_json::Value = serde_json::from_str(&resp).unwrap_or_default();
        let prompt_tokens = v
            .get("prompt_eval_count")
            .and_then(|n| n.as_u64())
            .unwrap_or(0) as usize;
        let eval_tokens = v.get("eval_count").and_then(|n| n.as_u64()).unwrap_or(0) as usize;
        if prompt_tokens > 0 {
            used_tokens = prompt_tokens + eval_tokens;
        }

        let mut msg = v.get("message").cloned().unwrap_or_default();
        if msg.get("role").is_none() {
            msg["role"] = json!("assistant");
        }

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

                messages.push(json!({
                    "role": "tool",
                    "tool_name": name,
                    "content": clamp_tool_output(result),
                }));
            }
            continue;
        }

        // Otherwise return the content
        if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
            let answer = content.to_string();
            messages.push(json!({"role": "assistant", "content": answer.clone()}));
            return Reply {
                text: answer,
                used_tokens,
            };
        }
        return Reply {
            text: resp,
            used_tokens,
        };
    }

    Reply {
        text: "Tool loop exceeded".to_string(),
        used_tokens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_short_tool_output() {
        let short = "ok".to_string();
        assert_eq!(clamp_tool_output(short.clone()), short);
    }

    #[test]
    fn clamps_long_tool_output() {
        let long = "я".repeat(MAX_TOOL_OUTPUT_CHARS + 1_000);
        let clamped = clamp_tool_output(long);
        assert!(clamped.starts_with(&"я".repeat(MAX_TOOL_OUTPUT_CHARS)));
        assert!(!clamped.starts_with(&"я".repeat(MAX_TOOL_OUTPUT_CHARS + 1)));
        assert!(clamped.contains("вывод обрезан"));
    }

    #[test]
    fn resolves_unique_prefixes() {
        assert!(matches!(resolve_command("q"), Resolved::Command("quit")));
        assert!(matches!(resolve_command("co"), Resolved::Command("compact")));
        assert!(matches!(resolve_command("cl"), Resolved::Command("clear")));
        assert!(matches!(resolve_command("h"), Resolved::Command("help")));
        assert!(matches!(resolve_command("M"), Resolved::Command("model")));
        assert!(matches!(resolve_command("tok"), Resolved::Command("tokens")));
        assert!(matches!(resolve_command("t"), Resolved::Ambiguous(_)));
        assert!(matches!(resolve_command("zzz"), Resolved::Unknown));
    }

    #[test]
    fn shows_context_in_k() {
        assert_eq!(k_tokens(131_072), "128K");
        assert_eq!(k_tokens(24_576), "24K");
        assert_eq!(k_tokens(4096), "4K");
    }
}

