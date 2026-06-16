use dotenv::dotenv;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub telegram_bot_token: String,
    pub mysql_host: String,
    pub mysql_port: u16,
    pub mysql_user: String,
    pub mysql_password: String,
    pub mysql_database: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        dotenv().ok();

        let telegram_bot_token = env::var("TELEGRAM_BOT_TOKEN")
            .map_err(|_| "TELEGRAM_BOT_TOKEN is required".to_string())?;

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
        })
    }
}
