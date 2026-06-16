use reqwest::Client;
use serde_json::{json, Value};
use log::debug;
use crate::error::Result;

const YANDEX_SPELLER_API: &str = "https://speller.yandex.net/services/spellservice.json/checkText";
const SPELLER_OPTIONS: u32 = 1 + 2 + 4; // ignore digits, URLs, find repeat words
const SPELLER_LANG: &str = "ru,en";

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

    // Skip commands (text starting with /)
    if text.trim().starts_with('/') {
        return Ok(None);
    }

    // Filter out text with only special characters
    let cleaned: String = text.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
    if cleaned.trim().len() < 2 {
        return Ok(None);
    }

    let client = Client::new();
    let options_str = SPELLER_OPTIONS.to_string();
    let form_data = [
        ("text", text),
        ("options", options_str.as_str()),
        ("lang", SPELLER_LANG),
    ];

    for attempt in 0..max_retries {
        match client
            .post(YANDEX_SPELLER_API)
            .form(&form_data)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            Ok(response) => {
                if response.status() == 400 {
                    debug!("YandexSpeller bad request for text: {}... (skipping)", &text[..std::cmp::min(50, text.len())]);
                    return Ok(None);
                }

                match response.json::<Vec<Value>>().await {
                    Ok(results) => {
                        if results.is_empty() {
                            return Ok(None);
                        }

                        let mut errors = Vec::new();
                        for error in &results {
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
                        debug!("Failed to parse YandexSpeller response (attempt {}/{}): {}", attempt + 1, max_retries, e);
                    }
                }
            }
            Err(e) => {
                debug!("YandexSpeller request error (attempt {}/{}): {}", attempt + 1, max_retries, e);
            }
        }

        if attempt < max_retries - 1 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    debug!("Failed to check spelling after {} attempts for text: {}...", max_retries, &text[..std::cmp::min(50, text.len())]);
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

pub fn format_chat_message(
    _text: &str,
    _corrected_text: &str,
    errors: &[SpellingError],
    author_name: Option<&str>,
) -> Option<String> {
    if errors.is_empty() {
        return None;
    }

    let parts: Vec<String> = errors
        .iter()
        .map(|e| {
            let suggestions = e.all_suggestions.iter().take(2).cloned().collect::<Vec<_>>().join(" / ");
            format!("<b>{}</b> → <i>{}</i>", e.original, suggestions)
        })
        .collect();

    let prefix = match author_name {
        Some(name) => format!("✏️ {}, ", name),
        None => "✏️ ".to_string(),
    };

    Some(format!("{}{}", prefix, parts.join("; ")))
}
