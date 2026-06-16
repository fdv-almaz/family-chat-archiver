# CLAUDE.md — Python реализация

## Структура проекта

```
python/
├── bot.py              # Основной файл бота
├── config.py           # Конфигурация (env переменные)
├── db.py               # Работа с MySQL
├── processors.py       # Обработчики сообщений и медиа
├── spelling.py         # Проверка орфографии (YandexSpeller)
├── requirements.txt    # Зависимости
├── .env.example        # Шаблон конфига
└── CLAUDE.md          # Этот файл
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

**Создание БД:**
```bash
mysql -u user -p database < schema.sql
```

## Основные компоненты

### bot.py
- Инициализация TelegramBotAPI
- Основной polling loop
- Обработчики событий сообщений
- Graceful shutdown

### db.py
- Connection pooling к MySQL (использовать `DBPool`)
- CRUD операции для каждой таблицы
- Retry logic при потере соединения
- Транзакции для сохранения сообщения + медиа

### processors.py
- `extract_message_data()` — парсинг сообщения (текст, медиа, ссылки)
- `extract_user_data()` — информация об авторе
- `save_message_to_db()` — сохранение с обработкой ошибок
- `handle_media()` — загрузка и сохранение информации о медиа

### spelling.py
- `check_spelling()` — вызов YandexSpeller API
- `format_corrections()` — форматирование ошибок для вывода в чат
- Retry logic при недоступности API

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
```
