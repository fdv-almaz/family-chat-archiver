use serde_json::json;
use log::error;
use crate::error::{Error, Result};
use super::DbPool;

pub struct User {
    pub user_id: i64,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub is_bot: bool,
}

pub struct Message {
    pub message_id: i64,
    pub user_id: i64,
    pub chat_id: i64,
    pub text: Option<String>,
    pub message_type: String,
}

pub struct Media {
    pub message_id: i64,
    pub media_type: String,
    pub file_id: String,
    pub file_unique_id: String,
    pub file_size: Option<i32>,
    pub mime_type: Option<String>,
}

pub struct SpellingCorrection {
    pub message_id: i64,
    pub original_text: String,
    pub corrected_text: String,
    pub errors: String,
    pub sent_to_chat: bool,
}

impl DbPool {
    pub async fn insert_or_update_user(&self, user: &User) -> Result<()> {
        let mut conn = self.get_connection()?;

        let query = "INSERT INTO users (user_id, username, first_name, last_name, is_bot)
                    VALUES (?, ?, ?, ?, ?)
                    ON DUPLICATE KEY UPDATE
                        username = VALUES(username),
                        first_name = VALUES(first_name),
                        last_name = VALUES(last_name)";

        conn.exec_drop(query, (&user.user_id, &user.username, &user.first_name, &user.last_name, &user.is_bot))
            .map_err(|e| {
                error!("Failed to insert/update user {}: {}", user.user_id, e);
                Error::Database(format!("Failed to insert/update user: {}", e))
            })?;

        Ok(())
    }

    pub async fn insert_message(&self, msg: &Message) -> Result<()> {
        let mut conn = self.get_connection()?;

        let query = "INSERT INTO messages (message_id, user_id, chat_id, text, message_type)
                    VALUES (?, ?, ?, ?, ?)";

        conn.exec_drop(query, (&msg.message_id, &msg.user_id, &msg.chat_id, &msg.text, &msg.message_type))
            .map_err(|e| {
                error!("Failed to insert message {}: {}", msg.message_id, e);
                Error::Database(format!("Failed to insert message: {}", e))
            })?;

        Ok(())
    }

    pub async fn insert_media(&self, media: &Media) -> Result<()> {
        let mut conn = self.get_connection()?;

        let query = "INSERT INTO media (message_id, type, file_id, file_unique_id, file_size, mime_type)
                    VALUES (?, ?, ?, ?, ?, ?)";

        conn.exec_drop(query, (
            &media.message_id,
            &media.media_type,
            &media.file_id,
            &media.file_unique_id,
            &media.file_size,
            &media.mime_type,
        ))
        .map_err(|e| {
            error!("Failed to insert media for message {}: {}", media.message_id, e);
            Error::Database(format!("Failed to insert media: {}", e))
        })?;

        Ok(())
    }

    pub async fn insert_link(&self, message_id: i64, url: &str) -> Result<()> {
        let mut conn = self.get_connection()?;

        let query = "INSERT INTO links (message_id, url) VALUES (?, ?)";

        conn.exec_drop(query, (message_id, url))
            .map_err(|e| {
                error!("Failed to insert link for message {}: {}", message_id, e);
                Error::Database(format!("Failed to insert link: {}", e))
            })?;

        Ok(())
    }

    pub async fn insert_spelling_correction(&self, correction: &SpellingCorrection) -> Result<()> {
        let mut conn = self.get_connection()?;

        let query = "INSERT INTO spelling_corrections
                    (message_id, original_text, corrected_text, errors, sent_to_chat)
                    VALUES (?, ?, ?, ?, ?)";

        conn.exec_drop(query, (
            &correction.message_id,
            &correction.original_text,
            &correction.corrected_text,
            &correction.errors,
            &correction.sent_to_chat,
        ))
        .map_err(|e| {
            error!("Failed to insert spelling correction for message {}: {}", correction.message_id, e);
            Error::Database(format!("Failed to insert spelling correction: {}", e))
        })?;

        Ok(())
    }

    pub async fn insert_service_event(&self, chat_id: i64, event_type: &str, user_id: Option<i64>, data: serde_json::Value) -> Result<()> {
        let mut conn = self.get_connection()?;

        let query = "INSERT INTO service_events (chat_id, event_type, user_id, data)
                    VALUES (?, ?, ?, ?)";

        let data_str = data.to_string();

        conn.exec_drop(query, (chat_id, event_type, user_id, data_str))
            .map_err(|e| {
                error!("Failed to insert service event: {}", e);
                Error::Database(format!("Failed to insert service event: {}", e))
            })?;

        Ok(())
    }
}
