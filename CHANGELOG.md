# Changelog

Все заметные изменения проекта будут фиксироваться в этом файле.
Формат основан на [Keep a Changelog](https://keepachangelog.com/ru/1.1.0/),
проект следует [Semantic Versioning](https://semver.org/lang/ru/).

## [0.9.1] — 2026-06-17

Security-релиз. Внутренний security review закрыл ряд рисков в веб-интерфейсе.

### Безопасность
- **Web/XSS:** принудительно включён Jinja2 autoescape — пользовательский
  текст экранируется во всех шаблонах.
- **Web/CSRF:** добавлен same-origin guard middleware для POST/DELETE
  (`/message/{id}/delete`, `/restore`, `/hard-delete`). Дополнительные
  разрешённые источники — через `ALLOWED_ORIGINS`.
- **Web/Path traversal:** `/media/{id}` теперь проверяет `local_path`
  на принадлежность whitelist (`ALLOWED_MEDIA_DIRS` — по умолчанию
  `web/media_cache`, `python/storage`, `rust/storage`). Любой путь
  вне списка → 403.
- **Web/DoS:** ограничение длины параметра `q` (поиск) — 200 символов.
- **Bot/Logs:** убран лог `MYSQL_USER@HOST/DB` — оставлен только хост:порт.

### Исправлено
- `/corrections` — `Internal Server Error` из-за необъявленной
  `PAGE_SIZE` (использовалась как константа модуля). Заменено на
  `DEFAULT_PAGE_SIZE`.

### Документация
- README и `web/CLAUDE.md` дополнены разделом «Безопасность» с описанием,
  что закрыто в коде и что обязан обеспечить оператор (HTTPS, auth,
  права на `logs/`, `.env`).

## [0.9.0] — 2026-06-16

Первый функциональный релиз. Все основные сценарии работают на обеих
реализациях бота (Python и Rust) и в веб-интерфейсе.

### Добавлено
- **Бот (Python и Rust):** архивирование сообщений семейного чата в MySQL
  - Все типы контента: `text`, `photo`, `video`, `audio`, `voice`,
    `video_note`, `animation`, `document`, `sticker`, `contact`,
    `location`, `venue`, `poll`, `dice`
  - Денормализованные поля автора (`user_username/first_name/last_name`)
    и чата (`chat_title`, `chat_type`) прямо в записях `messages` —
    для быстрых выборок без JOIN
  - Служебные события: `user_joined`, `user_left`, `title_changed`,
    `photo_changed`, `photo_deleted`, `group_created`,
    `supergroup_created`, `channel_created`, `message_pinned`
  - Извлечение ссылок (`https?://…`) в отдельную таблицу `links`
- **Бот:** скачивание медиа-файлов в локальное хранилище при получении
  сообщения. Путь сохраняется в `media.local_path` (абсолютный).
  Параметры: `MEDIA_STORAGE_DIR`, `MEDIA_MAX_DOWNLOAD_SIZE` (по умолчанию 20 МБ —
  лимит Bot API).
- **Бот:** проверка орфографии русскоязычных сообщений через YandexSpeller API
  (`spellservice.json/checkText`)
  - Настройка `SPELLING_VISIBILITY`: `public` (reply в чате) / `private`
    (DM автору) / `off` (только сохранять в БД)
  - Подсказки в чат — одна строка с эмодзи: `✏️ Имя, ашибкой → ошибкой`
  - Полная история в таблице `spelling_corrections`
- **Бот:** команды `/start` и `/help` с описанием бота
- **Бот:** файловое логирование с ежедневной ротацией
  (Python: `TimedRotatingFileHandler`, Rust: `tracing-appender`)
- **Бот:** надёжность
  - Reconnection-логика к Telegram API с exponential backoff
  - Connection pooling к MySQL
  - Retry для YandexSpeller
  - `INSERT IGNORE` для защиты от дублей
  - Пропуск собственных сообщений (`is_bot` check) — нет циклов
  - Graceful shutdown по SIGINT/SIGTERM (Python: `signal`, Rust: tokio `ctrl_c`)
- **Web (FastAPI):**
  - `/` — лента сообщений с фильтрами (чат по имени, пользователь,
    тип, диапазон дат), полнотекстовый поиск, пагинация
  - Селектор размера страницы: `25, 50, 75 … 1000`, по умолчанию 100
  - Глобальная нумерация строк (продолжается между страницами)
  - Инлайн-плеер для audio/voice, миниатюры для photo/video,
    кнопка скачивания для document — прямо в ленте
  - `/message/{id}` — карточка с воспроизведением медиа, ссылками,
    орфо-подсказками; soft-delete / restore / hard-delete
  - `/users` — пользователи + количество сообщений
  - `/stats` — счётчики, бар-чарт за 30 дней, топ-10 авторов,
    типы сообщений
  - `/corrections` — лента орфографических исправлений
  - `/media/{id}` — отдаёт файл с диска (`local_path`),
    fallback на скачивание из Telegram + локальный кеш
- **БД:** soft-delete (`messages.deleted_at`) с пометкой удалённых сообщений
  и визуальным маркером в веб-интерфейсе
- **БД:** идемпотентные `ALTER TABLE` миграции во всех трёх частях —
  порядок запуска не важен, каждый компонент добавляет недостающие колонки
- **Документация:** `README.md`, корневой `CLAUDE.md`,
  отдельные `CLAUDE.md` для Python / Rust / Web,
  `schema.sql` для ручного создания БД

### Известные ограничения
- Telegram Bot API не уведомляет ботов об удалении сообщений в чате —
  колонка `deleted_at` сейчас ставится только вручную через веб-UI.
  Для автодетекта потребовался бы MTProto-companion (Telethon).
- Лимит скачивания файлов в Bot API — 20 МБ. Большие файлы не скачиваются,
  `local_path` остаётся NULL; веб попробует fallback на лету (с тем же лимитом).
- Запускать Python и Rust ботов одновременно с одним токеном **нельзя** —
  Telegram отдаёт updates только одному `getUpdates`. Для параллельного запуска
  заведите второго бота через @BotFather.
