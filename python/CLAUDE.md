# CLAUDE.md — Python реализация

## Структура проекта

```
python/
├── bot.py              # Основной файл: handlers, polling, отправка
├── config.py           # Конфигурация (env переменные, SPELLING_VISIBILITY)
├── db.py               # MySQL pool, миграции, CRUD-функции
├── spelling.py         # YandexSpeller API + форматирование подсказок
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
```
