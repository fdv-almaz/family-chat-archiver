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
            media_storage_dir: env::var("MEDIA_STORAGE_DIR").unwrap_or_else(|_| "storage".to_string()),
            media_max_download_size: env::var("MEDIA_MAX_DOWNLOAD_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(20 * 1024 * 1024),
        })
    }
}
