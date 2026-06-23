use mysql::prelude::Queryable;
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

pub struct DbMessage {
    pub message_id: i64,
    pub user_id: i64,
    pub user_username: Option<String>,
    pub user_first_name: Option<String>,
    pub user_last_name: Option<String>,
    pub chat_id: i64,
    pub chat_title: Option<String>,
    pub chat_type: Option<String>,
    pub text: Option<String>,
    pub message_type: String,
}

pub struct Media {
    pub message_id: i64,
    pub media_type: String,
    pub file_id: String,
    pub file_unique_id: String,
    pub file_name: Option<String>,
    pub file_size: Option<i64>,
    pub duration: Option<i32>,
    pub mime_type: Option<String>,
    pub local_path: Option<String>,
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

    pub async fn insert_message(&self, msg: &DbMessage) -> Result<()> {
        let mut conn = self.get_connection()?;

        let query = "INSERT IGNORE INTO messages
                    (message_id, user_id, user_username, user_first_name, user_last_name,
                     chat_id, chat_title, chat_type, text, message_type)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

        conn.exec_drop(query, (
            &msg.message_id, &msg.user_id,
            &msg.user_username, &msg.user_first_name, &msg.user_last_name,
            &msg.chat_id, &msg.chat_title, &msg.chat_type,
            &msg.text, &msg.message_type,
        ))
        .map_err(|e| {
            error!("Failed to insert message {}: {}", msg.message_id, e);
            Error::Database(format!("Failed to insert message: {}", e))
        })?;

        Ok(())
    }

    pub async fn insert_media(&self, media: &Media) -> Result<i64> {
        let mut conn = self.get_connection()?;

        let query = "INSERT INTO media
                    (message_id, type, file_id, file_unique_id, file_name, file_size, duration, mime_type, local_path)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)";

        conn.exec_drop(query, (
            &media.message_id,
            &media.media_type,
            &media.file_id,
            &media.file_unique_id,
            &media.file_name,
            &media.file_size,
            &media.duration,
            &media.mime_type,
            &media.local_path,
        ))
        .map_err(|e| {
            error!("Failed to insert media for message {}: {}", media.message_id, e);
            Error::Database(format!("Failed to insert media: {}", e))
        })?;

        Ok(conn.last_insert_id() as i64)
    }

    pub async fn update_media_local_path(&self, media_id: i64, local_path: &str) -> Result<()> {
        let mut conn = self.get_connection()?;
        conn.exec_drop(
            "UPDATE media SET local_path = ? WHERE media_id = ?",
            (local_path, media_id),
        )
        .map_err(|e| Error::Database(format!("Failed to update local_path: {}", e)))?;
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

    /// chat_id самого активного непубличного (группового) чата, или None.
    /// Fallback, когда TIP_CHAT_ID не задан — выбирает чат с наибольшим числом сообщений.
    pub async fn get_most_active_chat_id(&self) -> Result<Option<i64>> {
        let mut conn = self.get_connection()?;
        let row: Option<i64> = conn.query_first(
            "SELECT chat_id FROM messages
             WHERE chat_id IS NOT NULL
               AND (chat_type IS NULL OR chat_type <> 'private')
             GROUP BY chat_id
             ORDER BY COUNT(*) DESC
             LIMIT 1",
        )
        .map_err(|e| Error::Database(format!("Failed to detect most active chat: {}", e)))?;
        Ok(row)
    }

    /// До `limit` последних успешно отправленных советов чата (новые сверху).
    /// Передаются модели как «уже было», чтобы она не повторялась.
    pub async fn get_recent_tips(&self, chat_id: i64, limit: u32) -> Result<Vec<String>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let mut conn = self.get_connection()?;
        let rows: Vec<String> = conn
            .exec(
                "SELECT response FROM daily_tips
                 WHERE chat_id = ?
                   AND sent_to_chat = TRUE
                   AND response IS NOT NULL
                   AND response <> ''
                 ORDER BY created_at DESC
                 LIMIT ?",
                (chat_id, limit),
            )
            .map_err(|e| Error::Database(format!("Failed to fetch recent daily tips: {}", e)))?;
        Ok(rows)
    }

    /// Сохранить запрос/ответ совета дня и статус отправки.
    pub async fn insert_daily_tip(
        &self,
        chat_id: i64,
        model: &str,
        prompt: &str,
        response: Option<&str>,
        sent_to_chat: bool,
        error: Option<&str>,
    ) -> Result<()> {
        let mut conn = self.get_connection()?;

        let query = "INSERT INTO daily_tips
                    (chat_id, model, prompt, response, sent_to_chat, error)
                    VALUES (?, ?, ?, ?, ?, ?)";

        conn.exec_drop(query, (chat_id, model, prompt, response, sent_to_chat, error))
            .map_err(|e| {
                error!("Failed to insert daily tip: {}", e);
                Error::Database(format!("Failed to insert daily tip: {}", e))
            })?;

        Ok(())
    }

    pub async fn insert_service_event(
        &self,
        chat_id: i64,
        chat_title: Option<&str>,
        event_type: &str,
        user_id: Option<i64>,
        user_username: Option<&str>,
        user_first_name: Option<&str>,
        data: serde_json::Value,
    ) -> Result<()> {
        let mut conn = self.get_connection()?;

        let query = "INSERT INTO service_events
                    (chat_id, chat_title, event_type, user_id, user_username, user_first_name, data)
                    VALUES (?, ?, ?, ?, ?, ?, ?)";

        let data_str = data.to_string();

        conn.exec_drop(query, (
            chat_id, chat_title, event_type, user_id,
            user_username, user_first_name, data_str,
        ))
        .map_err(|e| {
            error!("Failed to insert service event: {}", e);
            Error::Database(format!("Failed to insert service event: {}", e))
        })?;

        Ok(())
    }
}
