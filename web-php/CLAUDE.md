# CLAUDE.md — Web интерфейс (PHP)

PHP-копия веб-интерфейса (`../web`, FastAPI). Полностью повторяет функциональность
Python-версии: те же маршруты, фильтры, шаблоны, стили и модель безопасности.
Работает с **той же MySQL**, что и боты и Python-веб — это альтернативная
реализация просмотрщика, не отдельная база.

Без фреймворков и Composer: чистый PHP 8 + PDO. Никаких зависимостей ставить не нужно.

## Структура

```
web-php/
├── public/
│   ├── index.php       # фронт-контроллер + роутер (и router для php -S)
│   └── static/         # style.css, app.js (копии из ../web/static)
├── src/
│   ├── Config.php      # чтение .env + константы (копия web/config.py)
│   ├── Db.php          # PDO + все SQL-запросы (копия web/db.py)
│   ├── Telegram.php    # скачивание медиа с кешем через cURL (копия web/telegram.py)
│   └── helpers.php     # e() (экранирование), fdate(), truncate(), render_page()
├── templates/          # base, index, message, users, stats, corrections, tips (.php)
├── media_cache/        # локальный кеш скачанных файлов
├── serve.php           # запуск встроенного сервера с учётом WEB_HOST/WEB_PORT
├── .env.example
└── CLAUDE.md
```

## Запуск

```bash
cd web-php
cp .env.example .env
# Отредактировать MYSQL_*, TELEGRAM_BOT_TOKEN (для медиа), при необходимости WEB_HOST/WEB_PORT

# Вариант 1 — через лаунчер (читает WEB_HOST/WEB_PORT из .env, печатает предупреждение
# об отсутствии авторизации при не-loopback хосте):
php serve.php

# Вариант 2 — напрямую:
php -S 127.0.0.1:8000 -t public public/index.php

# Открыть http://127.0.0.1:8000
```

Под Apache/nginx: направить все запросы (rewrite) на `public/index.php`, docroot — `public/`.

**Требования:** PHP 8.0+ с расширениями `pdo_mysql`, `curl`, `mbstring`, `json`.

## Конфигурация (.env)

Идентична Python-версии. `MEDIA_CACHE_DIR` и относительные пути в `ALLOWED_MEDIA_DIRS`
резолвятся относительно каталога `web-php/`.

| Переменная | По умолчанию | Назначение |
|-----------|--------------|------------|
| `WEB_HOST` | `127.0.0.1` | Интерфейс биндинга (используется `serve.php`) |
| `WEB_PORT` | `8000` | Порт биндинга (используется `serve.php`) |
| `MYSQL_*` | — | Доступ к общей с ботом БД |
| `TELEGRAM_BOT_TOKEN` | — | Только для fallback-скачивания медиа |
| `MEDIA_CACHE_DIR` | `media_cache` | Кеш fallback-файлов |
| `ALLOWED_MEDIA_DIRS` | `media_cache` + `../python/storage` + `../rust/storage` | Whitelist путей для `/media/{id}` |
| `ALLOWED_ORIGINS` | — | Доп. Origin/Referer для POST (CSRF) |

## Маршруты (совпадают с Python-версией)

- **`GET /`** — лента сообщений: фильтры (чат, пользователь, тип, даты, поиск),
  пагинация (25–1000, шаг 25, по умолчанию 100), глобальная нумерация строк,
  инлайн-плеер/миниатюры/ссылки на документы, маркер удалённых, маркер орфо-исправления
  ✏️ с подсказкой «было → стало» (`Db::listMessages` → `spelling_hint`, `correctionPairs`).
- **`GET /message/{id}`** — карточка сообщения: медиа, ссылки, орфо-подсказки, кнопки
  soft-delete / restore / hard-delete.
- **`POST /message/{id}/delete|restore|hard-delete`** — управление soft-delete (303 redirect).
- **`GET /users`**, **`GET /stats`**, **`GET /corrections`**, **`GET /tips`** — как в Python.
- **`GET /media/{id}`** — отдача файла: `local_path` бота → веб-кеш → скачивание из Telegram.
- **`GET /api/stats/per-day?days=30`** — JSON для бар-чарта.

## Безопасность (паритет с Python-версией)

- ✅ **SQL injection** — все пользовательские параметры через PDO prepared statements
  (`?`); `LIMIT/OFFSET` — только валидированные `(int)`. DDL — хардкод-имена колонок.
- ✅ **XSS (stored)** — весь вывод данных из БД экранируется хелпером `e()`
  (htmlspecialchars, ENT_QUOTES, UTF-8) — аналог Jinja autoescape.
- ✅ **MIME-sniffing XSS в `/media/{id}`** — `X-Content-Type-Options: nosniff` на всех
  файлах; inline только `image/*`, `audio/*`, `video/*`, `application/pdf` (SVG — нет),
  остальное — `Content-Disposition: attachment` (`stream_file` в `public/index.php`).
- ✅ **CSP и заголовки безопасности** — для всех ответов выставляются строгая
  `Content-Security-Policy` (`default-src 'self'`, без inline-скриптов/фреймов),
  `X-Frame-Options: DENY`, `Referrer-Policy: same-origin`, `X-Content-Type-Options:
  nosniff`. Inline-скрипт графика вынесен в `data-chart`-атрибут, inline-обработчики
  (`confirm`, ссылка «Назад») заменены на классы `js-confirm`/`js-back` в `app.js`.
- ✅ **Небезопасные схемы ссылок** — URL в карточке кликабелен только при `http(s)`,
  иначе показывается как текст (`<code>`).
- ✅ **CSRF** — same-origin guard для POST/PUT/PATCH/DELETE в `public/index.php`;
  доп. источники — `ALLOWED_ORIGINS`.
- ✅ **Path traversal** — в `/media/{id}` `local_path` проверяется на принадлежность
  `ALLOWED_MEDIA_DIRS` (иначе 403); имя кеш-файла из `file_unique_id`/расширения
  санитизируется в `Telegram.php` (defense-in-depth).
- ✅ **DoS через `q`** — длина поиска ограничена 200 символами (`mb_substr`).
- ✅ **Без утечки трейсов** — `set_exception_handler` + `display_errors=0`: клиенту
  общий 500, детали только в лог.
- ✅ **Range-запросы** — `/media` отдаёт `206 Partial Content` (перемотка аудио/видео),
  как FastAPI FileResponse.
- ✅ **Command injection** — в `serve.php` `WEB_HOST` из `.env` экранируется
  (`escapeshellarg`) перед запуском `php -S`.

> Проверено вживую (PHP) на временной БД из `schema.sql`: security-заголовки и CSP
> присутствуют, `/media` HTML-файла отдаётся `attachment`+`nosniff` (не исполняется),
> Range → 206, `javascript:`-ссылка не превращается в `href`, XSS экранируется.

**Что должен обеспечить оператор** — то же, что и в `../web`:
авторизация отсутствует (слушать только `127.0.0.1` или ставить за nginx + auth + HTTPS),
отдельный MySQL-пользователь, права на `.env`, ограничение доступа к `media_cache/`.

## Отличия от Python-версии

- Один процесс на запрос (модель PHP), без пула соединений — для просмотрщика достаточно.
- Скачивание медиа синхронное (cURL) вместо async httpx.
- Запускать можно одновременно с Python-вебом и ботами (та же БД, updates не потребляются).

## Связь со схемой БД

Зависит от тех же таблиц, что и `../web`. При старте выполняет идемпотентные миграции
(`Db::ensureColumns`): `messages.deleted_at`, `media.local_path`, создание `daily_tips`
на свежей БД. При изменении схемы синхронизировать с `schema.sql`, ботами и `../web/db.py`.
