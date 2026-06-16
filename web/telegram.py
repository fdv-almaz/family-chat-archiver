"""Download media files from Telegram on demand, with local caching."""
import os
import httpx
import logging
from config import TELEGRAM_BOT_TOKEN, MEDIA_CACHE_DIR

logger = logging.getLogger(__name__)

TELEGRAM_API = "https://api.telegram.org"


async def _get_file_path(file_id: str) -> str | None:
    """Resolve file_id to a relative path via getFile."""
    url = f"{TELEGRAM_API}/bot{TELEGRAM_BOT_TOKEN}/getFile"
    async with httpx.AsyncClient(timeout=15) as client:
        r = await client.get(url, params={"file_id": file_id})
        if r.status_code != 200:
            logger.warning(f"getFile failed: {r.status_code} {r.text}")
            return None
        data = r.json()
        if not data.get("ok"):
            logger.warning(f"getFile not ok: {data}")
            return None
        return data["result"].get("file_path")


async def fetch_media(file_id: str, file_unique_id: str, suggested_ext: str = "") -> str | None:
    """
    Return absolute path to cached file. Downloads from Telegram if not cached.
    Returns None on failure.
    """
    if not TELEGRAM_BOT_TOKEN:
        return None

    # Cache key by unique_id (stable across re-uploads)
    cache_pattern = os.path.join(MEDIA_CACHE_DIR, file_unique_id)
    # Check for any cached file with this unique_id prefix
    for fname in os.listdir(MEDIA_CACHE_DIR):
        if fname.startswith(file_unique_id):
            return os.path.join(MEDIA_CACHE_DIR, fname)

    file_path = await _get_file_path(file_id)
    if not file_path:
        return None

    # Determine extension from Telegram's file_path
    _, ext = os.path.splitext(file_path)
    if not ext and suggested_ext:
        ext = suggested_ext if suggested_ext.startswith('.') else '.' + suggested_ext

    cache_file = cache_pattern + ext

    download_url = f"{TELEGRAM_API}/file/bot{TELEGRAM_BOT_TOKEN}/{file_path}"
    async with httpx.AsyncClient(timeout=60) as client:
        async with client.stream("GET", download_url) as r:
            if r.status_code != 200:
                logger.warning(f"download failed: {r.status_code}")
                return None
            with open(cache_file, "wb") as f:
                async for chunk in r.aiter_bytes():
                    f.write(chunk)

    return cache_file


def mime_from_ext(path: str) -> str:
    ext = os.path.splitext(path)[1].lower()
    return {
        '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg',
        '.png': 'image/png', '.gif': 'image/gif', '.webp': 'image/webp',
        '.mp4': 'video/mp4', '.mov': 'video/quicktime', '.webm': 'video/webm',
        '.mp3': 'audio/mpeg', '.m4a': 'audio/mp4', '.ogg': 'audio/ogg',
        '.oga': 'audio/ogg', '.opus': 'audio/ogg',
        '.pdf': 'application/pdf',
    }.get(ext, 'application/octet-stream')
