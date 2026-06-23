use dotenv::dotenv;
use std::env;

#[derive(Debug, Clone, PartialEq)]
pub enum SpellingVisibility {
    Public,
    Private,
    Off,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub telegram_bot_token: String,
    pub mysql_host: String,
    pub mysql_port: u16,
    pub mysql_user: String,
    pub mysql_password: String,
    pub mysql_database: String,
    pub spelling_visibility: SpellingVisibility,
    pub log_dir: String,
    pub log_file_prefix: String,
    pub log_to_console: bool,
    pub media_storage_dir: String,
    pub media_max_download_size: u64,
    // --- Совет дня (фоновый планировщик внутри бота) ---
    pub anthropic_api_key: Option<String>,
    pub anthropic_model: String,
    pub tip_chat_id: Option<i64>,
    pub tip_hour: u32,
    pub tip_minute: u32,
    pub tip_system_prompt: String,
    pub tip_history_limit: u32,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        dotenv().ok();

        let telegram_bot_token = env::var("TELEGRAM_BOT_TOKEN")
            .map_err(|_| "TELEGRAM_BOT_TOKEN is required".to_string())?;

        let visibility = match env::var("SPELLING_VISIBILITY")
            .unwrap_or_else(|_| "public".to_string())
            .to_lowercase()
            .as_str()
        {
            "private" => SpellingVisibility::Private,
            "off" => SpellingVisibility::Off,
            _ => SpellingVisibility::Public,
        };

        let log_to_console = env::var("LOG_TO_CONSOLE")
            .unwrap_or_else(|_| "true".to_string())
            .to_lowercase();
        let log_to_console = matches!(log_to_console.as_str(), "1" | "true" | "yes");

        Ok(Config {
            telegram_bot_token,
            mysql_host: env::var("MYSQL_HOST").unwrap_or_else(|_| "localhost".to_string()),
            mysql_port: env::var("MYSQL_PORT")
                .unwrap_or_else(|_| "3306".to_string())
                .parse()
                .unwrap_or(3306),
            mysql_user: env::var("MYSQL_USER").unwrap_or_else(|_| "root".to_string()),
            mysql_password: env::var("MYSQL_PASSWORD").unwrap_or_default(),
            mysql_database: env::var("MYSQL_DATABASE")
                .unwrap_or_else(|_| "family_chat".to_string()),
            spelling_visibility: visibility,
            log_dir: env::var("LOG_DIR").unwrap_or_else(|_| "logs".to_string()),
            log_file_prefix: env::var("LOG_FILE_PREFIX").unwrap_or_else(|_| "bot.log".to_string()),
            log_to_console,
            media_storage_dir: Self::resolve_storage_dir(),
            media_max_download_size: env::var("MEDIA_MAX_DOWNLOAD_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(20 * 1024 * 1024),
            anthropic_api_key: env::var("ANTHROPIC_API_KEY").ok().filter(|s| !s.is_empty()),
            anthropic_model: env::var("ANTHROPIC_MODEL")
                .unwrap_or_else(|_| "claude-opus-4-8".to_string()),
            tip_chat_id: env::var("TIP_CHAT_ID").ok().and_then(|s| s.trim().parse().ok()),
            tip_hour: env::var("TIP_HOUR").ok().and_then(|s| s.parse().ok()).unwrap_or(6),
            tip_minute: env::var("TIP_MINUTE").ok().and_then(|s| s.parse().ok()).unwrap_or(0),
            tip_system_prompt: Self::load_system_prompt(),
            tip_history_limit: env::var("TIP_HISTORY_LIMIT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30),
        })
    }

    /// Системный промпт совета дня вынесен в конфигурационный файл (а не в код),
    /// чтобы его можно было править без пересборки. Путь задаётся
    /// `TIP_SYSTEM_PROMPT_FILE`; относительный путь привязывается к корню проекта
    /// (на уровень выше крейта `rust/`), по умолчанию — общий для Python и Rust
    /// `daily_tip_prompt.txt`. Если файл недоступен — встроенный fallback,
    /// чтобы фича не падала (с предупреждением в лог).
    fn load_system_prompt() -> String {
        const FALLBACK: &str = concat!(
            "Ты — добрый помощник в семейном чате, где есть и взрослые, и дети. ",
            "Пришли один короткий безопасный для всех возрастов «совет дня» по-русски, ",
            "без Markdown и эмодзи, без вступлений."
        );

        let raw = env::var("TIP_SYSTEM_PROMPT_FILE")
            .unwrap_or_else(|_| "../daily_tip_prompt.txt".to_string());
        let path = if std::path::Path::new(&raw).is_absolute() {
            std::path::PathBuf::from(&raw)
        } else {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(&raw)
        };

        match std::fs::read_to_string(&path) {
            Ok(s) if !s.trim().is_empty() => s.trim().to_string(),
            Ok(_) => {
                eprintln!(
                    "ВНИМАНИЕ: файл системного промпта {} пуст — использую встроенный fallback",
                    path.display()
                );
                FALLBACK.to_string()
            }
            Err(e) => {
                eprintln!(
                    "ВНИМАНИЕ: не удалось прочитать файл системного промпта {} ({}) — использую встроенный fallback",
                    path.display(),
                    e
                );
                FALLBACK.to_string()
            }
        }
    }

    /// Resolve the media storage directory. A relative `MEDIA_STORAGE_DIR`
    /// (the default `storage`) is anchored to the crate root captured at build
    /// time (`CARGO_MANIFEST_DIR` = `rust/`), NOT the current working directory.
    /// This keeps files in `rust/storage` no matter where the binary is launched
    /// from (e.g. `target/release`), so `local_path` stays inside the web whitelist.
    fn resolve_storage_dir() -> String {
        let dir = env::var("MEDIA_STORAGE_DIR").unwrap_or_else(|_| "storage".to_string());
        if std::path::Path::new(&dir).is_absolute() {
            dir
        } else {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(dir)
                .to_string_lossy()
                .into_owned()
        }
    }
}
