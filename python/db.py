import logging
import mysql.connector
from mysql.connector import pooling, Error
from config import MYSQL_HOST, MYSQL_PORT, MYSQL_USER, MYSQL_PASSWORD, MYSQL_DATABASE

logger = logging.getLogger(__name__)

class DBPool:
    _instance = None

    def __new__(cls):
        if cls._instance is None:
            cls._instance = super().__new__(cls)
            cls._instance._init_pool()
        return cls._instance

    def _init_pool(self):
        try:
            self.pool = pooling.MySQLConnectionPool(
                pool_name="family_chat_pool",
                pool_size=5,
                pool_reset_session=True,
                host=MYSQL_HOST,
                port=MYSQL_PORT,
                user=MYSQL_USER,
                password=MYSQL_PASSWORD,
                database=MYSQL_DATABASE
            )
            logger.info('Database pool initialized successfully')
        except Error as e:
            logger.error(f'Failed to initialize database pool: {e}')
            raise

    def get_connection(self):
        try:
            return self.pool.get_connection()
        except Error as e:
            logger.error(f'Failed to get connection from pool: {e}')
            raise

def create_tables():
    db = DBPool()
    conn = db.get_connection()
    cursor = conn.cursor()

    try:
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS users (
                user_id BIGINT PRIMARY KEY,
                username VARCHAR(32),
                first_name VARCHAR(255),
                last_name VARCHAR(255),
                is_bot BOOLEAN DEFAULT FALSE,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        """)

        cursor.execute("""
            CREATE TABLE IF NOT EXISTS messages (
                message_id BIGINT PRIMARY KEY,
                user_id BIGINT,
                chat_id BIGINT,
                text LONGTEXT,
                message_type ENUM('text', 'photo', 'video', 'document', 'voice', 'service'),
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE SET NULL
            ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        """)

        cursor.execute("""
            CREATE TABLE IF NOT EXISTS media (
                media_id INT AUTO_INCREMENT PRIMARY KEY,
                message_id BIGINT,
                type ENUM('photo', 'video', 'document', 'voice'),
                file_id VARCHAR(255),
                file_unique_id VARCHAR(255),
                file_size INT,
                mime_type VARCHAR(100),
                FOREIGN KEY (message_id) REFERENCES messages(message_id) ON DELETE CASCADE
            ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        """)

        cursor.execute("""
            CREATE TABLE IF NOT EXISTS links (
                link_id INT AUTO_INCREMENT PRIMARY KEY,
                message_id BIGINT,
                url VARCHAR(2048),
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (message_id) REFERENCES messages(message_id) ON DELETE CASCADE
            ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        """)

        cursor.execute("""
            CREATE TABLE IF NOT EXISTS spelling_corrections (
                correction_id INT AUTO_INCREMENT PRIMARY KEY,
                message_id BIGINT,
                original_text LONGTEXT,
                corrected_text LONGTEXT,
                errors JSON,
                sent_to_chat BOOLEAN DEFAULT FALSE,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (message_id) REFERENCES messages(message_id) ON DELETE CASCADE
            ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        """)

        cursor.execute("""
            CREATE TABLE IF NOT EXISTS service_events (
                event_id INT AUTO_INCREMENT PRIMARY KEY,
                chat_id BIGINT,
                event_type VARCHAR(50),
                user_id BIGINT,
                data JSON,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        """)

        conn.commit()
        logger.info('All tables created/verified successfully')
    except Error as e:
        logger.error(f'Failed to create tables: {e}')
        raise
    finally:
        cursor.close()
        conn.close()

def insert_or_update_user(user):
    db = DBPool()
    conn = db.get_connection()
    cursor = conn.cursor()

    try:
        cursor.execute("""
            INSERT INTO users (user_id, username, first_name, last_name, is_bot)
            VALUES (%s, %s, %s, %s, %s)
            ON DUPLICATE KEY UPDATE
                username = VALUES(username),
                first_name = VALUES(first_name),
                last_name = VALUES(last_name)
        """, (
            user.id,
            user.username,
            user.first_name,
            user.last_name,
            user.is_bot
        ))
        conn.commit()
    except Error as e:
        logger.error(f'Failed to insert/update user {user.id}: {e}')
        raise
    finally:
        cursor.close()
        conn.close()

def insert_message(message_id, user_id, chat_id, text, message_type):
    db = DBPool()
    conn = db.get_connection()
    cursor = conn.cursor()

    try:
        cursor.execute("""
            INSERT INTO messages (message_id, user_id, chat_id, text, message_type)
            VALUES (%s, %s, %s, %s, %s)
        """, (message_id, user_id, chat_id, text, message_type))
        conn.commit()
    except Error as e:
        logger.error(f'Failed to insert message {message_id}: {e}')
        raise
    finally:
        cursor.close()
        conn.close()

def insert_media(message_id, media_type, file_id, file_unique_id, file_size=None, mime_type=None):
    db = DBPool()
    conn = db.get_connection()
    cursor = conn.cursor()

    try:
        cursor.execute("""
            INSERT INTO media (message_id, type, file_id, file_unique_id, file_size, mime_type)
            VALUES (%s, %s, %s, %s, %s, %s)
        """, (message_id, media_type, file_id, file_unique_id, file_size, mime_type))
        conn.commit()
    except Error as e:
        logger.error(f'Failed to insert media for message {message_id}: {e}')
        raise
    finally:
        cursor.close()
        conn.close()

def insert_link(message_id, url):
    db = DBPool()
    conn = db.get_connection()
    cursor = conn.cursor()

    try:
        cursor.execute("""
            INSERT INTO links (message_id, url)
            VALUES (%s, %s)
        """, (message_id, url))
        conn.commit()
    except Error as e:
        logger.error(f'Failed to insert link for message {message_id}: {e}')
        raise
    finally:
        cursor.close()
        conn.close()

def insert_spelling_correction(message_id, original_text, corrected_text, errors_json, sent_to_chat=False):
    db = DBPool()
    conn = db.get_connection()
    cursor = conn.cursor()

    try:
        cursor.execute("""
            INSERT INTO spelling_corrections
            (message_id, original_text, corrected_text, errors, sent_to_chat)
            VALUES (%s, %s, %s, %s, %s)
        """, (message_id, original_text, corrected_text, errors_json, sent_to_chat))
        conn.commit()
    except Error as e:
        logger.error(f'Failed to insert spelling correction for message {message_id}: {e}')
        raise
    finally:
        cursor.close()
        conn.close()

def insert_service_event(chat_id, event_type, user_id=None, data_json=None):
    db = DBPool()
    conn = db.get_connection()
    cursor = conn.cursor()

    try:
        cursor.execute("""
            INSERT INTO service_events (chat_id, event_type, user_id, data)
            VALUES (%s, %s, %s, %s)
        """, (chat_id, event_type, user_id, data_json))
        conn.commit()
    except Error as e:
        logger.error(f'Failed to insert service event: {e}')
        raise
    finally:
        cursor.close()
        conn.close()
