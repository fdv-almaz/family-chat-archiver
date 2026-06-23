# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Проект: Family Chat Archiver

Телеграм-бот для архивирования сообщений семейной группы в MySQL с проверкой правописания.

**Основные функции:**
1. Сохранение всей переписки (текст, медиа, ссылки)
2. Сохранение информации об авторах и служебных событиях
3. Проверка правописания русскоязычных текстов через YandexSpeller API
4. Отправка исправлений в чат и сохранение в отдельную таблицу БД
5. «Совет дня»: фоновый планировщик внутри бота ежедневно в `TIP_HOUR:TIP_MINUTE`
   (по умолчанию 6:00) запрашивает короткий совет у последней модели Claude
   (с учётом того, что в чате есть и дети, и взрослые) и шлёт его в чат; запрос и
   ответ сохраняются в таблицу `daily_tips`. Фича включается заданием `ANTHROPIC_API_KEY`.
   Чтобы советы не повторялись, последние `TIP_HISTORY_LIMIT` уже отправленных
   советов передаются модели с инструкцией не дублировать их.
   Команда `/check_tip` (или `/tip`) — ручной запуск: сразу генерирует совет и шлёт
   его в текущий чат (для проверки, без ожидания расписания).

## Структура

```
family-chat-archiver/
├── python/          # Python реализация бота (pyTelegramBotAPI)
├── rust/            # Rust реализация бота (teloxide)
├── web/             # Веб-интерфейс просмотра/поиска/управления (FastAPI)
├── schema.sql       # Схема БД (опционально для ручного создания)
└── CLAUDE.md        # Этот файл
```

Каждая папка содержит свой CLAUDE.md с деталями. Все три модуля работают с одной MySQL.

## Общая архитектура

```
Telegram API
    │
    ▼ updates                            ▲ get_file / download_file
[Bot (Python или Rust)]──────────────────┤
    │                                    │
    ▼ метаданные + spelling              │ (для файлов > 20 МБ — пропуск)
[MySQL]   ← read-only ←  [Web (FastAPI)]─┘
              │                ▲
              │                │
   `media.local_path`          │
              │                │
              ▼                │
       [./storage]──────────── │
       (файлы скачаны ботом)   │
                               │
            web fallback при отсутствии local_path
            (для legacy данных) — кеш в `web/media_cache/`
```

**Кто что хранит:**
- **MySQL** — метаданные (текст, file_id, file_size, имена, время, пометка `deleted_at`)
- **`./storage/{file_unique_id}.ext`** — сами байты файлов, скачивает бот при получении
- **`./media_cache/`** в web — fallback для legacy записей без `local_path`

## Схема базы данных

Актуальная схема — в файле [`schema.sql`](./schema.sql) в корне проекта. Таблицы создаются автоматически при первом запуске.

**Таблицы:**
- `users` — пользователи
- `messages` — сообщения (с денормализованными `user_username/first_name/last_name`, `chat_title/type`, плюс `deleted_at` для soft-delete из веб-UI)
- `media` — медиа-файлы (`type`: photo, video, audio, voice, video_note, animation, document, sticker; поля `file_name`, `duration`)
- `links` — ссылки из сообщений
- `spelling_corrections` — история орфографических ошибок
- `service_events` — служебные события (user_joined, title_changed и т.д.)
- `daily_tips` — история «совета дня» (запрос, ответ, модель, статус отправки)

**При изменении схемы:** обновите одновременно `schema.sql`, `python/db.py::create_tables()`, `rust/src/db/pool.rs::create_tables()` и при необходимости `web/db.py::_ensure_*_column()` (включая идемпотентные `ALTER TABLE` миграции).

**Soft-delete:** колонка `messages.deleted_at` ставится только из веб-UI вручную. Telegram Bot API не уведомляет об удалении сообщений в чате — для автодетекта нужен MTProto-companion (Telethon).

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

# Совет дня (включается заданием ANTHROPIC_API_KEY)
ANTHROPIC_API_KEY=...
ANTHROPIC_MODEL=claude-opus-4-8   # модель Claude для совета
TIP_CHAT_ID=                      # чат рассылки; пусто → самый активный групповой чат из БД
TIP_HOUR=6                        # время рассылки (локальное)
TIP_MINUTE=0
TIP_SYSTEM_PROMPT_FILE=../daily_tip_prompt.txt  # файл системного промпта (общий для Python и Rust)
TIP_HISTORY_LIMIT=30              # сколько прошлых советов передавать модели, чтобы не повторялась
```

См. `.env.example` в каждой реализации.
