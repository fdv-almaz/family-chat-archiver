import mysql.connector
from mysql.connector import pooling
from config import MYSQL_HOST, MYSQL_PORT, MYSQL_USER, MYSQL_PASSWORD, MYSQL_DATABASE

_pool = pooling.MySQLConnectionPool(
    pool_name="web_pool",
    pool_size=5,
    host=MYSQL_HOST,
    port=MYSQL_PORT,
    user=MYSQL_USER,
    password=MYSQL_PASSWORD,
    database=MYSQL_DATABASE,
    charset='utf8mb4',
)


def query(sql: str, params=None, one: bool = False):
    conn = _pool.get_connection()
    try:
        cursor = conn.cursor(dictionary=True)
        cursor.execute(sql, params or ())
        rows = cursor.fetchall()
        cursor.close()
        return rows[0] if one and rows else (None if one else rows)
    finally:
        conn.close()


def execute(sql: str, params=None) -> int:
    conn = _pool.get_connection()
    try:
        cursor = conn.cursor()
        cursor.execute(sql, params or ())
        conn.commit()
        affected = cursor.rowcount
        cursor.close()
        return affected
    finally:
        conn.close()


def list_messages(chat_id=None, user_id=None, message_type=None,
                  date_from=None, date_to=None, search=None,
                  limit=50, offset=0):
    where = []
    params = []

    if chat_id:
        where.append("m.chat_id = %s")
        params.append(chat_id)
    if user_id:
        where.append("m.user_id = %s")
        params.append(user_id)
    if message_type:
        where.append("m.message_type = %s")
        params.append(message_type)
    if date_from:
        where.append("m.created_at >= %s")
        params.append(date_from)
    if date_to:
        where.append("m.created_at <= %s")
        params.append(date_to)
    if search:
        where.append("m.text LIKE %s")
        params.append(f"%{search}%")

    where_sql = ("WHERE " + " AND ".join(where)) if where else ""

    sql = f"""
        SELECT m.*,
               COALESCE(u.first_name, m.user_first_name) AS display_first_name,
               COALESCE(u.username, m.user_username) AS display_username
        FROM messages m
        LEFT JOIN users u ON u.user_id = m.user_id
        {where_sql}
        ORDER BY m.created_at DESC
        LIMIT %s OFFSET %s
    """
    params.extend([limit, offset])
    return query(sql, params)


def count_messages(chat_id=None, user_id=None, message_type=None,
                   date_from=None, date_to=None, search=None):
    where = []
    params = []
    if chat_id:
        where.append("chat_id = %s"); params.append(chat_id)
    if user_id:
        where.append("user_id = %s"); params.append(user_id)
    if message_type:
        where.append("message_type = %s"); params.append(message_type)
    if date_from:
        where.append("created_at >= %s"); params.append(date_from)
    if date_to:
        where.append("created_at <= %s"); params.append(date_to)
    if search:
        where.append("text LIKE %s"); params.append(f"%{search}%")

    where_sql = ("WHERE " + " AND ".join(where)) if where else ""
    row = query(f"SELECT COUNT(*) AS c FROM messages {where_sql}", params, one=True)
    return row['c'] if row else 0


def get_message(message_id: int):
    msg = query("""
        SELECT m.*,
               COALESCE(u.first_name, m.user_first_name) AS display_first_name,
               COALESCE(u.username, m.user_username) AS display_username
        FROM messages m
        LEFT JOIN users u ON u.user_id = m.user_id
        WHERE m.message_id = %s
    """, (message_id,), one=True)
    if not msg:
        return None
    msg['media'] = query("SELECT * FROM media WHERE message_id = %s", (message_id,))
    msg['links'] = query("SELECT * FROM links WHERE message_id = %s", (message_id,))
    msg['corrections'] = query(
        "SELECT * FROM spelling_corrections WHERE message_id = %s ORDER BY created_at",
        (message_id,)
    )
    return msg


def get_media_by_id(media_id: int):
    return query("SELECT * FROM media WHERE media_id = %s", (media_id,), one=True)


def list_users(limit=100):
    return query("""
        SELECT u.*,
               (SELECT COUNT(*) FROM messages m WHERE m.user_id = u.user_id) AS message_count
        FROM users u
        ORDER BY message_count DESC
        LIMIT %s
    """, (limit,))


def list_chats():
    return query("""
        SELECT chat_id,
               MAX(chat_title) AS chat_title,
               MAX(chat_type) AS chat_type,
               COUNT(*) AS message_count
        FROM messages
        GROUP BY chat_id
        ORDER BY message_count DESC
    """)


def list_message_types():
    return query("""
        SELECT message_type, COUNT(*) AS c
        FROM messages
        WHERE message_type IS NOT NULL
        GROUP BY message_type
        ORDER BY c DESC
    """)


def stats_messages_per_day(days: int = 30):
    return query("""
        SELECT DATE(created_at) AS day, COUNT(*) AS c
        FROM messages
        WHERE created_at >= DATE_SUB(NOW(), INTERVAL %s DAY)
        GROUP BY DATE(created_at)
        ORDER BY day
    """, (days,))


def stats_overview():
    rows = {}
    rows['total_messages'] = query("SELECT COUNT(*) AS c FROM messages", one=True)['c']
    rows['total_users'] = query("SELECT COUNT(*) AS c FROM users", one=True)['c']
    rows['total_media'] = query("SELECT COUNT(*) AS c FROM media", one=True)['c']
    rows['total_links'] = query("SELECT COUNT(*) AS c FROM links", one=True)['c']
    rows['total_corrections'] = query("SELECT COUNT(*) AS c FROM spelling_corrections", one=True)['c']
    return rows


def top_users(limit: int = 10):
    return query("""
        SELECT m.user_id,
               COALESCE(u.first_name, MAX(m.user_first_name)) AS first_name,
               COALESCE(u.username, MAX(m.user_username)) AS username,
               COUNT(*) AS message_count
        FROM messages m
        LEFT JOIN users u ON u.user_id = m.user_id
        WHERE m.user_id IS NOT NULL
        GROUP BY m.user_id, u.first_name, u.username
        ORDER BY message_count DESC
        LIMIT %s
    """, (limit,))


def list_corrections(limit=50, offset=0):
    return query("""
        SELECT sc.*, m.user_first_name, m.user_username
        FROM spelling_corrections sc
        LEFT JOIN messages m ON m.message_id = sc.message_id
        ORDER BY sc.created_at DESC
        LIMIT %s OFFSET %s
    """, (limit, offset))


def delete_message(message_id: int) -> int:
    return execute("DELETE FROM messages WHERE message_id = %s", (message_id,))
