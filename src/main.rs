mod tools;

use std::error::Error;
use std::sync::Arc;
use crate::tools::registry::register_tool;
use crate::tools::get_time_tool::GetTimeTool;
use crate::tools::get_local_timezone_tool::GetLocalTimeZoneTool;
mod chat_app;
// use crate::chat_app;

fn main() -> Result<(), Box<dyn Error>> {
    // Register the get_time tool
    register_tool(Arc::new(GetTimeTool));
    // Register the get_local_timezone tool
    register_tool(Arc::new(GetLocalTimeZoneTool));

    println!("Ollama Local Connection");
    println!("-----------------------");

    println!("\nКак использовать проект:");
    println!("   1. Запустите Ollama: ollama serve");
    println!("   2. Загрузите модель:  ollama pull llama3");
    println!("   3. Запустите пример:  cargo run --bin chat-ollama");

    println!("\nСтруктура проекта:");
    println!("   Cargo.toml          - зависимости (reqwest, serde)");
    println!("   src/main.rs         - этот файл");
    println!("   src/chat_app.rs     - модуль для работы с Ollama API");
    println!("   src/check_ollama.rs - проверка соединения");

    println!("\nЗапускаем пример чата...\n");

    chat_app::run_chat_app()?;

    println!("\nГотово.");
    Ok(())
}
