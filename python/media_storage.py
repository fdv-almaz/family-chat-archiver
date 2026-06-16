"""Download media files from Telegram to local storage at archive time."""
import os
import logging
from config import MEDIA_STORAGE_DIR, MEDIA_MAX_DOWNLOAD_SIZE

logger = logging.getLogger(__name__)


def _find_cached(file_unique_id: str) -> str | None:
    """Return path if a file with this unique_id is already stored."""
    if not file_unique_id:
        return None
    try:
        for name in os.listdir(MEDIA_STORAGE_DIR):
            if name.startswith(file_unique_id):
                return os.path.abspath(os.path.join(MEDIA_STORAGE_DIR, name))
    except OSError:
        pass
    return None


def download_and_save(bot, file_id: str, file_unique_id: str,
                      file_size: int | None = None,
                      suggested_ext: str = "") -> str | None:
    """
    Download a Telegram file via Bot API and save to local storage.
    Returns absolute path on success, None on failure or skip.
    """
    if not file_id or not file_unique_id:
        return None

    # Already cached?
    existing = _find_cached(file_unique_id)
    if existing:
        return existing

    # Telegram Bot API limit is ~20 MB. Skip larger files.
    if file_size and file_size > MEDIA_MAX_DOWNLOAD_SIZE:
        logger.info(f'Skipping download of {file_unique_id}: size {file_size} > limit {MEDIA_MAX_DOWNLOAD_SIZE}')
        return None

    try:
        file_info = bot.get_file(file_id)
    except Exception as e:
        logger.warning(f'get_file failed for {file_id}: {e}')
        return None

    # Determine extension
    _, ext = os.path.splitext(file_info.file_path or "")
    if not ext and suggested_ext:
        ext = suggested_ext if suggested_ext.startswith('.') else '.' + suggested_ext

    out_path = os.path.abspath(os.path.join(MEDIA_STORAGE_DIR, file_unique_id + ext))

    try:
        data = bot.download_file(file_info.file_path)
        with open(out_path, 'wb') as f:
            f.write(data)
        logger.debug(f'Saved media to {out_path} ({len(data)} bytes)')
        return out_path
    except Exception as e:
        logger.warning(f'download failed for {file_id}: {e}')
        return None
