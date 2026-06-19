"""Совет дня — фоновая задача внутри бота.

Ежедневно в TIP_HOUR:TIP_MINUTE (по умолчанию 6:00) генерирует через Claude API
короткий «совет дня» и отправляет его в семейный чат. Запрос и ответ
сохраняются в таблицу `daily_tips`. Планировщик запускается из bot.py в
фоновом потоке и работает в том же процессе, что и бот.

Можно вызвать вручную для проверки (один раз, без ожидания расписания):
    python daily_tip.py
"""
import html
import logging
import threading
import time
from datetime import date, datetime, timedelta

import anthropic
from telebot.apihelper import ApiException

import config
from db import get_most_active_chat_id, insert_daily_tip

logger = logging.getLogger(__name__)

# Учитываем, что в чате есть и взрослые, и дети: совет должен быть безопасным
# для любого возраста и не затрагивать неподходящие темы.
SYSTEM_PROMPT = (
    "Ты — добрый помощник в семейном групповом чате. В чате есть и взрослые, "
    "и дети. Каждое утро ты присылаешь короткий «совет дня».\n\n"
    "Правила:\n"
    "- Пиши только по-русски, тепло и дружелюбно, на «вы» ко всем сразу.\n"
    "- Один совет за раз, 2–5 предложений.\n"
    "- Темы: полезные привычки, быт и уют, здоровье и движение, учёба и "
    "развитие, добрые отношения в семье, безопасность, любопытный факт.\n"
    "- Совет должен быть безопасным и понятным для всех возрастов, включая "
    "детей.\n"
    "- Не затрагивай темы, не подходящие детям: политику, религиозные споры, "
    "конкретные лекарства и медицинские предписания, финансовые риски и "
    "инвестиции, любой контент 18+.\n"
    "- Не используй Markdown-разметку; эмодзи — максимум один и только если "
    "уместно.\n"
    "- Не пиши вступлений вроде «Вот совет дня» — сразу сам совет."
)


def build_user_prompt() -> str:
    """User-турн с текущей датой — чтобы советы не повторялись день ото дня."""
    return f"Сегодня {date.today().isoformat()}. Пришли совет дня."


def generate_tip(user_prompt: str) -> str:
    """Запросить совет у Claude и вернуть текст."""
    client = anthropic.Anthropic(api_key=config.ANTHROPIC_API_KEY)
    response = client.messages.create(
        model=config.ANTHROPIC_MODEL,
        max_tokens=1024,
        system=SYSTEM_PROMPT,
        messages=[{"role": "user", "content": user_prompt}],
    )
    parts = [block.text for block in response.content if block.type == "text"]
    return "".join(parts).strip()


def resolve_chat_id():
    """TIP_CHAT_ID из конфига или самый активный групповой чат из БД."""
    if config.TIP_CHAT_ID is not None:
        return config.TIP_CHAT_ID
    chat_id = get_most_active_chat_id()
    if chat_id is not None:
        logger.info(f'TIP_CHAT_ID не задан, выбран самый активный чат: {chat_id}')
    return chat_id


def run_once(bot, chat_id=None) -> bool:
    """Сгенерировать и отправить один совет; сохранить запрос/ответ в БД.

    `bot` — общий экземпляр TeleBot из bot.py (parse_mode='HTML').
    `chat_id` — если задан (команда /check_tip), совет шлётся именно туда;
    иначе используется TIP_CHAT_ID или самый активный групповой чат.
    Возвращает True, если совет успешно отправлен.
    """
    if not config.ANTHROPIC_API_KEY:
        logger.error('ANTHROPIC_API_KEY не задан — совет дня пропущен')
        return False

    if chat_id is None:
        chat_id = resolve_chat_id()
    if chat_id is None:
        logger.error('Не удалось определить чат для совета дня '
                     '(TIP_CHAT_ID не задан и в БД нет групповых чатов)')
        return False

    user_prompt = build_user_prompt()
    full_prompt = f"{SYSTEM_PROMPT}\n\n---\n{user_prompt}"

    # 1) Сгенерировать совет
    try:
        tip = generate_tip(user_prompt)
    except Exception as e:
        logger.error(f'Ошибка генерации совета через Claude API: {e}')
        insert_daily_tip(chat_id, config.ANTHROPIC_MODEL, full_prompt, None, False, error=str(e))
        return False

    if not tip:
        logger.error('Claude вернул пустой ответ')
        insert_daily_tip(chat_id, config.ANTHROPIC_MODEL, full_prompt, None, False, error='empty response')
        return False

    # 2) Отправить в чат. Бот работает в режиме HTML, поэтому экранируем текст.
    sent = False
    error = None
    try:
        bot.send_message(chat_id, html.escape(tip))
        sent = True
        logger.info(f'Совет дня отправлен в чат {chat_id}')
    except ApiException as e:
        error = str(e)
        logger.error(f'Не удалось отправить совет дня в чат {chat_id}: {e}')

    # 3) Сохранить запрос и ответ в БД
    insert_daily_tip(chat_id, config.ANTHROPIC_MODEL, full_prompt, tip, sent, error=error)
    return sent


def _seconds_until(hour: int, minute: int) -> float:
    """Секунд до ближайшего наступления заданного времени (локальное время)."""
    now = datetime.now()
    target = now.replace(hour=hour, minute=minute, second=0, microsecond=0)
    if target <= now:
        target += timedelta(days=1)
    return (target - now).total_seconds()


def _scheduler_loop(bot):
    while True:
        secs = _seconds_until(config.TIP_HOUR, config.TIP_MINUTE)
        logger.info(f'Следующий совет дня через {int(secs)} c')
        # Спим кусками по 60 c (поток демонический — корректно завершится с процессом).
        remaining = secs
        while remaining > 0:
            chunk = min(60.0, remaining)
            time.sleep(chunk)
            remaining -= chunk
        try:
            run_once(bot)
        except Exception as e:
            logger.error(f'Совет дня: непредвиденная ошибка: {e}')
        # Сдвиг, чтобы следующий расчёт пришёлся уже на завтрашний день.
        time.sleep(60)


def start_scheduler(bot):
    """Запустить фоновый планировщик. Без ANTHROPIC_API_KEY — тихо выходит."""
    if not config.ANTHROPIC_API_KEY:
        logger.info('Совет дня отключён: ANTHROPIC_API_KEY не задан')
        return
    thread = threading.Thread(target=_scheduler_loop, args=(bot,),
                              daemon=True, name='daily-tip')
    thread.start()
    logger.info(f'Планировщик совета дня запущен '
                f'(ежедневно в {config.TIP_HOUR:02d}:{config.TIP_MINUTE:02d})')


if __name__ == '__main__':
    # Ручной разовый запуск для проверки (без ожидания расписания).
    from telebot import TeleBot
    from db import create_tables

    create_tables()
    _bot = TeleBot(config.TELEGRAM_BOT_TOKEN, parse_mode='HTML')
    run_once(_bot)
