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
                user_username VARCHAR(32),
                user_first_name VARCHAR(255),
                user_last_name VARCHAR(255),
                chat_id BIGINT,
                chat_title VARCHAR(255),
                chat_type VARCHAR(20),
                text LONGTEXT,
                message_type VARCHAR(20),
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE SET NULL,
                INDEX idx_chat_date (chat_id, created_at),
                INDEX idx_user_date (user_id, created_at)
            ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        """)

        # Auto-add columns if table existed before with old schema
        for col_def in [
            ("user_username", "VARCHAR(32)"),
            ("user_first_name", "VARCHAR(255)"),
            ("user_last_name", "VARCHAR(255)"),
            ("chat_title", "VARCHAR(255)"),
            ("chat_type", "VARCHAR(20)"),
        ]:
            try:
                cursor.execute(f"ALTER TABLE messages ADD COLUMN {col_def[0]} {col_def[1]}")
                logger.info(f'Added column {col_def[0]} to messages table')
            except Error:
                pass  # Column already exists

        # Migrate message_type from ENUM to VARCHAR if needed (to allow new types)
        try:
            cursor.execute("ALTER TABLE messages MODIFY COLUMN message_type VARCHAR(20)")
        except Error:
            pass

        cursor.execute("""
            CREATE TABLE IF NOT EXISTS media (
                media_id INT AUTO_INCREMENT PRIMARY KEY,
                message_id BIGINT,
                type VARCHAR(20),
                file_id VARCHAR(255),
                file_unique_id VARCHAR(255),
                file_name VARCHAR(255),
                file_size BIGINT,
                duration INT,
                mime_type VARCHAR(100),
                FOREIGN KEY (message_id) REFERENCES messages(message_id) ON DELETE CASCADE,
                INDEX idx_message_id (message_id),
                INDEX idx_type (type)
            ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        """)

        # Migrate media columns for existing DBs
        try:
            cursor.execute("ALTER TABLE media MODIFY COLUMN type VARCHAR(20)")
        except Error:
            pass
        try:
            cursor.execute("ALTER TABLE media MODIFY COLUMN file_size BIGINT")
        except Error:
            pass
        for col_def in [("file_name", "VARCHAR(255)"), ("duration", "INT")]:
            try:
                cursor.execute(f"ALTER TABLE media ADD COLUMN {col_def[0]} {col_def[1]}")
                logger.info(f'Added column {col_def[0]} to media table')
            except Error:
                pass

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
                chat_title VARCHAR(255),
                event_type VARCHAR(50),
                user_id BIGINT,
                user_username VARCHAR(32),
                user_first_name VARCHAR(255),
                data JSON,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        """)

        for col_def in [
            ("chat_title", "VARCHAR(255)"),
            ("user_username", "VARCHAR(32)"),
            ("user_first_name", "VARCHAR(255)"),
        ]:
            try:
                cursor.execute(f"ALTER TABLE service_events ADD COLUMN {col_def[0]} {col_def[1]}")
                logger.info(f'Added column {col_def[0]} to service_events table')
            except Error:
                pass

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

def insert_message(message_id, user_id, chat_id, text, message_type,
                   user_username=None, user_first_name=None, user_last_name=None,
                   chat_title=None, chat_type=None):
    db = DBPool()
    conn = db.get_connection()
    cursor = conn.cursor()

    try:
        cursor.execute("""
            INSERT IGNORE INTO messages
            (message_id, user_id, user_username, user_first_name, user_last_name,
             chat_id, chat_title, chat_type, text, message_type)
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
        """, (message_id, user_id, user_username, user_first_name, user_last_name,
              chat_id, chat_title, chat_type, text, message_type))
        conn.commit()
    except Error as e:
        logger.error(f'Failed to insert message {message_id}: {e}')
        raise
    finally:
        cursor.close()
        conn.close()

def insert_media(message_id, media_type, file_id, file_unique_id,
                 file_size=None, mime_type=None, file_name=None, duration=None):
    db = DBPool()
    conn = db.get_connection()
    cursor = conn.cursor()

    try:
        cursor.execute("""
            INSERT INTO media
            (message_id, type, file_id, file_unique_id, file_name, file_size, duration, mime_type)
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s)
        """, (message_id, media_type, file_id, file_unique_id,
              file_name, file_size, duration, mime_type))
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

def insert_service_event(chat_id, event_type, user_id=None, data_json=None,
                         chat_title=None, user_username=None, user_first_name=None):
    db = DBPool()
    conn = db.get_connection()
    cursor = conn.cursor()

    try:
        cursor.execute("""
            INSERT INTO service_events
            (chat_id, chat_title, event_type, user_id, user_username, user_first_name, data)
            VALUES (%s, %s, %s, %s, %s, %s, %s)
        """, (chat_id, chat_title, event_type, user_id, user_username, user_first_name, data_json))
        conn.commit()
    except Error as e:
        logger.error(f'Failed to insert service event: {e}')
        raise
    finally:
        cursor.close()
        conn.close()
