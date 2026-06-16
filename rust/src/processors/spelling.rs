use reqwest::Client;
use serde_json::{json, Value};
use log::{warn, error};
use crate::error::{Error, Result};

const YANDEX_SPELLER_API: &str = "https://speller.yandex.net/services/spellchecker.json/checkTexts";

#[derive(Debug, Clone)]
pub struct SpellingError {
    pub original: String,
    pub suggested: String,
    pub position: i32,
    pub all_suggestions: Vec<String>,
}

pub async fn check_spelling(text: &str, max_retries: u32) -> Result<Option<Vec<SpellingError>>> {
    if text.trim().len() < 2 {
        return Ok(None);
    }

    // Filter out text with only special characters
    let cleaned: String = text.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
    if cleaned.trim().len() < 2 {
        return Ok(None);
    }

    let client = Client::new();

    for attempt in 0..max_retries {
        match client
            .post(YANDEX_SPELLER_API)
            .query(&[("text", text)])
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(response) => match response.json::<Vec<Vec<Value>>>().await {
                Ok(results) => {
                    if results.is_empty() || results[0].is_empty() {
                        return Ok(None);
                    }

                    let mut errors = Vec::new();
                    for error in &results[0] {
                        if let (Some(word), Some(pos), Some(suggestions)) = (
                            error.get("word").and_then(|v| v.as_str()),
                            error.get("pos").and_then(|v| v.as_i64()),
                            error.get("s").and_then(|v| v.as_array()),
                        ) {
                            let suggestions_vec: Vec<String> = suggestions
                                .iter()
                                .filter_map(|s| s.as_str().map(|s| s.to_string()))
                                .collect();

                            if let Some(first_suggestion) = suggestions_vec.first() {
                                errors.push(SpellingError {
                                    original: word.to_string(),
                                    suggested: first_suggestion.clone(),
                                    position: pos as i32,
                                    all_suggestions: suggestions_vec,
                                });
                            }
                        }
                    }

                    return Ok(if errors.is_empty() { None } else { Some(errors) });
                }
                Err(e) => {
                    warn!("Failed to parse YandexSpeller response (attempt {}/{}): {}", attempt + 1, max_retries, e);
                }
            },
            Err(e) => {
                warn!("YandexSpeller request error (attempt {}/{}): {}", attempt + 1, max_retries, e);
            }
        }

        if attempt < max_retries - 1 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    error!("Failed to check spelling after {} attempts", max_retries);
    Ok(None)
}

pub fn format_correction_message(text: &str, errors: &[SpellingError]) -> (String, Vec<Value>) {
    let mut corrected_text = text.to_string();
    let mut processed_errors = Vec::new();

    for error in errors {
        corrected_text = corrected_text.replacen(&error.original, &error.suggested, 1);
        processed_errors.push(json!({
            "original": error.original,
            "suggested": error.suggested,
            "position": error.position,
            "all_suggestions": error.all_suggestions,
        }));
    }

    (corrected_text, processed_errors)
}

pub fn format_chat_message(text: &str, corrected_text: &str, errors: &[SpellingError]) -> Option<String> {
    if errors.is_empty() {
        return None;
    }

    let mut lines = vec![
        "<b>Исправление орфографии:</b>".to_string(),
        format!("<i>Оригинал:</i> <code>{}</code>", text),
        format!("<i>Исправлено:</i> <code>{}</code>", corrected_text),
        "\n<b>Ошибки:</b>".to_string(),
    ];

    for error in errors {
        let suggestions = error
            .all_suggestions
            .iter()
            .take(3)
            .collect::<Vec<_>>()
            .join(" / ");
        lines.push(format!("• <code>{}</code> → {}", error.original, suggestions));
    }

    Some(lines.join("\n"))
}
