mod config;
mod db;
mod error;
mod processors;

use teloxide::prelude::*;
use teloxide::types::{Message as TgMessage, PhotoSize};
use log::{error, info, debug};
use serde_json::json;
use std::sync::Arc;

use config::Config;
use db::{DbPool, User, DbMessage, Media, SpellingCorrection};
use processors::{check_spelling, format_correction_message, format_chat_message, extract_urls};

const START_MESSAGE: &str = "<b>👋 Family Chat Archiver</b>\n\n\
Это бот для архивирования всех сообщений в семейной группе.\n\n\
<b>Основные возможности:</b>\n\
✅ Сохранение всех сообщений (текст, фото, видео, документы)\n\
✅ Сохранение информации об авторах\n\
✅ Проверка орфографии русскоязычных текстов\n\
✅ Сохранение ссылок и медиа-контента\n\
✅ Запись служебных событий (вход/выход участников)\n\n\
<b>Как это работает:</b>\n\
Бот автоматически архивирует все сообщения в группе без участия пользователя. Для исправления орфографии используется YandexSpeller API.\n\n\
<b>Хранение данных:</b>\n\
Все данные сохраняются в защищённой MySQL базе данных.\n\n\
<i>Бот работает в фоновом режиме и не требует команд.</i>";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let config = Config::from_env().map_err(|e| format!("Config error: {}", e))?;
    let db_pool = DbPool::new(&config).map_err(|e| format!("DB error: {}", e))?;

    db_pool.create_tables().await.map_err(|e| format!("DB init error: {}", e))?;

    let db_pool = Arc::new(db_pool);
    let bot = Bot::new(&config.telegram_bot_token);

    info!("Bot started, listening for messages...");

    let handler = Update::filter_message()
        .endpoint({
            let db = Arc::clone(&db_pool);
            move |bot: Bot, msg: TgMessage| {
                let db = Arc::clone(&db);
                async move { handle_message(bot, msg, db).await }
            }
        });

    Dispatcher::builder(bot, handler)
        .default_handler(|upd| async move {
            debug!("Unhandled update: {:?}", upd);
        })
        .error_handler(LoggingErrorHandler::with_custom_text("Dispatcher error"))
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    info!("Bot shutdown complete");
    Ok(())
}

async fn handle_message(
    bot: Bot,
    message: TgMessage,
    db: Arc<DbPool>,
) -> ResponseResult<()> {
    // Skip messages from bots to prevent loops
    if let Some(user) = message.from.as_ref() {
        if user.is_bot {
            return Ok(());
        }

        let db_user = User {
            user_id: user.id.0 as i64,
            username: user.username.clone(),
            first_name: user.first_name.clone(),
            last_name: user.last_name.clone(),
            is_bot: user.is_bot,
        };
        if let Err(e) = db.insert_or_update_user(&db_user).await {
            error!("Failed to save user: {}", e);
        }
    }

    // Handle /start and /help commands
    if let Some(text) = message.text() {
        if text == "/start" || text == "/help" {
            let _ = bot
                .send_message(message.chat.id, START_MESSAGE)
                .parse_mode(teloxide::types::ParseMode::Html)
                .await;
            return Ok(());
        }
    }

    if let Some(text) = message.text() {
        handle_text_message(&bot, &message, db.as_ref(), text).await;
    } else if let Some(photos) = message.photo() {
        handle_photo_message(&bot, &message, db.as_ref(), photos, message.caption()).await;
    } else if let Some(video) = message.video() {
        handle_media_message(
            &bot, &message, db.as_ref(), "video",
            &video.file.id, &video.file.unique_id, video.file.size as i32,
            None, message.caption()
        ).await;
    } else if let Some(document) = message.document() {
        let mime = document.mime_type.as_ref().map(|m| m.to_string());
        handle_media_message(
            &bot, &message, db.as_ref(), "document",
            &document.file.id, &document.file.unique_id, document.file.size as i32,
            mime.as_deref(),
            message.caption()
        ).await;
    } else if let Some(voice) = message.voice() {
        let mime = voice.mime_type.as_ref().map(|m| m.to_string());
        handle_media_message(
            &bot, &message, db.as_ref(), "voice",
            &voice.file.id, &voice.file.unique_id, voice.file.size as i32,
            mime.as_deref(),
            message.caption()
        ).await;
    } else if !message.new_chat_members().unwrap_or(&[]).is_empty() {
        handle_service_event(db.as_ref(), &message, "user_joined").await;
    } else if message.left_chat_member().is_some() {
        handle_service_event(db.as_ref(), &message, "user_left").await;
    } else if message.new_chat_title().is_some() {
        handle_service_event(db.as_ref(), &message, "title_changed").await;
    } else if message.new_chat_photo().is_some() {
        handle_service_event(db.as_ref(), &message, "photo_changed").await;
    }

    Ok(())
}

async fn handle_text_message(
    bot: &Bot,
    message: &TgMessage,
    db: &DbPool,
    text: &str,
) {
    let db_message = DbMessage {
        message_id: message.id.0 as i64,
        user_id: message.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0),
        chat_id: message.chat.id.0,
        text: Some(text.to_string()),
        message_type: "text".to_string(),
    };

    if let Err(e) = db.insert_message(&db_message).await {
        error!("Failed to save message: {}", e);
        return;
    }

    for url in extract_urls(text) {
        if let Err(e) = db.insert_link(message.id.0 as i64, &url).await {
            error!("Failed to save link: {}", e);
        }
    }

    process_spelling(bot, message, db, text).await;
}

async fn handle_photo_message(
    bot: &Bot,
    message: &TgMessage,
    db: &DbPool,
    photos: &[PhotoSize],
    caption: Option<&str>,
) {
    let caption_text = caption.unwrap_or("");

    let db_message = DbMessage {
        message_id: message.id.0 as i64,
        user_id: message.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0),
        chat_id: message.chat.id.0,
        text: Some(caption_text.to_string()),
        message_type: "photo".to_string(),
    };

    if let Err(e) = db.insert_message(&db_message).await {
        error!("Failed to save photo message: {}", e);
        return;
    }

    if let Some(photo) = photos.last() {
        let media = Media {
            message_id: message.id.0 as i64,
            media_type: "photo".to_string(),
            file_id: photo.file.id.clone(),
            file_unique_id: photo.file.unique_id.clone(),
            file_size: Some(photo.file.size as i32),
            mime_type: None,
        };
        if let Err(e) = db.insert_media(&media).await {
            error!("Failed to save media: {}", e);
        }
    }

    if !caption_text.is_empty() {
        process_spelling(bot, message, db, caption_text).await;
    }
}

async fn handle_media_message(
    bot: &Bot,
    message: &TgMessage,
    db: &DbPool,
    media_type: &str,
    file_id: &str,
    file_unique_id: &str,
    file_size: i32,
    mime_type: Option<&str>,
    caption: Option<&str>,
) {
    let caption_text = caption.unwrap_or("");

    let db_message = DbMessage {
        message_id: message.id.0 as i64,
        user_id: message.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0),
        chat_id: message.chat.id.0,
        text: Some(caption_text.to_string()),
        message_type: media_type.to_string(),
    };

    if let Err(e) = db.insert_message(&db_message).await {
        error!("Failed to save media message: {}", e);
        return;
    }

    let media = Media {
        message_id: message.id.0 as i64,
        media_type: media_type.to_string(),
        file_id: file_id.to_string(),
        file_unique_id: file_unique_id.to_string(),
        file_size: Some(file_size),
        mime_type: mime_type.map(|s| s.to_string()),
    };
    if let Err(e) = db.insert_media(&media).await {
        error!("Failed to save media: {}", e);
    }

    if !caption_text.is_empty() {
        process_spelling(bot, message, db, caption_text).await;
    }
}

async fn process_spelling(bot: &Bot, message: &TgMessage, db: &DbPool, text: &str) {
    match check_spelling(text, 3).await {
        Ok(Some(errors)) => {
            let (corrected_text, processed_errors) = format_correction_message(text, &errors);
            let errors_json = serde_json::to_string(&processed_errors).unwrap_or_default();

            let correction = SpellingCorrection {
                message_id: message.id.0 as i64,
                original_text: text.to_string(),
                corrected_text: corrected_text.clone(),
                errors: errors_json,
                sent_to_chat: true,
            };
            if let Err(e) = db.insert_spelling_correction(&correction).await {
                error!("Failed to save spelling correction: {}", e);
            }

            if let Some(correction_msg) = format_chat_message(text, &corrected_text, &errors) {
                let _ = bot
                    .send_message(message.chat.id, correction_msg)
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await;
            }
        }
        Err(e) => error!("Spelling check failed: {}", e),
        _ => {}
    }
}

async fn handle_service_event(db: &DbPool, message: &TgMessage, event_type: &str) {
    let user_id = message.from.as_ref().map(|u| u.id.0 as i64);

    let data = json!({
        "message_id": message.id.0,
        "event": event_type
    });

    if let Err(e) = db.insert_service_event(message.chat.id.0, event_type, user_id, data).await {
        error!("Failed to save service event: {}", e);
    }

    debug!("Service event recorded: {} at chat {}", event_type, message.chat.id.0);
}
