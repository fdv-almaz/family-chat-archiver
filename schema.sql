-- Схема базы данных для Family Chat Archiver

CREATE DATABASE IF NOT EXISTS family_chat;
USE family_chat;

-- Пользователи
CREATE TABLE IF NOT EXISTS users (
    user_id BIGINT PRIMARY KEY,
    username VARCHAR(32),
    first_name VARCHAR(255),
    last_name VARCHAR(255),
    is_bot BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- Сообщения
CREATE TABLE IF NOT EXISTS messages (
    message_id BIGINT PRIMARY KEY,
    user_id BIGINT,
    chat_id BIGINT,
    text LONGTEXT,
    message_type ENUM('text', 'photo', 'video', 'document', 'voice', 'service'),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE SET NULL,
    INDEX idx_chat_date (chat_id, created_at),
    INDEX idx_user_date (user_id, created_at)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- Медиа-файлы
CREATE TABLE IF NOT EXISTS media (
    media_id INT AUTO_INCREMENT PRIMARY KEY,
    message_id BIGINT,
    type ENUM('photo', 'video', 'document', 'voice'),
    file_id VARCHAR(255),
    file_unique_id VARCHAR(255),
    file_size INT,
    mime_type VARCHAR(100),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (message_id) REFERENCES messages(message_id) ON DELETE CASCADE,
    INDEX idx_message_id (message_id)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- Ссылки
CREATE TABLE IF NOT EXISTS links (
    link_id INT AUTO_INCREMENT PRIMARY KEY,
    message_id BIGINT,
    url VARCHAR(2048),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (message_id) REFERENCES messages(message_id) ON DELETE CASCADE,
    INDEX idx_message_id (message_id)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- Исправления орфографии
CREATE TABLE IF NOT EXISTS spelling_corrections (
    correction_id INT AUTO_INCREMENT PRIMARY KEY,
    message_id BIGINT,
    original_text LONGTEXT,
    corrected_text LONGTEXT,
    errors JSON,
    sent_to_chat BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (message_id) REFERENCES messages(message_id) ON DELETE CASCADE,
    INDEX idx_message_id (message_id),
    INDEX idx_created_date (created_at)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- Служебные события
CREATE TABLE IF NOT EXISTS service_events (
    event_id INT AUTO_INCREMENT PRIMARY KEY,
    chat_id BIGINT,
    event_type VARCHAR(50),
    user_id BIGINT,
    data JSON,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_chat_date (chat_id, created_at),
    INDEX idx_event_type (event_type)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
