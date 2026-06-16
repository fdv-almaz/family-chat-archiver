import logging
import signal
import sys
import json
import re
import time
from telebot import TeleBot, types

import config
from db import DBPool, create_tables, insert_or_update_user, insert_message, insert_media, insert_link, insert_spelling_correction, insert_service_event
from spelling import check_spelling, format_correction_message, format_chat_message

logger = logging.getLogger(__name__)

bot = TeleBot(config.TELEGRAM_BOT_TOKEN, parse_mode='HTML')
running = True

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
        bot.reply_to(message, START_MESSAGE)
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

        # Save user
        insert_or_update_user(message.from_user)

        # Save message
        insert_message(
            message.message_id,
            message.from_user.id,
            message.chat.id,
            message.text,
            'text'
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

            # Send correction to chat
            correction_message = format_chat_message(message.text, corrected_text, processed_errors)
            if correction_message:
                bot.reply_to(message, correction_message)

        logger.debug(f'Text message processed: user={message.from_user.id}, message_id={message.message_id}')

    except Exception as e:
        logger.error(f'Error handling text message: {e}')

@bot.message_handler(content_types=['photo'])
def handle_photo_message(message: types.Message):
    try:
        insert_or_update_user(message.from_user)

        caption = message.caption or ''
        insert_message(
            message.message_id,
            message.from_user.id,
            message.chat.id,
            caption,
            'photo'
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
                    bot.reply_to(message, correction_message)

        logger.debug(f'Photo message processed: user={message.from_user.id}, message_id={message.message_id}')

    except Exception as e:
        logger.error(f'Error handling photo message: {e}')

@bot.message_handler(content_types=['video', 'document', 'voice'])
def handle_media_message(message: types.Message):
    try:
        insert_or_update_user(message.from_user)

        message_type = None
        media = None
        file_size = None

        if message.video:
            message_type = 'video'
            media = message.video
            file_size = message.video.file_size
        elif message.document:
            message_type = 'document'
            media = message.document
            file_size = message.document.file_size
        elif message.voice:
            message_type = 'voice'
            media = message.voice
            file_size = message.voice.file_size

        caption = message.caption or ''
        insert_message(
            message.message_id,
            message.from_user.id,
            message.chat.id,
            caption,
            message_type
        )

        if media:
            insert_media(
                message.message_id,
                message_type,
                media.file_id,
                media.file_unique_id,
                file_size,
                media.mime_type if hasattr(media, 'mime_type') else None
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
                    bot.reply_to(message, correction_message)

        logger.debug(f'{message_type} message processed: user={message.from_user.id}, message_id={message.message_id}')

    except Exception as e:
        logger.error(f'Error handling media message: {e}')

@bot.message_handler(func=lambda message: True)
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

        if event_type:
            data = {
                'message_id': message.message_id,
                'title': message.new_chat_title if message.new_chat_title else None
            }
            insert_service_event(
                message.chat.id,
                event_type,
                message.from_user.id if message.from_user else None,
                json.dumps(data, ensure_ascii=False)
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

        # Start polling with reconnection logic
        logger.info('Bot started, polling for messages...')
        retry_count = 0
        max_retries = 5
        retry_delay = 5

        while running:
            try:
                bot.infinity_polling(timeout=30, long_polling_timeout=30)
            except KeyboardInterrupt:
                logger.info('Bot stopped by user')
                break
            except (ConnectionError, TimeoutError) as e:
                retry_count += 1
                if retry_count <= max_retries:
                    logger.warning(f'Connection error (attempt {retry_count}/{max_retries}): {e}')
                    logger.info(f'Reconnecting in {retry_delay} seconds...')
                    time.sleep(retry_delay)
                    retry_delay = min(retry_delay * 2, 60)
                else:
                    logger.error(f'Max retries ({max_retries}) exceeded. Stopping bot.')
                    raise
            except Exception as e:
                logger.error(f'Unexpected error: {e}')
                retry_count += 1
                if retry_count <= max_retries:
                    logger.info(f'Retrying in {retry_delay} seconds...')
                    time.sleep(retry_delay)
                    retry_delay = min(retry_delay * 2, 60)
                else:
                    raise

    except KeyboardInterrupt:
        logger.info('Bot stopped by user')
    except Exception as e:
        logger.error(f'Fatal error: {e}')
        raise
    finally:
        logger.info('Bot shutdown complete')

if __name__ == '__main__':
    main()
