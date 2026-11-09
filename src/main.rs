mod chat_app;

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
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
