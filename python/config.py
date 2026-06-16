import os
import logging
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

# Setup logging
logging.basicConfig(
    level=getattr(logging, LOG_LEVEL),
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)

logger = logging.getLogger(__name__)

# Validate required config
if not TELEGRAM_BOT_TOKEN:
    raise ValueError('TELEGRAM_BOT_TOKEN is required')

logger.info(f'Config loaded: DB={MYSQL_USER}@{MYSQL_HOST}:{MYSQL_PORT}/{MYSQL_DATABASE}')
