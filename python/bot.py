import logging
import signal
import sys
import json
import re
import time
from telebot import TeleBot, types
from telebot.apihelper import ApiException

import config
from db import DBPool, create_tables, insert_or_update_user, insert_message, insert_media, insert_link, insert_spelling_correction, insert_service_event
from spelling import check_spelling, format_correction_message, format_chat_message

logger = logging.getLogger(__name__)

bot = TeleBot(config.TELEGRAM_BOT_TOKEN, parse_mode='HTML')
running = True

def extract_chat_title(chat) -> str:
    """Get chat title for groups/channels or 'first_name last_name' for private chats."""
    if chat.title:
        return chat.title
    parts = [chat.first_name, chat.last_name]
    return ' '.join(p for p in parts if p) or None

def message_context(message: types.Message) -> dict:
    """Extract user and chat context for message insertion."""
    user = message.from_user
    chat = message.chat
    return {
        'user_username': user.username if user else None,
        'user_first_name': user.first_name if user else None,
        'user_last_name': user.last_name if user else None,
        'chat_title': extract_chat_title(chat),
        'chat_type': chat.type,
    }

def safe_send(send_func, *args, max_retries=3, **kwargs):
    """Wrap bot.send_message/reply_to with retry on connection errors."""
    delay = 1
    for attempt in range(max_retries):
        try:
            return send_func(*args, **kwargs)
        except (ConnectionError, TimeoutError) as e:
            logger.warning(f'Send failed (attempt {attempt + 1}/{max_retries}): {e}')
            if attempt < max_retries - 1:
                time.sleep(delay)
                delay = min(delay * 2, 10)
        except ApiException as e:
            logger.error(f'Telegram API error: {e}')
            return None
        except Exception as e:
            error_str = str(e)
            if 'Connection aborted' in error_str or 'RemoteDisconnected' in error_str or 'ConnectionError' in error_str:
                logger.warning(f'Connection issue (attempt {attempt + 1}/{max_retries}): {e}')
                if attempt < max_retries - 1:
                    time.sleep(delay)
                    delay = min(delay * 2, 10)
            else:
                logger.error(f'Unexpected error in send: {e}')
                return None
    logger.error(f'Failed to send after {max_retries} attempts')
    return None

START_MESSAGE = """
<b>👋 Family Chat Archiver</b>

Это бот для архивирования всех сообщений в семейной группе.

<b>Основные возможности:</b>
✅ Сохранение всех сообщений (текст, фото, видео, документы)
✅ Сохранение информации об авторах
✅ Проверка орфографии русскоязычных текстов
✅ Сохранение ссылок и медиа-контента
✅ Запись служебных событий (вход/выход участников)

<b>Как это работает:</b>
Бот автоматически архивирует все сообщения в группе без участия пользователя. Для исправления орфографии используется YandexSpeller API.

<b>Хранение данных:</b>
Все данные сохраняются в защищённой MySQL базе данных.

<i>Бот работает в фоновом режиме и не требует команд.</i>
"""

@bot.message_handler(commands=['start', 'help'])
def handle_start(message: types.Message):
    try:
        safe_send(bot.reply_to, message, START_MESSAGE)
        logger.debug(f'Start command handled for user {message.from_user.id}')
    except Exception as e:
        logger.error(f'Error handling start command: {e}')

def signal_handler(signum, frame):
    global running
    logger.info('Graceful shutdown initiated')
    running = False
    sys.exit(0)

@bot.message_handler(content_types=['text'])
def handle_text_message(message: types.Message):
    try:
        if not message.text:
            return

        # Skip messages from bots (including self) to prevent loops
        if message.from_user and message.from_user.is_bot:
            return

        # Save user
        insert_or_update_user(message.from_user)

        # Save message
        insert_message(
            message.message_id,
            message.from_user.id,
            message.chat.id,
            message.text,
            'text',
            **message_context(message)
        )

        # Extract links from text
        urls = re.findall(r'https?://\S+', message.text)
        for url in urls:
            insert_link(message.message_id, url)

        # Check spelling
        spelling_errors = check_spelling(message.text)
        if spelling_errors:
            corrected_text, processed_errors = format_correction_message(message.text, spelling_errors)

            # Save correction to DB
            errors_json = json.dumps(processed_errors, ensure_ascii=False)
            insert_spelling_correction(
                message.message_id,
                message.text,
                corrected_text,
                errors_json,
                sent_to_chat=True
            )

            # Send correction to chat (visible to all)
            correction_message = format_chat_message(message.text, corrected_text, processed_errors)
            if correction_message:
                safe_send(bot.send_message, message.chat.id, correction_message)

        logger.debug(f'Text message processed: user={message.from_user.id}, message_id={message.message_id}')

    except Exception as e:
        logger.error(f'Error handling text message: {e}')

@bot.message_handler(content_types=['photo'])
def handle_photo_message(message: types.Message):
    try:
        if message.from_user and message.from_user.is_bot:
            return

        insert_or_update_user(message.from_user)

        caption = message.caption or ''
        insert_message(
            message.message_id,
            message.from_user.id,
            message.chat.id,
            caption,
            'photo',
            **message_context(message)
        )

        # Save photo info
        photo = message.photo[-1]
        insert_media(
            message.message_id,
            'photo',
            photo.file_id,
            photo.file_unique_id,
            photo.file_size
        )

        # Check spelling in caption if present
        if caption:
            spelling_errors = check_spelling(caption)
            if spelling_errors:
                corrected_text, processed_errors = format_correction_message(caption, spelling_errors)
                errors_json = json.dumps(processed_errors, ensure_ascii=False)
                insert_spelling_correction(
                    message.message_id,
                    caption,
                    corrected_text,
                    errors_json,
                    sent_to_chat=True
                )
                correction_message = format_chat_message(caption, corrected_text, processed_errors)
                if correction_message:
                    safe_send(bot.send_message, message.chat.id, correction_message)

        logger.debug(f'Photo message processed: user={message.from_user.id}, message_id={message.message_id}')

    except Exception as e:
        logger.error(f'Error handling photo message: {e}')

MEDIA_TYPE_MAP = {
    'video': lambda m: m.video,
    'document': lambda m: m.document,
    'voice': lambda m: m.voice,
    'audio': lambda m: m.audio,
    'video_note': lambda m: m.video_note,
    'animation': lambda m: m.animation,
    'sticker': lambda m: m.sticker,
}

@bot.message_handler(content_types=list(MEDIA_TYPE_MAP.keys()))
def handle_media_message(message: types.Message):
    try:
        if message.from_user and message.from_user.is_bot:
            return

        insert_or_update_user(message.from_user)

        # Detect which media type is present
        message_type = None
        media = None
        for mtype, getter in MEDIA_TYPE_MAP.items():
            m = getter(message)
            if m:
                message_type = mtype
                media = m
                break

        if not media:
            return

        caption = message.caption or ''
        insert_message(
            message.message_id,
            message.from_user.id,
            message.chat.id,
            caption,
            message_type,
            **message_context(message)
        )

        insert_media(
            message.message_id,
            message_type,
            getattr(media, 'file_id', None),
            getattr(media, 'file_unique_id', None),
            file_size=getattr(media, 'file_size', None),
            mime_type=getattr(media, 'mime_type', None),
            file_name=getattr(media, 'file_name', None),
            duration=getattr(media, 'duration', None),
        )

        # Check spelling in caption
        if caption:
            spelling_errors = check_spelling(caption)
            if spelling_errors:
                corrected_text, processed_errors = format_correction_message(caption, spelling_errors)
                errors_json = json.dumps(processed_errors, ensure_ascii=False)
                insert_spelling_correction(
                    message.message_id,
                    caption,
                    corrected_text,
                    errors_json,
                    sent_to_chat=True
                )
                correction_message = format_chat_message(caption, corrected_text, processed_errors)
                if correction_message:
                    safe_send(bot.send_message, message.chat.id, correction_message)

        logger.debug(f'{message_type} message processed: user={message.from_user.id}, message_id={message.message_id}')

    except Exception as e:
        logger.exception(f'Error handling media message: {e}')

@bot.message_handler(content_types=['contact', 'location', 'venue', 'poll', 'dice'])
def handle_special_message(message: types.Message):
    """Handle contact, location, venue, poll, dice messages."""
    try:
        if message.from_user and message.from_user.is_bot:
            return

        insert_or_update_user(message.from_user)

        message_type = message.content_type  # 'contact', 'location', etc.

        # Build descriptive text payload
        text_parts = []
        if message.contact:
            c = message.contact
            text_parts.append(f'Contact: {c.first_name or ""} {c.last_name or ""} {c.phone_number or ""}'.strip())
        if message.location:
            loc = message.location
            text_parts.append(f'Location: lat={loc.latitude}, lon={loc.longitude}')
        if message.venue:
            v = message.venue
            text_parts.append(f'Venue: {v.title} ({v.address})')
        if message.poll:
            p = message.poll
            opts = '; '.join(o.text for o in p.options)
            text_parts.append(f'Poll: {p.question} [{opts}]')
        if message.dice:
            text_parts.append(f'Dice: {message.dice.emoji} = {message.dice.value}')

        insert_message(
            message.message_id,
            message.from_user.id,
            message.chat.id,
            ' | '.join(text_parts),
            message_type,
            **message_context(message)
        )
        logger.debug(f'{message_type} message processed: message_id={message.message_id}')

    except Exception as e:
        logger.exception(f'Error handling special message: {e}')

@bot.message_handler(content_types=[
    'new_chat_members', 'left_chat_member', 'new_chat_title',
    'new_chat_photo', 'delete_chat_photo', 'group_chat_created',
    'supergroup_chat_created', 'channel_chat_created',
    'migrate_to_chat_id', 'migrate_from_chat_id', 'pinned_message'
])
def handle_other_events(message: types.Message):
    try:
        event_type = None

        if message.new_chat_members:
            event_type = 'user_joined'
            for new_member in message.new_chat_members:
                insert_or_update_user(new_member)
        elif message.left_chat_member:
            event_type = 'user_left'
            insert_or_update_user(message.left_chat_member)
        elif message.new_chat_title:
            event_type = 'title_changed'
        elif message.new_chat_photo:
            event_type = 'photo_changed'
        elif message.delete_chat_photo:
            event_type = 'photo_deleted'
        elif message.group_chat_created:
            event_type = 'group_created'
        elif message.supergroup_chat_created:
            event_type = 'supergroup_created'
        elif message.channel_chat_created:
            event_type = 'channel_created'
        elif message.pinned_message:
            event_type = 'message_pinned'

        if event_type:
            data = {
                'message_id': message.message_id,
                'title': message.new_chat_title if message.new_chat_title else None
            }
            insert_service_event(
                message.chat.id,
                event_type,
                message.from_user.id if message.from_user else None,
                json.dumps(data, ensure_ascii=False),
                chat_title=extract_chat_title(message.chat),
                user_username=message.from_user.username if message.from_user else None,
                user_first_name=message.from_user.first_name if message.from_user else None,
            )
            logger.debug(f'Service event recorded: {event_type}')

    except Exception as e:
        logger.error(f'Error handling service event: {e}')

def main():
    global running

    # Setup signal handler for graceful shutdown
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)

    logger.info('Initializing bot...')

    try:
        # Initialize database
        create_tables()
        logger.info('Database initialized')

        # infinity_polling has built-in retry and won't raise on network errors
        # restart_on_change=False prevents auto-restart on code changes
        logger.info('Bot started, polling for messages...')
        bot.infinity_polling(
            timeout=30,
            long_polling_timeout=30,
            restart_on_change=False,
            logger_level=logging.WARNING
        )

    except KeyboardInterrupt:
        logger.info('Bot stopped by user')
    except Exception as e:
        logger.error(f'Fatal error: {e}')
        raise
    finally:
        logger.info('Bot shutdown complete')

if __name__ == '__main__':
    main()
