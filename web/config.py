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

MEDIA_CACHE_DIR = os.getenv('MEDIA_CACHE_DIR', 'media_cache')
os.makedirs(MEDIA_CACHE_DIR, exist_ok=True)
