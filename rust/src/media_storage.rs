use std::path::{Path, PathBuf};
use log::{debug, warn};
use teloxide::prelude::*;
use teloxide::net::Download;
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Returns existing cached path if a file with this unique_id already exists.
async fn find_cached(storage_dir: &str, file_unique_id: &str) -> Option<PathBuf> {
    let mut entries = match fs::read_dir(storage_dir).await {
        Ok(e) => e,
        Err(_) => return None,
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        if let Some(name) = entry.file_name().to_str() {
            if name.starts_with(file_unique_id) {
                let path = entry.path();
                return std::fs::canonicalize(&path).ok().or(Some(path));
            }
        }
    }
    None
}

fn to_absolute(p: &Path) -> PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(p))
            .unwrap_or_else(|_| p.to_path_buf())
    }
}

/// Download a Telegram file via Bot API and save to storage_dir.
/// Returns absolute path on success, None on failure or skip.
pub async fn download_and_save(
    bot: &Bot,
    storage_dir: &str,
    file_id: &str,
    file_unique_id: &str,
    file_size: Option<i64>,
    max_size: u64,
) -> Option<String> {
    if file_id.is_empty() || file_unique_id.is_empty() {
        return None;
    }

    // Already cached?
    if let Some(p) = find_cached(storage_dir, file_unique_id).await {
        return p.to_str().map(|s| s.to_string());
    }

    // Skip files exceeding Bot API limit (~20 MB).
    if let Some(sz) = file_size {
        if sz as u64 > max_size {
            debug!("Skipping download of {}: size {} > limit {}", file_unique_id, sz, max_size);
            return None;
        }
    }

    // Resolve file path via getFile
    let tg_file = match bot.get_file(file_id).await {
        Ok(f) => f,
        Err(e) => {
            warn!("get_file failed for {}: {}", file_id, e);
            return None;
        }
    };

    // Pick extension from telegram-provided path
    let ext = Path::new(&tg_file.path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();

    let out_path = to_absolute(&PathBuf::from(storage_dir).join(format!("{}{}", file_unique_id, ext)));

    let mut file = match fs::File::create(&out_path).await {
        Ok(f) => f,
        Err(e) => {
            warn!("failed to create {}: {}", out_path.display(), e);
            return None;
        }
    };

    if let Err(e) = bot.download_file(&tg_file.path, &mut file).await {
        warn!("download failed for {}: {}", file_id, e);
        let _ = fs::remove_file(&out_path).await;
        return None;
    }
    let _ = file.flush().await;

    debug!("Saved media to {}", out_path.display());
    out_path.to_str().map(|s| s.to_string())
}
