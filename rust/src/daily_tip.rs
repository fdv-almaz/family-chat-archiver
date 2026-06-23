//! Совет дня — фоновая задача внутри бота.
//!
//! Ежедневно в `TIP_HOUR:TIP_MINUTE` (по умолчанию 6:00) генерирует через
//! Claude API короткий «совет дня» и отправляет его в семейный чат. Запрос и
//! ответ сохраняются в таблицу `daily_tips`. Планировщик запускается из
//! `main()` как `tokio`-задача и работает в том же процессе, что и бот.

use std::sync::Arc;
use std::time::Duration;

use chrono::{Local, Timelike};
use log::{error, info};
use reqwest::Client;
use serde_json::{json, Value};
use teloxide::prelude::*;
use teloxide::types::ChatId;

use crate::config::Config;
use crate::db::DbPool;
use crate::error::{Error, Result};

const ANTHROPIC_API: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

// Системный промпт вынесен в конфигурационный файл (см. Config::tip_system_prompt
// и daily_tip_prompt.txt в корне проекта) — учитывает, что в чате есть и взрослые,
// и дети: совет должен быть безопасным для любого возраста.

// Маркер перед текстом, чтобы в чате было понятно, что это совет дня.
const TIP_HEADER: &str = "💡 <b>Совет дня</b>\n\n";

/// Запускает фоновый планировщик. Если ключ Anthropic не задан — тихо выходит
/// (фича выключена), бот продолжает работать как обычно.
pub fn spawn_scheduler(bot: Bot, db: Arc<DbPool>, cfg: Arc<Config>) {
    if cfg.anthropic_api_key.is_none() {
        info!("Совет дня отключён: ANTHROPIC_API_KEY не задан");
        return;
    }

    info!("Планировщик совета дня запущен (ежедневно в {:02}:{:02})", cfg.tip_hour, cfg.tip_minute);

    tokio::spawn(async move {
        loop {
            let secs = seconds_until(cfg.tip_hour, cfg.tip_minute);
            info!("Следующий совет дня через {} c", secs);
            tokio::time::sleep(Duration::from_secs(secs)).await;

            if let Err(e) = run_once(&bot, &db, &cfg, None).await {
                error!("Совет дня не отправлен: {}", e);
            }

            // Сдвиг, чтобы следующий расчёт пришёлся уже на завтрашний день.
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

/// Секунд до ближайшего наступления заданного времени (локальное время).
fn seconds_until(hour: u32, minute: u32) -> u64 {
    let now = Local::now();
    let mut target = now
        .with_hour(hour.min(23))
        .and_then(|t| t.with_minute(minute.min(59)))
        .and_then(|t| t.with_second(0))
        .and_then(|t| t.with_nanosecond(0))
        .unwrap_or(now);
    if target <= now {
        target = target + chrono::Duration::days(1);
    }
    (target - now).num_seconds().max(0) as u64
}

/// User-турн с текущей датой и списком уже отправленных советов.
///
/// `previous_tips` — тексты ранее отправленных советов (новые сверху). Они
/// передаются модели с явной инструкцией не повторять их (ни по теме, ни по
/// формулировке), чтобы советы не дублировались.
fn build_user_prompt(previous_tips: &[String]) -> String {
    let mut prompt = format!("Сегодня {}. Пришли совет дня.", Local::now().format("%Y-%m-%d"));
    if !previous_tips.is_empty() {
        let listed = previous_tips
            .iter()
            .enumerate()
            .map(|(i, t)| format!("{}. {}", i + 1, t))
            .collect::<Vec<_>>()
            .join("\n");
        prompt.push_str(&format!(
            "\n\nЭти советы уже были отправлены ранее — НЕ повторяй их \
             (ни по теме, ни по содержанию, ни по формулировке), предложи \
             что-то новое:\n{}",
            listed
        ));
    }
    prompt
}

/// Сгенерировать и отправить один совет; сохранить запрос/ответ в БД.
///
/// `chat_override` — если задан (команда `/check_tip`), совет шлётся именно в
/// этот чат; иначе используется `TIP_CHAT_ID` или самый активный групповой чат.
pub async fn run_once(bot: &Bot, db: &DbPool, cfg: &Config, chat_override: Option<i64>) -> Result<()> {
    let api_key = cfg
        .anthropic_api_key
        .as_deref()
        .ok_or_else(|| Error::Config("ANTHROPIC_API_KEY не задан".to_string()))?;

    // Определяем чат: явное переопределение → TIP_CHAT_ID → самый активный из БД.
    let chat_id = match chat_override {
        Some(id) => id,
        None => match cfg.tip_chat_id {
            Some(id) => id,
            None => match db.get_most_active_chat_id().await? {
                Some(id) => {
                    info!("TIP_CHAT_ID не задан, выбран самый активный чат: {}", id);
                    id
                }
                None => {
                    return Err(Error::Config(
                        "Не удалось определить чат (TIP_CHAT_ID не задан и в БД нет групповых чатов)"
                            .to_string(),
                    ));
                }
            },
        },
    };

    let previous_tips = db.get_recent_tips(chat_id, cfg.tip_history_limit).await?;
    let user_prompt = build_user_prompt(&previous_tips);
    let full_prompt = format!("{}\n\n---\n{}", cfg.tip_system_prompt, user_prompt);

    // 1) Сгенерировать совет
    let tip = match generate_tip(api_key, &cfg.anthropic_model, &cfg.tip_system_prompt, &user_prompt).await {
        Ok(t) => t,
        Err(e) => {
            db.insert_daily_tip(chat_id, &cfg.anthropic_model, &full_prompt,
                                None, false, Some(&e.to_string())).await?;
            return Err(e);
        }
    };

    // 2) Отправить в чат. Заголовок-маркер + экранированный текст (режим HTML).
    let message_text = format!("{}{}", TIP_HEADER, teloxide::utils::html::escape(&tip));
    let (sent, err) = match bot
        .send_message(ChatId(chat_id), message_text)
        .parse_mode(teloxide::types::ParseMode::Html)
        .await
    {
        Ok(_) => {
            info!("Совет дня отправлен в чат {}", chat_id);
            (true, None)
        }
        Err(e) => {
            error!("Не удалось отправить совет дня в чат {}: {}", chat_id, e);
            (false, Some(e.to_string()))
        }
    };

    // 3) Сохранить запрос и ответ в БД
    db.insert_daily_tip(chat_id, &cfg.anthropic_model, &full_prompt,
                        Some(&tip), sent, err.as_deref()).await?;

    if sent {
        Ok(())
    } else {
        Err(Error::Api("Не удалось отправить совет дня".to_string()))
    }
}

/// Запросить совет у Claude через Messages API и вернуть текст.
async fn generate_tip(api_key: &str, model: &str, system_prompt: &str, user_prompt: &str) -> Result<String> {
    let client = Client::new();
    let body = json!({
        "model": model,
        "max_tokens": 1024,
        "system": system_prompt,
        "messages": [{ "role": "user", "content": user_prompt }],
    });

    let response = client
        .post(ANTHROPIC_API)
        .header("x-api-key", api_key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("content-type", "application/json")
        .json(&body)
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| Error::Api(format!("Anthropic request failed: {}", e)))?;

    let status = response.status();
    let payload: Value = response
        .json()
        .await
        .map_err(|e| Error::Api(format!("Failed to parse Anthropic response: {}", e)))?;

    if !status.is_success() {
        return Err(Error::Api(format!("Anthropic API {}: {}", status, payload)));
    }

    // Собираем текст из всех блоков type == "text".
    let text: String = payload
        .get("content")
        .and_then(|c| c.as_array())
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
        .trim()
        .to_string();

    if text.is_empty() {
        return Err(Error::Api("Claude вернул пустой ответ".to_string()));
    }
    Ok(text)
}
