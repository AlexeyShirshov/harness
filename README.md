# 🔌 Проект Ollama

Локальное соединение с локальным сервером Ollama и Rust-приложение для работы с LLM моделями.

## 📋 Требования

1. **Ollama установлен** (https://ollama.ai)
2. **Rust** установлен
3. Оllama запущен: `ollama serve`

## ✅ Доступность Ollama API

Проверьте, запущен ли Ollama на локальной машине командой:

```bash
# Windows PowerShell в OpenCode
ollama list
ollama serve
```

## 🚀 Установка модели по умолчанию

Для запуска с базовой моделью (рекомендуется):

```powershell
ollama pull llama3
ollama run llama3
```

Или другие модели: `mistral`, `gemma2`, `phi3` и др.

## 🔌 Подключение к проекту

1. Убедитесь, что Ollama запущен (`ollama serve`)
2. Запустите проект:
   ```powershell
   cargo run
   ```

## 📖 Инструкции по получению доступных моделей

В терминале OpenCode выполните:

```powershell
ollama list
```

Чтобы получить доступные модели Llama 3:

```powershell
ollama pull llama3
```

Это создаст модель `llama3` (около 5 ГБ) или используйте другие:
- `mistral`
- `gemma2`
- `phi3`

## 🧪 Взаимодействие с API

Ollama предоставляет REST API для:
- Список моделей
- Чат с LLM
- Загрузка/удаление моделей
- И другие операции.

API по умолчанию работает на порт `11434` с базовым URL `/api/tags` (список) и `/api/generate` (запрос).

## 🔧 Дополнительные настройки

Создайте `.env` файл на основе `.env.example`:

```powershell
ollama serve
```

Установка модели `llama3` в терминале локального компьютера:

```powershell
ollama pull llama3
```

## 📞 Контакты

- Оllama на GitHub: https://github.com/ollama/ollama
- Документация: https://ollama.ai/docs
