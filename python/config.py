import os
import logging
from logging.handlers import TimedRotatingFileHandler
from dotenv import load_dotenv

load_dotenv()

# Read version from VERSION file at repo root (one level up from python/)
_VERSION_FILE = os.path.join(os.path.dirname(__file__), '..', 'VERSION')
try:
    with open(_VERSION_FILE) as _vf:
        VERSION = _vf.read().strip()
except OSError:
    VERSION = 'unknown'

TELEGRAM_BOT_TOKEN = os.getenv('TELEGRAM_BOT_TOKEN')
MYSQL_HOST = os.getenv('MYSQL_HOST', 'localhost')
MYSQL_PORT = int(os.getenv('MYSQL_PORT', '3306'))
MYSQL_USER = os.getenv('MYSQL_USER', 'root')
MYSQL_PASSWORD = os.getenv('MYSQL_PASSWORD', '')
MYSQL_DATABASE = os.getenv('MYSQL_DATABASE', 'family_chat')
LOG_LEVEL = os.getenv('LOG_LEVEL', 'INFO')

# Spelling correction visibility:
#   "public"  - send correction to chat (everyone sees, replies to original)
#   "private" - send DM to the message author (only they see; user must have started the bot)
#   "off"     - don't send to chat, only save to DB
SPELLING_VISIBILITY = os.getenv('SPELLING_VISIBILITY', 'public').lower()
if SPELLING_VISIBILITY not in ('public', 'private', 'off'):
    SPELLING_VISIBILITY = 'public'

# --- Совет дня (cron в 6:00, см. daily_tip.py) ---
# Ключ Anthropic API нужен только скрипту daily_tip.py (не самому боту).
ANTHROPIC_API_KEY = os.getenv('ANTHROPIC_API_KEY')
# Модель Claude для совета дня. По умолчанию — актуальная Opus.
ANTHROPIC_MODEL = os.getenv('ANTHROPIC_MODEL', 'claude-opus-4-8')
# Чат для рассылки совета. Если не задан — берётся самый активный групповой чат из БД.
_TIP_CHAT_ID_RAW = os.getenv('TIP_CHAT_ID')
TIP_CHAT_ID = int(_TIP_CHAT_ID_RAW) if _TIP_CHAT_ID_RAW else None
# Время рассылки (локальное), по умолчанию 06:00.
TIP_HOUR = int(os.getenv('TIP_HOUR', '6'))
TIP_MINUTE = int(os.getenv('TIP_MINUTE', '0'))

# Where to store downloaded media files (Telegram limit is ~20 MB per download via Bot API).
# A relative path is anchored to this module's directory (python/), NOT the current
# working directory — so files always land in python/storage regardless of how the bot
# is launched (cwd-independent; keeps local_path matching the web whitelist).
MEDIA_STORAGE_DIR = os.getenv('MEDIA_STORAGE_DIR', 'storage')
if not os.path.isabs(MEDIA_STORAGE_DIR):
    MEDIA_STORAGE_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), MEDIA_STORAGE_DIR))
os.makedirs(MEDIA_STORAGE_DIR, exist_ok=True)
# Skip downloading files larger than this (bytes). Telegram Bot API hard limit is 20 MB.
MEDIA_MAX_DOWNLOAD_SIZE = int(os.getenv('MEDIA_MAX_DOWNLOAD_SIZE', str(20 * 1024 * 1024)))

# File logging config
LOG_FILE = os.getenv('LOG_FILE', 'logs/bot.log')
LOG_RETENTION_DAYS = int(os.getenv('LOG_RETENTION_DAYS', '7'))
LOG_TO_CONSOLE = os.getenv('LOG_TO_CONSOLE', 'true').lower() in ('1', 'true', 'yes')

# Setup logging with file rotation + optional console output
log_dir = os.path.dirname(LOG_FILE)
if log_dir:
    os.makedirs(log_dir, exist_ok=True)

log_format = logging.Formatter('%(asctime)s - %(name)s - %(levelname)s - %(message)s')

# Rotating file handler: new file at midnight, keep LOG_RETENTION_DAYS days
file_handler = TimedRotatingFileHandler(
    LOG_FILE, when='midnight', interval=1,
    backupCount=LOG_RETENTION_DAYS, encoding='utf-8'
)
file_handler.setFormatter(log_format)
file_handler.suffix = '%Y-%m-%d'

handlers = [file_handler]
if LOG_TO_CONSOLE:
    console_handler = logging.StreamHandler()
    console_handler.setFormatter(log_format)
    handlers.append(console_handler)

logging.basicConfig(
    level=getattr(logging, LOG_LEVEL),
    handlers=handlers,
    force=True,
)

logger = logging.getLogger(__name__)

# Validate required config
if not TELEGRAM_BOT_TOKEN:
    raise ValueError('TELEGRAM_BOT_TOKEN is required')

logger.info(f'Family Chat Archiver v{VERSION}')
logger.info(f'Logging to {LOG_FILE} (retention: {LOG_RETENTION_DAYS} days, console: {LOG_TO_CONSOLE})')
logger.info(f'Config loaded: DB host={MYSQL_HOST}:{MYSQL_PORT}')
