use mysql::{Pool, Opts, OptsBuilder};
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
                chat_id BIGINT,
                text LONGTEXT,
                message_type ENUM('text', 'photo', 'video', 'document', 'voice', 'service'),
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE SET NULL
            ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",

            "CREATE TABLE IF NOT EXISTS media (
                media_id INT AUTO_INCREMENT PRIMARY KEY,
                message_id BIGINT,
                type ENUM('photo', 'video', 'document', 'voice'),
                file_id VARCHAR(255),
                file_unique_id VARCHAR(255),
                file_size INT,
                mime_type VARCHAR(100),
                FOREIGN KEY (message_id) REFERENCES messages(message_id) ON DELETE CASCADE
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
                event_type VARCHAR(50),
                user_id BIGINT,
                data JSON,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",
        ];

        for query in queries {
            conn.query_drop(query)
                .map_err(|e| Error::Database(format!("Failed to create table: {}", e)))?;
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
