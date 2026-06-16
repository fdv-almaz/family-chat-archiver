import os
import logging
from logging.handlers import TimedRotatingFileHandler
from dotenv import load_dotenv

load_dotenv()

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

logger.info(f'Logging to {LOG_FILE} (retention: {LOG_RETENTION_DAYS} days, console: {LOG_TO_CONSOLE})')
logger.info(f'Config loaded: DB={MYSQL_USER}@{MYSQL_HOST}:{MYSQL_PORT}/{MYSQL_DATABASE}')
