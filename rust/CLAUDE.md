# CLAUDE.md — Rust реализация

## Структура проекта

```
rust/
├── src/
│   ├── main.rs              # Entry point, dispatcher, handlers
│   ├── config.rs            # Config + SpellingVisibility enum
│   ├── error.rs             # Error типы
│   ├── db/
│   │   ├── mod.rs           # Module re-exports
│   │   ├── pool.rs          # MySQL pool + create_tables + миграции
│   │   └── models.rs        # DbMessage, User, Media, SpellingCorrection + insert_*
│   └── processors/
│       ├── mod.rs
│       ├── message.rs       # extract_urls()
│       └── spelling.rs      # YandexSpeller API + форматирование
├── Cargo.toml
├── .env.example
└── CLAUDE.md
```

## Команды разработки

```bash
cargo build --release    # Сборка
cargo run                # Запуск (для разработки)
cargo test               # Тесты
cargo clippy             # Линтер
```

## Основные компоненты

### main.rs
- `main()`: загрузка конфига, инициализация `DbPool`, создание таблиц, запуск teloxide Dispatcher с `enable_ctrlc_handler`
- `handle_message()`: dispatch по типу контента — text / photo / video / audio / voice / video_note / animation / document / sticker / contact / location / venue / poll / dice / служебные события
- `build_db_message()`: денормализованные user/chat поля
- `chat_title()`, `chat_type()`: хелперы для извлечения info из `Chat`
- `process_spelling()`: применяет `SpellingVisibility` (public reply / private DM / off)
- Пропускает сообщения от ботов (защита от циклов)
- `/start`, `/help`: HTML-описание бота

### db/pool.rs
- `DbPool`: `Arc<mysql::Pool>` для shared use
- `create_tables()`: CREATE IF NOT EXISTS + список идемпотентных `ALTER TABLE` для миграций

### db/models.rs
- Структуры `User`, `DbMessage` (имя `Db*` чтобы не конфликтовать с `teloxide::Message`), `Media`, `SpellingCorrection`
- `insert_or_update_user`, `insert_message` (INSERT IGNORE), `insert_media`, `insert_link`, `insert_spelling_correction`, `insert_service_event`

### processors/spelling.rs
- `check_spelling()`: POST к `speller.yandex.net/services/spellservice.json/checkText`, parse `Vec<Value>`, retry с backoff
- Пропускает короткие тексты, команды, чисто служебные символы
- `format_correction_message()`: применяет первое предложение к тексту
- `format_chat_message()`: компактный однострочный формат с обращением по имени

## Зависимости (см. `Cargo.toml`)

- `teloxide 0.12` — Telegram Bot API
- `tokio 1` (full) — async runtime
- `mysql 25.0` + `mysql_common 0.32` — MySQL driver
- `reqwest 0.11` (json) — HTTP для YandexSpeller
- `serde`, `serde_json` — JSON
- `regex` + `lazy_static` — извлечение URL
- `log` + `env_logger` — логирование
- `dotenv` — `.env` файлы

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
SPELLING_VISIBILITY=public      # public | private | off
```

## Замечания

- При добавлении нового типа сообщения: расширить `handle_message()`, при необходимости добавить колонки в `db/pool.rs` миграции и обновить `schema.sql` в корне.
- При изменении схемы БД — добавить `ALTER TABLE` в `create_tables()` миграции (идемпотентно — ошибки игнорируются).
