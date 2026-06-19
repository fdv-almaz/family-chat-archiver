# CLAUDE.md — Python реализация

## Структура проекта

```
python/
├── bot.py              # Основной файл: handlers, polling, отправка, запуск планировщика
├── config.py           # Конфигурация (env переменные)
├── db.py               # MySQL pool, миграции, CRUD-функции
├── spelling.py         # YandexSpeller API + форматирование подсказок
├── daily_tip.py        # «Совет дня»: фоновый планировщик + генерация через Claude API
├── media_storage.py    # Скачивание Telegram-файлов в локальное хранилище
├── storage/            # Скачанные файлы (file_unique_id + расширение)
├── logs/               # Файловые логи с ежедневной ротацией
├── requirements.txt    # Зависимости
├── .env.example        # Шаблон конфига
└── CLAUDE.md           # Этот файл
```

## Команды разработки

**Установка зависимостей:**
```bash
pip install -r requirements.txt
```

**Запуск бота:**
```bash
python bot.py
```

**Создание БД (опционально — таблицы создаются автоматически при первом запуске):**
```bash
mysql -u root -p < ../schema.sql
```

## Основные компоненты

### bot.py
- TeleBot инициализация + polling
- Обработчики: `handle_start`, `handle_text_message`, `handle_photo_message`, `handle_media_message` (audio/video/voice/video_note/animation/document/sticker), `handle_special_message` (contact/location/venue/poll/dice), `handle_other_events` (служебные)
- Хелперы: `message_context()` (денормализованные user/chat поля), `safe_send()` (retry на сетевые сбои), `send_spelling_correction()` (роутинг по `SPELLING_VISIBILITY`)
- Пропускает сообщения от ботов (защита от циклов)

### db.py
- `DBPool` — singleton с MySQL connection pool
- `create_tables()` — создание + идемпотентные `ALTER TABLE` миграции
- CRUD: `insert_or_update_user`, `insert_message`, `insert_media`, `insert_link`, `insert_spelling_correction`, `insert_service_event`
- `INSERT IGNORE` для защиты от дубликатов

### media_storage.py
- `download_and_save(bot, file_id, file_unique_id, file_size, suggested_ext)` — скачивает файл из Telegram через `bot.get_file()` + `bot.download_file()`, сохраняет в `MEDIA_STORAGE_DIR/{file_unique_id}{ext}`
- Пропускает повторное скачивание (поиск кеша по префиксу `file_unique_id`)
- Пропускает файлы больше `MEDIA_MAX_DOWNLOAD_SIZE` (по умолчанию 20 МБ — лимит Bot API)
- Вызывается из `bot.py::save_media_with_file()` сразу после `insert_media()`; путь к локальному файлу сохраняется в `media.local_path`

### daily_tip.py
- `start_scheduler(bot)` — запускает демон-поток, который спит до `TIP_HOUR:TIP_MINUTE` и шлёт совет; вызывается из `bot.py::main()`. Без `ANTHROPIC_API_KEY` тихо выходит (фича выключена)
- `run_once(bot, chat_id=None)` — генерирует совет через `anthropic` SDK (`config.ANTHROPIC_MODEL`), шлёт в чат (текст экранируется, т.к. бот в режиме HTML), сохраняет запрос+ответ в `daily_tips`. `chat_id` переопределяет чат (для команды `/check_tip`)
- `resolve_chat_id()` — `TIP_CHAT_ID` или самый активный групповой чат из БД (`db.get_most_active_chat_id`)
- `SYSTEM_PROMPT` — учитывает, что в чате есть и дети, и взрослые (безопасные темы)
- Команда `/check_tip` (`/tip`) в `bot.py::handle_check_tip` — ручной запуск в текущий чат
- `python daily_tip.py` — ручной разовый запуск из CLI (без ожидания расписания)

### spelling.py
- `check_spelling()` — POST к `speller.yandex.net/services/spellservice.json/checkText` с retry и backoff
- Пропускает короткий текст, команды (`/...`), чисто служебные символы
- `format_correction_message()` — применяет исправления
- `format_chat_message()` — компактный однострочный формат с обращением по имени

## Зависимости

- `pyTelegramBotAPI` — работа с Telegram Bot API
- `mysql-connector-python` — драйвер MySQL
- `python-dotenv` — загрузка .env
- `requests` — HTTP запросы к YandexSpeller
- `anthropic` — официальный SDK Claude API (совет дня)

## Особенности реализации на Python

- `bot.infinity_polling()` имеет встроенный retry — внешний цикл retry убран (был избыточным и конфликтовал с внутренним)
- `safe_send()` обёртка вокруг `bot.send_message/reply_to` с retry на сетевые сбои (ConnectionError, RemoteDisconnected, TimeoutError) и exponential backoff
- `is_bot` проверка в начале каждого хендлера — предотвращает циклы орфо-проверок на собственных ответах
- Connection pooling MySQL (5 соединений) с явным закрытием в `finally`
- `INSERT IGNORE` для защиты от дублирующихся сообщений (идемпотентность)
- Графический shutdown через `signal.signal(SIGINT/SIGTERM)`

## Переменные окружения (.env)

```
TELEGRAM_BOT_TOKEN=your_token_here
MYSQL_HOST=localhost
MYSQL_PORT=3306
MYSQL_USER=root
MYSQL_PASSWORD=
MYSQL_DATABASE=family_chat

LOG_LEVEL=INFO
LOG_FILE=logs/bot.log              # путь и имя файла логов
LOG_RETENTION_DAYS=7               # дней хранить ротированные файлы
LOG_TO_CONSOLE=true                # дублировать в stdout

SPELLING_VISIBILITY=public         # public | private | off

MEDIA_STORAGE_DIR=storage          # папка для скачанных медиа-файлов
MEDIA_MAX_DOWNLOAD_SIZE=20971520   # пропускать файлы больше N байт (Bot API лимит 20 МБ)

# Совет дня (включается заданием ANTHROPIC_API_KEY)
ANTHROPIC_API_KEY=                 # ключ Anthropic API
ANTHROPIC_MODEL=claude-opus-4-8    # модель Claude
TIP_CHAT_ID=                       # чат рассылки; пусто → самый активный групповой чат из БД
TIP_HOUR=6                         # время рассылки (локальное)
TIP_MINUTE=0
```
