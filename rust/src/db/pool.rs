use mysql::{Pool, OptsBuilder};
use mysql::prelude::Queryable;
use std::sync::Arc;
use log::info;
use crate::config::Config;
use crate::error::{Error, Result};

pub struct DbPool {
    pool: Arc<Pool>,
}

impl DbPool {
    pub fn new(config: &Config) -> Result<Self> {
        let opts = OptsBuilder::new()
            .ip_or_hostname(Some(&config.mysql_host))
            .tcp_port(config.mysql_port)
            .user(Some(&config.mysql_user))
            .pass(Some(&config.mysql_password))
            .db_name(Some(&config.mysql_database));

        let pool = Pool::new(opts)
            .map_err(|e| Error::Database(format!("Failed to create pool: {}", e)))?;

        info!("Database pool initialized: {}@{}:{}/{}",
            config.mysql_user, config.mysql_host, config.mysql_port, config.mysql_database);

        Ok(DbPool {
            pool: Arc::new(pool),
        })
    }

    pub fn get_connection(&self) -> Result<mysql::PooledConn> {
        self.pool.get_conn()
            .map_err(|e| Error::Database(format!("Failed to get connection: {}", e)))
    }

    pub async fn create_tables(&self) -> Result<()> {
        let mut conn = self.get_connection()?;

        let queries = vec![
            "CREATE TABLE IF NOT EXISTS users (
                user_id BIGINT PRIMARY KEY,
                username VARCHAR(32),
                first_name VARCHAR(255),
                last_name VARCHAR(255),
                is_bot BOOLEAN DEFAULT FALSE,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",

            "CREATE TABLE IF NOT EXISTS messages (
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
                deleted_at TIMESTAMP NULL,
                FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE SET NULL,
                INDEX idx_chat_date (chat_id, created_at),
                INDEX idx_user_date (user_id, created_at)
            ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",

            "CREATE TABLE IF NOT EXISTS media (
                media_id INT AUTO_INCREMENT PRIMARY KEY,
                message_id BIGINT,
                type VARCHAR(20),
                file_id VARCHAR(255),
                file_unique_id VARCHAR(255),
                file_name VARCHAR(255),
                file_size BIGINT,
                duration INT,
                mime_type VARCHAR(100),
                local_path VARCHAR(500),
                FOREIGN KEY (message_id) REFERENCES messages(message_id) ON DELETE CASCADE,
                INDEX idx_message_id (message_id),
                INDEX idx_type (type)
            ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",

            "CREATE TABLE IF NOT EXISTS links (
                link_id INT AUTO_INCREMENT PRIMARY KEY,
                message_id BIGINT,
                url VARCHAR(2048),
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (message_id) REFERENCES messages(message_id) ON DELETE CASCADE
            ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",

            "CREATE TABLE IF NOT EXISTS spelling_corrections (
                correction_id INT AUTO_INCREMENT PRIMARY KEY,
                message_id BIGINT,
                original_text LONGTEXT,
                corrected_text LONGTEXT,
                errors JSON,
                sent_to_chat BOOLEAN DEFAULT FALSE,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (message_id) REFERENCES messages(message_id) ON DELETE CASCADE
            ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",

            "CREATE TABLE IF NOT EXISTS service_events (
                event_id INT AUTO_INCREMENT PRIMARY KEY,
                chat_id BIGINT,
                chat_title VARCHAR(255),
                event_type VARCHAR(50),
                user_id BIGINT,
                user_username VARCHAR(32),
                user_first_name VARCHAR(255),
                data JSON,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",

            "CREATE TABLE IF NOT EXISTS daily_tips (
                tip_id INT AUTO_INCREMENT PRIMARY KEY,
                chat_id BIGINT,
                model VARCHAR(64),
                prompt LONGTEXT,
                response LONGTEXT,
                sent_to_chat BOOLEAN DEFAULT FALSE,
                error LONGTEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_chat_date (chat_id, created_at)
            ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",
        ];

        for query in queries {
            conn.query_drop(query)
                .map_err(|e| Error::Database(format!("Failed to create table: {}", e)))?;
        }

        // Auto-add columns if table existed with old schema (idempotent migrations)
        let migrations = vec![
            "ALTER TABLE messages ADD COLUMN user_username VARCHAR(32)",
            "ALTER TABLE messages ADD COLUMN user_first_name VARCHAR(255)",
            "ALTER TABLE messages ADD COLUMN user_last_name VARCHAR(255)",
            "ALTER TABLE messages ADD COLUMN chat_title VARCHAR(255)",
            "ALTER TABLE messages ADD COLUMN chat_type VARCHAR(20)",
            "ALTER TABLE messages MODIFY COLUMN message_type VARCHAR(20)",
            "ALTER TABLE messages ADD COLUMN deleted_at TIMESTAMP NULL",
            "ALTER TABLE media MODIFY COLUMN type VARCHAR(20)",
            "ALTER TABLE media MODIFY COLUMN file_size BIGINT",
            "ALTER TABLE media ADD COLUMN file_name VARCHAR(255)",
            "ALTER TABLE media ADD COLUMN duration INT",
            "ALTER TABLE media ADD COLUMN local_path VARCHAR(500)",
            "ALTER TABLE service_events ADD COLUMN chat_title VARCHAR(255)",
            "ALTER TABLE service_events ADD COLUMN user_username VARCHAR(32)",
            "ALTER TABLE service_events ADD COLUMN user_first_name VARCHAR(255)",
        ];

        for migration in migrations {
            let _ = conn.query_drop(migration); // Ignore errors (column may already exist)
        }

        info!("All tables created/verified successfully");
        Ok(())
    }
}

impl Clone for DbPool {
    fn clone(&self) -> Self {
        DbPool {
            pool: Arc::clone(&self.pool),
        }
    }
}
