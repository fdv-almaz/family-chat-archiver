# CLAUDE.md — Rust реализация

## Структура проекта

```
rust/
├── src/
│   ├── main.rs              # Entry point, dispatcher, handlers, запуск планировщика
│   ├── config.rs            # Config + SpellingVisibility enum
│   ├── daily_tip.rs         # «Совет дня»: планировщик (tokio) + генерация через Claude API
│   ├── error.rs             # Error типы
│   ├── media_storage.rs     # Скачивание Telegram-файлов в локальное хранилище
│   ├── db/
│   │   ├── mod.rs           # Module re-exports
│   │   ├── pool.rs          # MySQL pool + create_tables + миграции
│   │   └── models.rs        # DbMessage, User, Media, SpellingCorrection + insert_*
│   └── processors/
│       ├── mod.rs
│       ├── message.rs       # extract_urls()
│       └── spelling.rs      # YandexSpeller API + форматирование
├── storage/                 # Скачанные медиа (создаётся при старте)
├── logs/                    # Ротированные логи (tracing-appender, daily)
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

### daily_tip.rs
- `spawn_scheduler(bot, db, cfg)`: `tokio::spawn` фоновой задачи, которая спит до `TIP_HOUR:TIP_MINUTE` и шлёт совет; вызывается из `main()` (передаётся `bot.clone()`). Без `ANTHROPIC_API_KEY` тихо выходит (фича выключена)
- `run_once(bot, db, cfg, chat_override)`: генерирует совет через Claude Messages API (`reqwest`), шлёт в чат (обычный текст), сохраняет запрос+ответ в `daily_tips`. `chat_override` переопределяет чат (команда `/check_tip`)
- Команда `/check_tip` (`/tip`) обрабатывается в `main.rs::handle_message` — ручной запуск в текущий чат
- `generate_tip()`: POST к `api.anthropic.com/v1/messages` с заголовками `x-api-key` / `anthropic-version`
- `seconds_until()`: расчёт времени до ближайшего срабатывания (локальное время, `chrono`)
- Чат: `tip_chat_id` или `db.get_most_active_chat_id()` (самый активный групповой чат)
- Системный промпт вынесен в конфигурационный файл `daily_tip_prompt.txt` (корень проекта; путь переопределяется `TIP_SYSTEM_PROMPT_FILE`) и читается в `Config::tip_system_prompt` — правится без пересборки; учитывает, что в чате есть и дети, и взрослые. Если файл недоступен — встроенный fallback + предупреждение в stderr

### db/pool.rs
- `DbPool`: `Arc<mysql::Pool>` для shared use
- `create_tables()`: CREATE IF NOT EXISTS + список идемпотентных `ALTER TABLE` для миграций

### db/models.rs
- Структуры `User`, `DbMessage` (имя `Db*` чтобы не конфликтовать с `teloxide::Message`), `Media`, `SpellingCorrection`
- `insert_or_update_user`, `insert_message` (INSERT IGNORE), `insert_media`, `insert_link`, `insert_spelling_correction`, `insert_service_event`

### media_storage.rs
- `download_and_save(bot, dir, file_id, file_unique_id, file_size, max)`: async скачивание через `teloxide::net::Download` (`bot.get_file()` + `bot.download_file()`)
- Сохранение в `MEDIA_STORAGE_DIR/{file_unique_id}{ext}`
- Пропуск повторного скачивания (поиск по префиксу) и файлов больше лимита (~20 МБ Bot API)
- Вызывается из `save_media_and_download()` в `main.rs` после `db.insert_media()`; путь обновляется в `media.local_path` через `update_media_local_path()`

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
- `chrono` — расчёт времени срабатывания планировщика совета дня

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

MEDIA_STORAGE_DIR=storage       # папка для скачанных медиа
MEDIA_MAX_DOWNLOAD_SIZE=20971520  # пропускать файлы больше N байт

# Совет дня (включается заданием ANTHROPIC_API_KEY)
ANTHROPIC_API_KEY=              # ключ Anthropic API
ANTHROPIC_MODEL=claude-opus-4-8  # модель Claude
TIP_CHAT_ID=                   # чат рассылки; пусто → самый активный групповой чат из БД
TIP_HOUR=6                     # время рассылки (локальное)
TIP_MINUTE=0
TIP_SYSTEM_PROMPT_FILE=../daily_tip_prompt.txt  # файл системного промпта (правится без пересборки)
```

## Замечания

- При добавлении нового типа сообщения: расширить `handle_message()`, при необходимости добавить колонки в `db/pool.rs` миграции и обновить `schema.sql` в корне.
- При изменении схемы БД — добавить `ALTER TABLE` в `create_tables()` миграции (идемпотентно — ошибки игнорируются).
