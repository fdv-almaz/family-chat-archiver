mod config;
mod db;
mod error;
mod media_storage;
mod processors;

use teloxide::prelude::*;
use teloxide::types::{Message as TgMessage, PhotoSize, Chat};
use log::{error, info, debug};
use serde_json::json;
use std::sync::Arc;

use config::{Config, SpellingVisibility};
use db::{DbPool, User, DbMessage, Media, SpellingCorrection};
use processors::{check_spelling, format_correction_message, format_chat_message, extract_urls};

const START_MESSAGE: &str = concat!(
    "<b>👋 Family Chat Archiver v", env!("CARGO_PKG_VERSION"), "</b>\n\n",
    "Это бот для архивирования всех сообщений в семейной группе.\n\n",
    "<b>Основные возможности:</b>\n",
    "✅ Сохранение всех сообщений (текст, фото, видео, документы, аудио)\n",
    "✅ Сохранение информации об авторах\n",
    "✅ Проверка орфографии русскоязычных текстов\n",
    "✅ Сохранение ссылок и медиа-контента\n",
    "✅ Запись служебных событий (вход/выход участников)\n\n",
    "<b>Как это работает:</b>\n",
    "Бот автоматически архивирует все сообщения в группе без участия пользователя. Для исправления орфографии используется YandexSpeller API.\n\n",
    "<b>Хранение данных:</b>\n",
    "Все данные сохраняются в защищённой MySQL базе данных.\n\n",
    "<i>Бот работает в фоновом режиме и не требует команд.</i>"
);

fn chat_title(chat: &Chat) -> Option<String> {
    if let Some(t) = chat.title() {
        return Some(t.to_string());
    }
    let first = chat.first_name();
    let last = chat.last_name();
    match (first, last) {
        (Some(f), Some(l)) => Some(format!("{} {}", f, l)),
        (Some(f), None) => Some(f.to_string()),
        (None, Some(l)) => Some(l.to_string()),
        (None, None) => None,
    }
}

fn chat_type(chat: &Chat) -> Option<String> {
    use teloxide::types::ChatKind;
    Some(match &chat.kind {
        ChatKind::Public(_) => "public".to_string(),
        ChatKind::Private(_) => "private".to_string(),
    })
}

fn build_db_message(message: &TgMessage, text: &str, message_type: &str) -> DbMessage {
    let user = message.from();
    DbMessage {
        message_id: message.id.0 as i64,
        user_id: user.map(|u| u.id.0 as i64).unwrap_or(0),
        user_username: user.and_then(|u| u.username.clone()),
        user_first_name: user.map(|u| u.first_name.clone()),
        user_last_name: user.and_then(|u| u.last_name.clone()),
        chat_id: message.chat.id.0,
        chat_title: chat_title(&message.chat),
        chat_type: chat_type(&message.chat),
        text: Some(text.to_string()),
        message_type: message_type.to_string(),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env().map_err(|e| format!("Config error: {}", e))?;

    // Setup logging: rotating daily file + optional console
    std::fs::create_dir_all(&config.log_dir).ok();
    let file_appender = tracing_appender::rolling::daily(&config.log_dir, &config.log_file_prefix);
    let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let registry = tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_writer(file_writer).with_ansi(false));

    if config.log_to_console {
        registry.with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout)).init();
    } else {
        registry.init();
    }

    // Bridge log crate (used by mysql, teloxide deps) into tracing
    tracing_log::LogTracer::init().ok();

    info!("Family Chat Archiver v{}", env!("CARGO_PKG_VERSION"));
    info!("Logging to {}/{} (console: {})", config.log_dir, config.log_file_prefix, config.log_to_console);

    let db_pool = DbPool::new(&config).map_err(|e| format!("DB error: {}", e))?;

    db_pool.create_tables().await.map_err(|e| format!("DB init error: {}", e))?;
    std::fs::create_dir_all(&config.media_storage_dir).ok();

    let db_pool = Arc::new(db_pool);
    let cfg = Arc::new(config.clone());
    let bot = Bot::new(&config.telegram_bot_token);

    info!("Bot started, listening for messages... (spelling: {:?})", config.spelling_visibility);

    let handler = Update::filter_message()
        .endpoint({
            let db = Arc::clone(&db_pool);
            let cfg = Arc::clone(&cfg);
            move |bot: Bot, msg: TgMessage| {
                let db = Arc::clone(&db);
                let cfg = Arc::clone(&cfg);
                async move { handle_message(bot, msg, db, cfg).await }
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
    cfg: Arc<Config>,
) -> ResponseResult<()> {
    if let Some(user) = message.from() {
        if user.is_bot {
            return Ok(());
        }

        let db_user = User {
            user_id: user.id.0 as i64,
            username: user.username.clone(),
            first_name: Some(user.first_name.clone()),
            last_name: user.last_name.clone(),
            is_bot: user.is_bot,
        };
        if let Err(e) = db.insert_or_update_user(&db_user).await {
            error!("Failed to save user: {}", e);
        }
    }

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
        handle_text_message(&bot, &message, db.as_ref(), &cfg, text).await;
    } else if let Some(photos) = message.photo() {
        handle_photo_message(&bot, &message, db.as_ref(), &cfg, photos, message.caption()).await;
    } else if let Some(video) = message.video() {
        save_media(&bot, &message, db.as_ref(), &cfg, "video",
            &video.file.id, &video.file.unique_id, video.file.size as i64,
            None, video.mime_type.as_ref().map(|m| m.to_string()),
            Some(video.duration as i32),
            message.caption()).await;
    } else if let Some(audio) = message.audio() {
        save_media(&bot, &message, db.as_ref(), &cfg, "audio",
            &audio.file.id, &audio.file.unique_id, audio.file.size as i64,
            audio.file_name.clone(),
            audio.mime_type.as_ref().map(|m| m.to_string()),
            Some(audio.duration as i32),
            message.caption()).await;
    } else if let Some(voice) = message.voice() {
        save_media(&bot, &message, db.as_ref(), &cfg, "voice",
            &voice.file.id, &voice.file.unique_id, voice.file.size as i64,
            None, voice.mime_type.as_ref().map(|m| m.to_string()),
            Some(voice.duration as i32),
            message.caption()).await;
    } else if let Some(video_note) = message.video_note() {
        save_media(&bot, &message, db.as_ref(), &cfg, "video_note",
            &video_note.file.id, &video_note.file.unique_id, video_note.file.size as i64,
            None, None, Some(video_note.duration as i32),
            None).await;
    } else if let Some(animation) = message.animation() {
        save_media(&bot, &message, db.as_ref(), &cfg, "animation",
            &animation.file.id, &animation.file.unique_id, animation.file.size as i64,
            animation.file_name.clone(),
            animation.mime_type.as_ref().map(|m| m.to_string()),
            Some(animation.duration as i32),
            message.caption()).await;
    } else if let Some(document) = message.document() {
        save_media(&bot, &message, db.as_ref(), &cfg, "document",
            &document.file.id, &document.file.unique_id, document.file.size as i64,
            document.file_name.clone(),
            document.mime_type.as_ref().map(|m| m.to_string()),
            None,
            message.caption()).await;
    } else if let Some(sticker) = message.sticker() {
        save_media(&bot, &message, db.as_ref(), &cfg, "sticker",
            &sticker.file.id, &sticker.file.unique_id, sticker.file.size as i64,
            None, None, None, None).await;
    } else if let Some(contact) = message.contact() {
        let text = format!("Contact: {} {} {}",
            contact.first_name,
            contact.last_name.as_deref().unwrap_or(""),
            contact.phone_number);
        save_simple_message(db.as_ref(), &message, "contact", &text).await;
    } else if let Some(location) = message.location() {
        let text = format!("Location: lat={}, lon={}", location.latitude, location.longitude);
        save_simple_message(db.as_ref(), &message, "location", &text).await;
    } else if let Some(venue) = message.venue() {
        let text = format!("Venue: {} ({})", venue.title, venue.address);
        save_simple_message(db.as_ref(), &message, "venue", &text).await;
    } else if let Some(poll) = message.poll() {
        let opts: Vec<String> = poll.options.iter().map(|o| o.text.clone()).collect();
        let text = format!("Poll: {} [{}]", poll.question, opts.join("; "));
        save_simple_message(db.as_ref(), &message, "poll", &text).await;
    } else if let Some(dice) = message.dice() {
        let text = format!("Dice: {:?} = {}", dice.emoji, dice.value);
        save_simple_message(db.as_ref(), &message, "dice", &text).await;
    } else if !message.new_chat_members().unwrap_or(&[]).is_empty() {
        handle_service_event(db.as_ref(), &message, "user_joined").await;
    } else if message.left_chat_member().is_some() {
        handle_service_event(db.as_ref(), &message, "user_left").await;
    } else if message.new_chat_title().is_some() {
        handle_service_event(db.as_ref(), &message, "title_changed").await;
    } else if message.new_chat_photo().is_some() {
        handle_service_event(db.as_ref(), &message, "photo_changed").await;
    } else if message.delete_chat_photo().is_some() {
        handle_service_event(db.as_ref(), &message, "photo_deleted").await;
    } else if message.group_chat_created().is_some() {
        handle_service_event(db.as_ref(), &message, "group_created").await;
    } else if message.super_group_chat_created().is_some() {
        handle_service_event(db.as_ref(), &message, "supergroup_created").await;
    } else if message.channel_chat_created().is_some() {
        handle_service_event(db.as_ref(), &message, "channel_created").await;
    } else if message.pinned_message().is_some() {
        handle_service_event(db.as_ref(), &message, "message_pinned").await;
    }

    Ok(())
}

async fn save_media_and_download(bot: &Bot, db: &DbPool, cfg: &Config, media: Media) {
    let file_id = media.file_id.clone();
    let file_unique_id = media.file_unique_id.clone();
    let file_size = media.file_size;

    let media_id = match db.insert_media(&media).await {
        Ok(id) => id,
        Err(e) => {
            error!("Failed to save media: {}", e);
            return;
        }
    };

    if let Some(path) = media_storage::download_and_save(
        bot, &cfg.media_storage_dir,
        &file_id, &file_unique_id,
        file_size, cfg.media_max_download_size,
    ).await {
        if let Err(e) = db.update_media_local_path(media_id, &path).await {
            error!("Failed to update local_path: {}", e);
        }
    }
}

async fn save_simple_message(db: &DbPool, message: &TgMessage, mtype: &str, text: &str) {
    let db_message = build_db_message(message, text, mtype);
    if let Err(e) = db.insert_message(&db_message).await {
        error!("Failed to save {} message: {}", mtype, e);
    }
}

async fn handle_text_message(
    bot: &Bot,
    message: &TgMessage,
    db: &DbPool,
    cfg: &Config,
    text: &str,
) {
    let db_message = build_db_message(message, text, "text");

    if let Err(e) = db.insert_message(&db_message).await {
        error!("Failed to save message: {}", e);
        return;
    }

    for url in extract_urls(text) {
        if let Err(e) = db.insert_link(message.id.0 as i64, &url).await {
            error!("Failed to save link: {}", e);
        }
    }

    process_spelling(bot, message, db, cfg, text).await;
}

async fn handle_photo_message(
    bot: &Bot,
    message: &TgMessage,
    db: &DbPool,
    cfg: &Config,
    photos: &[PhotoSize],
    caption: Option<&str>,
) {
    let caption_text = caption.unwrap_or("");
    let db_message = build_db_message(message, caption_text, "photo");

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
            file_name: None,
            file_size: Some(photo.file.size as i64),
            duration: None,
            mime_type: None,
            local_path: None,
        };
        save_media_and_download(bot, db, cfg, media).await;
    }

    if !caption_text.is_empty() {
        process_spelling(bot, message, db, cfg, caption_text).await;
    }
}

async fn save_media(
    bot: &Bot,
    message: &TgMessage,
    db: &DbPool,
    cfg: &Config,
    media_type: &str,
    file_id: &str,
    file_unique_id: &str,
    file_size: i64,
    file_name: Option<String>,
    mime_type: Option<String>,
    duration: Option<i32>,
    caption: Option<&str>,
) {
    let caption_text = caption.unwrap_or("");
    let db_message = build_db_message(message, caption_text, media_type);

    if let Err(e) = db.insert_message(&db_message).await {
        error!("Failed to save {} message: {}", media_type, e);
        return;
    }

    let media = Media {
        message_id: message.id.0 as i64,
        media_type: media_type.to_string(),
        file_id: file_id.to_string(),
        file_unique_id: file_unique_id.to_string(),
        file_name,
        file_size: Some(file_size),
        duration,
        mime_type,
        local_path: None,
    };
    save_media_and_download(bot, db, cfg, media).await;

    if !caption_text.is_empty() {
        process_spelling(bot, message, db, cfg, caption_text).await;
    }
}

async fn process_spelling(bot: &Bot, message: &TgMessage, db: &DbPool, cfg: &Config, text: &str) {
    match check_spelling(text, 3).await {
        Ok(Some(errors)) => {
            let (corrected_text, processed_errors) = format_correction_message(text, &errors);
            let errors_json = serde_json::to_string(&processed_errors).unwrap_or_default();

            let correction = SpellingCorrection {
                message_id: message.id.0 as i64,
                original_text: text.to_string(),
                corrected_text: corrected_text.clone(),
                errors: errors_json,
                sent_to_chat: cfg.spelling_visibility != SpellingVisibility::Off,
            };
            if let Err(e) = db.insert_spelling_correction(&correction).await {
                error!("Failed to save spelling correction: {}", e);
            }

            if cfg.spelling_visibility == SpellingVisibility::Off {
                return;
            }

            let author_name = message.from().map(|u| u.first_name.as_str());

            if let Some(correction_msg) = format_chat_message(text, &corrected_text, &errors, author_name) {
                match cfg.spelling_visibility {
                    SpellingVisibility::Private => {
                        // DM to the author
                        if let Some(user) = message.from() {
                            let _ = bot
                                .send_message(ChatId(user.id.0 as i64), correction_msg)
                                .parse_mode(teloxide::types::ParseMode::Html)
                                .await;
                        }
                    }
                    SpellingVisibility::Public => {
                        let _ = bot
                            .send_message(message.chat.id, correction_msg)
                            .parse_mode(teloxide::types::ParseMode::Html)
                            .reply_to_message_id(message.id)
                            .await;
                    }
                    SpellingVisibility::Off => {}
                }
            }
        }
        Err(e) => error!("Spelling check failed: {}", e),
        _ => {}
    }
}

async fn handle_service_event(db: &DbPool, message: &TgMessage, event_type: &str) {
    let user = message.from();
    let user_id = user.map(|u| u.id.0 as i64);
    let username = user.and_then(|u| u.username.as_deref());
    let first_name = user.map(|u| u.first_name.as_str());

    let data = json!({
        "message_id": message.id.0,
        "event": event_type
    });

    let title = chat_title(&message.chat);

    if let Err(e) = db.insert_service_event(
        message.chat.id.0,
        title.as_deref(),
        event_type,
        user_id,
        username,
        first_name,
        data,
    ).await {
        error!("Failed to save service event: {}", e);
    }

    debug!("Service event recorded: {} at chat {}", event_type, message.chat.id.0);
}
