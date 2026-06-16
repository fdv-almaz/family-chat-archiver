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

**Проблемы старой версии и как их избежать:**

1. **Зависание на получении обновлений:**
   - Использовать timeout для polling
   - Graceful обработка таймаутов
   - Логирование потерянных соединений

2. **Блокирующие операции с БД:**
   - Connection pooling с переиспользованием соединений
   - Батчинг запросов где возможно
   - Timeout для DB операций

3. **Утечки памяти:**
   - Правильное закрытие соединений
   - Явный graceful shutdown
   - Обработка исключений в exception handlers

## Переменные окружения (.env)

```
TELEGRAM_BOT_TOKEN=your_token_here
MYSQL_HOST=localhost
MYSQL_PORT=3306
MYSQL_USER=root
MYSQL_PASSWORD=
MYSQL_DATABASE=family_chat
LOG_LEVEL=INFO
SPELLING_VISIBILITY=public   # public | private | off
```
