# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Проект: Family Chat Archiver

Телеграм-бот для архивирования сообщений семейной группы в MySQL с проверкой правописания.

**Основные функции:**
1. Сохранение всей переписки (текст, медиа, ссылки)
2. Сохранение информации об авторах и служебных событиях
3. Проверка правописания русскоязычных текстов через YandexSpeller API
4. Отправка исправлений в чат и сохранение в отдельную таблицу БД

## Структура

```
family-chat-archiver/
├── python/          # Python реализация (телеграм-бот на pyTelegramBotAPI)
├── rust/            # Rust реализация (телеграм-бот на teloxide)
└── CLAUDE.md        # Этот файл
```

Каждая реализация полностью независима и содержит свой CLAUDE.md.

## Общая архитектура

```
Telegram API
    ↓
[Bot Handler (async)]
    ↓
[Message Processor]
    ├─→ Extract text, media, users
    ├─→ YandexSpeller check (async)
    └─→ Format response
    ↓
[Database Layer]
    ├─→ Save message
    ├─→ Save media
    ├─→ Save spelling corrections
    └─→ Save service events
    ↓
MySQL Database
```

## Схема базы данных

Актуальная схема — в файле [`schema.sql`](./schema.sql) в корне проекта. Таблицы создаются автоматически при первом запуске.

**Таблицы:**
- `users` — пользователи
- `messages` — сообщения (с денормализованными `user_username/first_name/last_name`, `chat_title/type`)
- `media` — медиа-файлы (`type`: photo, video, audio, voice, video_note, animation, document, sticker; поля `file_name`, `duration`)
- `links` — ссылки из сообщений
- `spelling_corrections` — история орфографических ошибок
- `service_events` — служебные события (user_joined, title_changed и т.д.)

**При изменении схемы:** обновите одновременно `schema.sql`, `python/db.py::create_tables()` и `rust/src/db/pool.rs::create_tables()` (включая идемпотентные `ALTER TABLE` миграции).

## Требования к надежности

- Graceful shutdown при SIGTERM
- Reconnection logic для Telegram API (exponential backoff)
- Connection pooling к MySQL
- Retry logic для запросов к YandexSpeller
- Обработка дублирующихся сообщений (idempotency)
- Логирование всех операций

## Переменные окружения

```
TELEGRAM_BOT_TOKEN=...
MYSQL_HOST=localhost
MYSQL_PORT=3306
MYSQL_USER=...
MYSQL_PASSWORD=...
MYSQL_DATABASE=...
LOG_LEVEL=info
SPELLING_VISIBILITY=public   # public | private | off
```

См. `.env.example` в каждой реализации.
