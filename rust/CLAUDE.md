# CLAUDE.md — Rust реализация

## Структура проекта

```
rust/
├── src/
│   ├── main.rs          # Entry point, polling loop
│   ├── config.rs        # Конфигурация (env переменные)
│   ├── db/
│   │   ├── mod.rs       # Module exports
│   │   ├── pool.rs      # MySQL connection pooling
│   │   └── models.rs    # DB моделе и операции
│   ├── handlers/
│   │   ├── mod.rs
│   │   ├── messages.rs  # Обработка сообщений
│   │   └── service.rs   # Служебные события
│   ├── processors/
│   │   ├── mod.rs
│   │   ├── message.rs   # Парсинг сообщений
│   │   ├── media.rs     # Обработка медиа
│   │   └── spelling.rs  # Проверка орфографии
│   └── error.rs         # Error типы
├── Cargo.toml           # Зависимости
├── .env.example
└── CLAUDE.md           # Этот файл
```

## Команды разработки

**Сборка проекта:**
```bash
cargo build --release
```

**Запуск бота:**
```bash
cargo run
```

**Запуск тестов:**
```bash
cargo test
```

**Проверка кода:**
```bash
cargo clippy
```

## Основные компоненты

### main.rs
- Инициализация конфига
- Инициализация БД пула
- Polling loop через teloxide
- Graceful shutdown с обработкой SIGTERM

### db/pool.rs
- `DbPool` — обертка над `mysql` pool
- Retry logic с exponential backoff
- Connection health check

### handlers/messages.rs
- `handle_message()` — основной handler
- Сохранение сообщения
- Запуск проверки орфографии
- Отправка исправлений в чат

### processors/spelling.rs
- `check_spelling()` — async запрос к YandexSpeller API
- `format_corrections()` — форматирование для отправки в чат
- Retry logic и timeout

### processors/media.rs
- `extract_media_info()` — информация из attachments
- `save_media()` — сохранение в таблицу media

## Зависимости

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
teloxide = { version = "0.12", features = ["macros"] }
mysql = "25.0"
mysql_common = "0.32"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
dotenv = "0.15"
log = "0.4"
env_logger = "0.10"
reqwest = { version = "0.11", features = ["json"] }
```

## Особенности реализации на Rust

**Преимущества для надежности:**

1. **Type safety** — impossibility of null pointer bugs, memory safety
2. **Async/await** — non-blocking I/O через tokio
3. **Error handling** — требуемая явная обработка ошибок
4. **Memory safety** — отсутствие утечек памяти и buffer overflows

**Специфические реализационные детали:**

- `teloxide::dispatching` для routing обработчиков
- `tokio::sync::Mutex` для синхронизации состояния
- `Result<T, E>` everywhere для обработки ошибок
- Graceful shutdown через `tokio::signal::ctrl_c()`

## Переменные окружения (.env)

```
TELEGRAM_BOT_TOKEN=your_token_here
MYSQL_HOST=localhost
MYSQL_PORT=3306
MYSQL_USER=root
MYSQL_PASSWORD=
MYSQL_DATABASE=family_chat
LOG_LEVEL=info
RUST_LOG=info,family_chat_archiver=debug
```
