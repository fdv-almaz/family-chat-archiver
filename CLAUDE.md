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

**Таблица users**
```sql
CREATE TABLE users (
  user_id BIGINT PRIMARY KEY,
  username VARCHAR(32),
  first_name VARCHAR(255),
  last_name VARCHAR(255),
  is_bot BOOLEAN,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

**Таблица messages**
```sql
CREATE TABLE messages (
  message_id BIGINT PRIMARY KEY,
  user_id BIGINT,
  chat_id BIGINT,
  text LONGTEXT,
  message_type ENUM('text', 'photo', 'video', 'document', 'voice', 'service'),
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (user_id) REFERENCES users(user_id)
);
```

**Таблица media**
```sql
CREATE TABLE media (
  media_id INT AUTO_INCREMENT PRIMARY KEY,
  message_id BIGINT,
  type ENUM('photo', 'video', 'document', 'voice'),
  file_id VARCHAR(255),
  file_unique_id VARCHAR(255),
  file_size INT,
  mime_type VARCHAR(100),
  FOREIGN KEY (message_id) REFERENCES messages(message_id)
);
```

**Таблица links**
```sql
CREATE TABLE links (
  link_id INT AUTO_INCREMENT PRIMARY KEY,
  message_id BIGINT,
  url VARCHAR(2048),
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (message_id) REFERENCES messages(message_id)
);
```

**Таблица spelling_corrections**
```sql
CREATE TABLE spelling_corrections (
  correction_id INT AUTO_INCREMENT PRIMARY KEY,
  message_id BIGINT,
  original_text LONGTEXT,
  corrected_text LONGTEXT,
  errors JSON,
  sent_to_chat BOOLEAN DEFAULT FALSE,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (message_id) REFERENCES messages(message_id)
);
```

**Таблица service_events**
```sql
CREATE TABLE service_events (
  event_id INT AUTO_INCREMENT PRIMARY KEY,
  chat_id BIGINT,
  event_type VARCHAR(50),
  user_id BIGINT,
  data JSON,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

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
```
