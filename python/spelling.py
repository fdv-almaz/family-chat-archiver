import logging
import requests
import json
import time
from typing import Optional, Dict, List

logger = logging.getLogger(__name__)

YANDEX_SPELLER_API = 'https://speller.yandex.net/services/spellchecker.json/checkTexts'

def check_spelling(text: str, max_retries: int = 3) -> Optional[List[Dict]]:
    if not text or len(text.strip()) < 2:
        return None

    # Skip commands (text starting with /)
    if text.strip().startswith('/'):
        return None

    # Filter out text with only special characters
    cleaned = ''.join(c for c in text if c.isalnum() or c.isspace())
    if len(cleaned.strip()) < 2:
        return None

    for attempt in range(max_retries):
        try:
            response = requests.post(
                YANDEX_SPELLER_API,
                params={'text': text},
                timeout=5
            )
            response.raise_for_status()
            errors = response.json()
            if errors and len(errors) > 0:
                return errors[0] if isinstance(errors, list) and errors else None
            return None
        except requests.exceptions.Timeout:
            logger.debug(f'YandexSpeller timeout (attempt {attempt + 1}/{max_retries})')
            if attempt < max_retries - 1:
                time.sleep(1)
        except requests.exceptions.HTTPError as e:
            if e.response.status_code == 400:
                logger.debug(f'YandexSpeller bad request for text: {text[:50]}... (skipping)')
                return None
            logger.debug(f'YandexSpeller HTTP error: {e} (attempt {attempt + 1}/{max_retries})')
            if attempt < max_retries - 1:
                time.sleep(1)
        except requests.exceptions.RequestException as e:
            logger.debug(f'YandexSpeller request error: {e} (attempt {attempt + 1}/{max_retries})')
            if attempt < max_retries - 1:
                time.sleep(1)

    logger.debug(f'Failed to check spelling after {max_retries} attempts for text: {text[:50]}...')
    return None

def format_correction_message(text: str, errors: List[Dict]) -> tuple[str, List[Dict]]:
    if not errors:
        return text, []

    corrected_text = text
    processed_errors = []

    for error in errors:
        if error.get('s'):
            suggestions = error['s']
            best_suggestion = suggestions[0] if suggestions else error['word']

            corrected_text = corrected_text.replace(error['word'], best_suggestion, 1)
            processed_errors.append({
                'original': error['word'],
                'suggested': best_suggestion,
                'position': error.get('pos', -1),
                'all_suggestions': suggestions
            })

    return corrected_text, processed_errors

def format_chat_message(text: str, corrected_text: str, errors: List[Dict]) -> str:
    if not errors:
        return None

    lines = [
        '📚 <b>Орфографическая подсказка!</b>',
        '',
        f'<i>Было:</i> <code>{text}</code>',
        f'<i>✨ Будет:</i> <code>{corrected_text}</code>',
        '',
        '🎯 <b>Что исправить:</b>'
    ]

    for error in errors:
        original = error['original']
        suggestions = ', '.join(error['all_suggestions'][:3])
        lines.append(f'  • <b>{original}</b> → <i>{suggestions}</i>')

    lines.extend([
        '',
        '💡 <i>Совет: обрати внимание на эти слова в следующий раз!</i>',
        '😊 <b>Спасибо за внимание к орфографии!</b>'
    ])

    return '\n'.join(lines)
