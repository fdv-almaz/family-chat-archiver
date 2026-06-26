//! Разовая генерация «совета дня» через Claude — без Telegram.
//!
//! Самостоятельный бинарник, повторяющий ровно ту же логику, что и фоновый
//! планировщик бота (`daily_tip::run_once`), но не зависящий от teloxide-бота:
//! он не шлёт совет в чат, а просто печатает его в stdout. Данные из семейной БД
//! берутся тем же способом, что и в боте:
//!   * чат определяется как `TIP_CHAT_ID` или самый активный групповой чат
//!     (`DbPool::get_most_active_chat_id`);
//!   * для антиповтора подгружаются последние `TIP_HISTORY_LIMIT` отправленных
//!     советов этого чата (`DbPool::get_recent_tips`) и передаются модели с
//!     инструкцией не повторяться;
//!   * системный и user-промпт строятся теми же функциями из `daily_tip`.
//!
//! Запуск (из каталога rust/, рядом с тем же .env, что и у бота):
//!     cargo run --bin generate_tip                  # сгенерировать и напечатать
//!     cargo run --bin generate_tip -- --chat-id -100…  # история для конкретного чата
//!     cargo run --bin generate_tip -- --no-history     # без учёта прошлых советов
//!     cargo run --bin generate_tip -- --save           # ещё и записать в daily_tips
//!
//! По умолчанию в БД ничего не пишется — это «только чтение»: бинарник лишь
//! запрашивает совет и выводит его. Флаг --save сохраняет пару запрос/ответ в
//! `daily_tips` (как делает бот; sent_to_chat=false, ведь в чат ничего не уходит).

// Переиспользуем исходники бота напрямую (#[path]) — без дублирования логики.
// Многие функции этих модулей в данном бинарнике не задействованы, поэтому
// глушим соответствующие предупреждения на уровне всего крейта.
#![allow(dead_code, unused_imports)]

#[path = "../config.rs"]
mod config;
#[path = "../error.rs"]
mod error;
#[path = "../db/mod.rs"]
mod db;
#[path = "../daily_tip.rs"]
mod daily_tip;

use std::process::ExitCode;

use config::Config;
use daily_tip::{build_user_prompt, generate_tip};
use db::DbPool;

fn print_help() {
    eprintln!(
        "Разовый запрос «совета дня» к Claude (без отправки в Telegram).\n\n\
         Использование: generate_tip [ОПЦИИ]\n\n\
         Опции:\n  \
         --chat-id <ID>   ID чата для подбора истории советов (по умолчанию — как\n                   \
         у бота: TIP_CHAT_ID или самый активный групповой чат из БД)\n  \
         --no-history     не подгружать прошлые советы из БД (отключить антиповтор)\n  \
         --save           сохранить запрос/ответ в таблицу daily_tips (sent_to_chat=false)\n  \
         -h, --help       показать эту справку"
    );
}

#[tokio::main]
async fn main() -> ExitCode {
    // --- Разбор аргументов (без внешних зависимостей) ---
    let mut chat_override: Option<i64> = None;
    let mut no_history = false;
    let mut save = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--chat-id" => match args.next().and_then(|s| s.trim().parse::<i64>().ok()) {
                Some(id) => chat_override = Some(id),
                None => {
                    eprintln!("Ошибка: --chat-id требует числовой аргумент");
                    return ExitCode::from(2);
                }
            },
            "--no-history" => no_history = true,
            "--save" => save = true,
            "-h" | "--help" => {
                print_help();
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("Неизвестный аргумент: {}\n", other);
                print_help();
                return ExitCode::from(2);
            }
        }
    }

    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Ошибка конфигурации: {}", e);
            return ExitCode::FAILURE;
        }
    };

    let api_key = match cfg.anthropic_api_key.as_deref() {
        Some(k) => k,
        None => {
            eprintln!("Ошибка: ANTHROPIC_API_KEY не задан в .env — генерация невозможна.");
            return ExitCode::FAILURE;
        }
    };

    let db = match DbPool::new(&cfg) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Ошибка подключения к БД: {}", e);
            return ExitCode::FAILURE;
        }
    };

    // Чат нужен только для подбора истории советов (антиповтор). Сам совет в
    // никакой чат не отправляется, поэтому при --no-history чат не обязателен.
    let chat_id: Option<i64> = match chat_override {
        Some(id) => Some(id),
        None => match cfg.tip_chat_id {
            Some(id) => Some(id),
            None => match db.get_most_active_chat_id().await {
                Ok(opt) => opt,
                Err(e) => {
                    eprintln!("Ошибка выбора чата из БД: {}", e);
                    return ExitCode::FAILURE;
                }
            },
        },
    };

    let previous_tips: Vec<String> = if no_history {
        Vec::new()
    } else {
        match chat_id {
            None => {
                eprintln!(
                    "Предупреждение: чат не определён (TIP_CHAT_ID не задан и в БД нет \
                     групповых чатов) — генерирую без учёта истории."
                );
                Vec::new()
            }
            Some(id) => match db.get_recent_tips(id, cfg.tip_history_limit).await {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("Ошибка чтения истории советов: {}", e);
                    return ExitCode::FAILURE;
                }
            },
        }
    };

    let user_prompt = build_user_prompt(&previous_tips);

    let tip = match generate_tip(api_key, &cfg.anthropic_model, &cfg.tip_system_prompt, &user_prompt).await {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Ошибка запроса к Claude API: {}", e);
            return ExitCode::FAILURE;
        }
    };

    println!("{}", tip);

    if save {
        match chat_id {
            Some(id) => {
                let full_prompt = format!("{}\n\n---\n{}", cfg.tip_system_prompt, user_prompt);
                if let Err(e) = db
                    .insert_daily_tip(id, &cfg.anthropic_model, &full_prompt, Some(&tip), false, None)
                    .await
                {
                    eprintln!("Ошибка сохранения в daily_tips: {}", e);
                    return ExitCode::FAILURE;
                }
                eprintln!(
                    "[сохранено в daily_tips, chat_id={}, учтено прошлых советов: {}]",
                    id,
                    previous_tips.len()
                );
            }
            None => eprintln!("Не сохранено: чат не определён (нечего указать в chat_id)."),
        }
    }

    ExitCode::SUCCESS
}
