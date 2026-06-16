mod config;
mod db;
mod error;
mod processors;

use teloxide::prelude::*;
use log::{error, info, debug};
use serde_json::json;
use std::sync::Arc;

use config::Config;
use db::{DbPool, User, Message, Media, SpellingCorrection};
use error::Result;
use processors::{check_spelling, format_correction_message, format_chat_message, extract_urls};

type MyDialogue = Dialogue<State, InMemStorage<State>>;

#[derive(Clone, Default)]
pub enum State {
    #[default]
    Start,
}

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
async fn main() -> Result<()> {
    env_logger::init();

    let config = Config::from_env()?;
    let db_pool = DbPool::new(&config)?;

    db_pool.create_tables().await?;

    let db_pool = Arc::new(db_pool);
    let bot = Bot::new(&config.telegram_bot_token);

    info!("Bot started, listening for messages...");

    let handler = Update::filter_message()
        .endpoint({
            let db = Arc::clone(&db_pool);
            move |bot: Bot, msg: Message| {
                let db = Arc::clone(&db);
                handle_message(bot, msg, db)
            }
        });

    let mut retry_count = 0;
    const MAX_RETRIES: u32 = 5;
    let mut retry_delay = std::time::Duration::from_secs(5);

    loop {
        match Dispatcher::builder(bot.clone(), handler.clone())
            .error_handler(LoggingErrorHandler::with_custom_text("An error from the dispatcher"))
            .build()
            .dispatch()
            .await
        {
            Ok(()) => {
                info!("Dispatcher completed successfully");
                break;
            }
            Err(e) => {
                retry_count += 1;
                if retry_count <= MAX_RETRIES {
                    error!("Dispatcher error (attempt {}/{}): {}", retry_count, MAX_RETRIES, e);
                    info!("Reconnecting in {:?}...", retry_delay);
                    tokio::time::sleep(retry_delay).await;
                    retry_delay = std::cmp::min(retry_delay.mul_f32(2.0), std::time::Duration::from_secs(60));
                } else {
                    error!("Max retries ({}) exceeded. Stopping bot.", MAX_RETRIES);
                    return Err(error::Error::Api(format!("Dispatcher failed after {} retries: {}", MAX_RETRIES, e)));
                }
            }
        }
    }

    Ok(())
}

async fn handle_message(
    bot: Bot,
    message: Message,
    db: Arc<DbPool>,
) -> ResponseResult<()> {
    // Save user info if available
    if let Some(user) = &message.from {
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

    // Handle commands
    if let Some(text) = message.text() {
        if text == "/start" || text == "/help" {
            let _ = bot
                .send_message(message.chat.id, START_MESSAGE)
                .reply_to_message_id(message.id)
                .await;
            debug!("Start command handled for user {:?}", message.from.as_ref().map(|u| u.id));
            return Ok(());
        }
    }

    if let Some(text) = message.text() {
        handle_text_message(&bot, &message, db.as_ref(), text).await?;
    } else if message.photo().is_some() {
        if let Some(photos) = message.photo() {
            handle_photo_message(&bot, &message, db.as_ref(), photos, message.caption()).await?;
        }
    } else if let Some(video) = message.video() {
        handle_media_message(
            &bot, &message, db.as_ref(), "video",
            &video.file.id, &video.file.unique_id, video.file.size,
            None, message.caption()
        ).await?;
    } else if let Some(document) = message.document() {
        handle_media_message(
            &bot, &message, db.as_ref(), "document",
            &document.file.id, &document.file.unique_id, document.file.size,
            document.mime_type.as_deref(), message.caption()
        ).await?;
    } else if let Some(voice) = message.voice() {
        handle_media_message(
            &bot, &message, db.as_ref(), "voice",
            &voice.file.id, &voice.file.unique_id, voice.file.size,
            voice.mime_type.as_deref(), message.caption()
        ).await?;
    } else if !message.new_chat_members().is_empty() {
        handle_service_event(&db, &message, "user_joined").await?;
    } else if message.left_chat_member().is_some() {
        handle_service_event(&db, &message, "user_left").await?;
    } else if message.new_chat_title().is_some() {
        handle_service_event(&db, &message, "title_changed").await?;
    } else if message.new_chat_photo().is_some() {
        handle_service_event(&db, &message, "photo_changed").await?;
    } else if message.delete_chat_photo() {
        handle_service_event(&db, &message, "photo_deleted").await?;
    }

    Ok(())
}

async fn handle_text_message(
    bot: &Bot,
    message: &Message,
    db: &DbPool,
    text: &str,
) -> ResponseResult<()> {
    let db_message = Message {
        message_id: message.id.0 as i64,
        user_id: message.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0),
        chat_id: message.chat.id.0,
        text: Some(text.to_string()),
        message_type: "text".to_string(),
    };

    if let Err(e) = db.insert_message(&db_message).await {
        error!("Failed to save message: {}", e);
    }

    // Extract and save links
    for url in extract_urls(text) {
        if let Err(e) = db.insert_link(message.id.0 as i64, &url).await {
            error!("Failed to save link: {}", e);
        }
    }

    // Check spelling
    match check_spelling(text, 3).await {
        Ok(Some(errors)) => {
            let (corrected_text, processed_errors) = format_correction_message(text, &errors);

            // Save correction to DB
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

            // Send correction to chat (visible to all)
            if let Some(correction_msg) = format_chat_message(text, &corrected_text, &errors) {
                let _ = bot.send_message(message.chat.id, correction_msg).await;
            }
        }
        Err(e) => error!("Spelling check failed: {}", e),
        _ => {}
    }

    debug!("Text message processed: user={}, message_id={}",
        message.from.as_ref().map(|u| u.id).unwrap_or_default().0,
        message.id.0);

    Ok(())
}

async fn handle_photo_message(
    bot: &Bot,
    message: &Message,
    db: &DbPool,
    photos: &[teloxide::types::PhotoSize],
    caption: Option<&str>,
) -> ResponseResult<()> {
    let caption_text = caption.unwrap_or("");

    let db_message = Message {
        message_id: message.id.0 as i64,
        user_id: message.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0),
        chat_id: message.chat.id.0,
        text: Some(caption_text.to_string()),
        message_type: "photo".to_string(),
    };

    if let Err(e) = db.insert_message(&db_message).await {
        error!("Failed to save photo message: {}", e);
    }

    // Save largest photo
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

    // Check spelling in caption
    if !caption_text.is_empty() {
        if let Ok(Some(errors)) = check_spelling(caption_text, 3).await {
            let (corrected_text, processed_errors) = format_correction_message(caption_text, &errors);
            let errors_json = serde_json::to_string(&processed_errors).unwrap_or_default();

            let correction = SpellingCorrection {
                message_id: message.id.0 as i64,
                original_text: caption_text.to_string(),
                corrected_text: corrected_text.clone(),
                errors: errors_json,
                sent_to_chat: true,
            };
            if let Err(e) = db.insert_spelling_correction(&correction).await {
                error!("Failed to save spelling correction: {}", e);
            }

            if let Some(correction_msg) = format_chat_message(caption_text, &corrected_text, &errors) {
                let _ = bot
                    .send_message(message.chat.id, correction_msg)
                    .reply_to_message_id(message.id)
                    .await;
            }
        }
    }

    debug!("Photo message processed: user={}, message_id={}",
        message.from.as_ref().map(|u| u.id).unwrap_or_default().0,
        message.id.0);

    Ok(())
}

async fn handle_media_message(
    bot: &Bot,
    message: &Message,
    db: &DbPool,
    media_type: &str,
    file_id: &str,
    file_unique_id: &str,
    file_size: u32,
    mime_type: Option<&str>,
    caption: Option<&str>,
) -> ResponseResult<()> {
    let caption_text = caption.unwrap_or("");

    let db_message = Message {
        message_id: message.id.0 as i64,
        user_id: message.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0),
        chat_id: message.chat.id.0,
        text: Some(caption_text.to_string()),
        message_type: media_type.to_string(),
    };

    if let Err(e) = db.insert_message(&db_message).await {
        error!("Failed to save media message: {}", e);
    }

    let media = Media {
        message_id: message.id.0 as i64,
        media_type: media_type.to_string(),
        file_id: file_id.to_string(),
        file_unique_id: file_unique_id.to_string(),
        file_size: Some(file_size as i32),
        mime_type: mime_type.map(|s| s.to_string()),
    };
    if let Err(e) = db.insert_media(&media).await {
        error!("Failed to save media: {}", e);
    }

    // Check spelling in caption
    if !caption_text.is_empty() {
        if let Ok(Some(errors)) = check_spelling(caption_text, 3).await {
            let (corrected_text, processed_errors) = format_correction_message(caption_text, &errors);
            let errors_json = serde_json::to_string(&processed_errors).unwrap_or_default();

            let correction = SpellingCorrection {
                message_id: message.id.0 as i64,
                original_text: caption_text.to_string(),
                corrected_text: corrected_text.clone(),
                errors: errors_json,
                sent_to_chat: true,
            };
            if let Err(e) = db.insert_spelling_correction(&correction).await {
                error!("Failed to save spelling correction: {}", e);
            }

            if let Some(correction_msg) = format_chat_message(caption_text, &corrected_text, &errors) {
                let _ = bot
                    .send_message(message.chat.id, correction_msg)
                    .reply_to_message_id(message.id)
                    .await;
            }
        }
    }

    debug!("{} message processed: user={}, message_id={}",
        media_type,
        message.from.as_ref().map(|u| u.id).unwrap_or_default().0,
        message.id.0);

    Ok(())
}

async fn handle_service_event(
    db: &DbPool,
    message: &Message,
    event_type: &str,
) -> ResponseResult<()> {
    let user_id = message.from.as_ref().map(|u| u.id.0 as i64);

    let data = json!({
        "message_id": message.id.0,
        "event": event_type
    });

    if let Err(e) = db.insert_service_event(message.chat.id.0, event_type, user_id, data).await {
        error!("Failed to save service event: {}", e);
    }

    debug!("Service event recorded: {} at chat {}", event_type, message.chat.id.0);

    Ok(())
}
