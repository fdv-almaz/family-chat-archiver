#!/usr/bin/env python3
"""Разовая генерация «совета дня» через Claude — без Telegram.

Самостоятельный скрипт, повторяющий ровно ту же логику, что и бот
(`daily_tip.py`), но не зависящий от TeleBot: он не шлёт совет в чат, а просто
печатает его в stdout. Данные из семейной БД берутся тем же способом, что и в
боте:
  * чат определяется как `TIP_CHAT_ID` или самый активный групповой чат
    (`db.get_most_active_chat_id`);
  * для антиповтора подгружаются последние `TIP_HISTORY_LIMIT` отправленных
    советов этого чата (`db.get_recent_tips`) и передаются модели с инструкцией
    не повторяться;
  * системный и user-промпт строятся теми же функциями из `daily_tip.py`.

Запуск (из каталога python/, рядом с тем же .env, что и у бота):
    python generate_tip.py                 # сгенерировать и напечатать совет
    python generate_tip.py --chat-id -100… # взять историю для конкретного чата
    python generate_tip.py --no-history     # без учёта прошлых советов
    python generate_tip.py --save           # ещё и записать в таблицу daily_tips

По умолчанию в БД ничего не пишется — это «только чтение»: скрипт лишь
запрашивает совет и выводит его. Флаг --save сохраняет пару запрос/ответ в
`daily_tips` (как делает бот; sent_to_chat=False, ведь в чат ничего не уходит).
"""
import argparse
import sys

import config
from daily_tip import build_user_prompt, generate_tip, resolve_chat_id
from db import get_recent_tips, insert_daily_tip


def main() -> int:
    parser = argparse.ArgumentParser(
        description='Разовый запрос «совета дня» к Claude (без отправки в Telegram).')
    parser.add_argument(
        '--chat-id', type=int, default=None,
        help='ID чата для подбора истории советов (по умолчанию — как у бота: '
             'TIP_CHAT_ID или самый активный групповой чат из БД).')
    parser.add_argument(
        '--no-history', action='store_true',
        help='Не подгружать прошлые советы из БД (отключить антиповтор).')
    parser.add_argument(
        '--save', action='store_true',
        help='Сохранить запрос/ответ в таблицу daily_tips (sent_to_chat=False).')
    args = parser.parse_args()

    if not config.ANTHROPIC_API_KEY:
        print('Ошибка: ANTHROPIC_API_KEY не задан в .env — генерация невозможна.',
              file=sys.stderr)
        return 1

    # Чат нужен только для подбора истории советов (антиповтор). Сам совет в
    # никакой чат не отправляется, поэтому при --no-history чат не обязателен.
    chat_id = args.chat_id if args.chat_id is not None else resolve_chat_id()

    previous_tips = []
    if not args.no_history:
        if chat_id is None:
            print('Предупреждение: чат не определён (TIP_CHAT_ID не задан и в БД '
                  'нет групповых чатов) — генерирую без учёта истории.',
                  file=sys.stderr)
        else:
            previous_tips = get_recent_tips(chat_id, config.TIP_HISTORY_LIMIT)

    user_prompt = build_user_prompt(previous_tips)

    try:
        tip = generate_tip(user_prompt)
    except Exception as e:
        print(f'Ошибка запроса к Claude API: {e}', file=sys.stderr)
        return 1

    if not tip:
        print('Claude вернул пустой ответ.', file=sys.stderr)
        return 1

    print(tip)

    if args.save:
        full_prompt = f"{config.TIP_SYSTEM_PROMPT}\n\n---\n{user_prompt}"
        insert_daily_tip(chat_id, config.ANTHROPIC_MODEL, full_prompt, tip,
                         sent_to_chat=False)
        print(f'\n[сохранено в daily_tips, chat_id={chat_id}, '
              f'учтено прошлых советов: {len(previous_tips)}]', file=sys.stderr)

    return 0


if __name__ == '__main__':
    raise SystemExit(main())
