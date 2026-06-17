import os
from dotenv import load_dotenv

load_dotenv()

MYSQL_HOST = os.getenv('MYSQL_HOST', 'localhost')
MYSQL_PORT = int(os.getenv('MYSQL_PORT', '3306'))
MYSQL_USER = os.getenv('MYSQL_USER', 'root')
MYSQL_PASSWORD = os.getenv('MYSQL_PASSWORD', '')
MYSQL_DATABASE = os.getenv('MYSQL_DATABASE', 'family_chat')

TELEGRAM_BOT_TOKEN = os.getenv('TELEGRAM_BOT_TOKEN', '')

WEB_HOST = os.getenv('WEB_HOST', '127.0.0.1')
WEB_PORT = int(os.getenv('WEB_PORT', '8000'))

# Extra allowed Origin/Referer hosts (besides the request's own Host).
# Comma-separated list, e.g. "archive.example.com,archive.example.com:8000"
ALLOWED_ORIGINS = [
    o.strip() for o in os.getenv('ALLOWED_ORIGINS', '').split(',') if o.strip()
]

MEDIA_CACHE_DIR = os.getenv('MEDIA_CACHE_DIR', 'media_cache')
os.makedirs(MEDIA_CACHE_DIR, exist_ok=True)

# Whitelist of directories from which /media/{id} is allowed to serve files.
# Comma-separated; defaults cover the bot storage folders and the local cache.
# Used to defend against path traversal if media.local_path is tampered with.
_default_allowed = ','.join([
    os.path.abspath(MEDIA_CACHE_DIR),
    os.path.abspath(os.path.join(os.path.dirname(__file__), '..', 'python', 'storage')),
    os.path.abspath(os.path.join(os.path.dirname(__file__), '..', 'rust', 'storage')),
])
ALLOWED_MEDIA_DIRS = [
    os.path.abspath(p.strip())
    for p in os.getenv('ALLOWED_MEDIA_DIRS', _default_allowed).split(',')
    if p.strip()
]

# Project version (read from VERSION at repo root)
_VERSION_FILE = os.path.join(os.path.dirname(__file__), '..', 'VERSION')
try:
    with open(_VERSION_FILE) as _vf:
        VERSION = _vf.read().strip()
except OSError:
    VERSION = 'unknown'
