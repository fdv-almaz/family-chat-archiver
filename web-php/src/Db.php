<?php
/**
 * Доступ к MySQL (PHP-копия web/db.py).
 *
 * Все пользовательские параметры передаются через подготовленные выражения
 * (защита от SQL-инъекций). LIMIT/OFFSET — всегда валидированные int,
 * подставляются как (int) inline (PDO не биндит их при отключённой эмуляции).
 */

declare(strict_types=1);

final class Db
{
    private static ?PDO $pdo = null;

    public static function pdo(): PDO
    {
        if (self::$pdo === null) {
            $dsn = sprintf(
                'mysql:host=%s;port=%d;dbname=%s;charset=utf8mb4',
                Config::$MYSQL_HOST,
                Config::$MYSQL_PORT,
                Config::$MYSQL_DATABASE
            );
            self::$pdo = new PDO($dsn, Config::$MYSQL_USER, Config::$MYSQL_PASSWORD, [
                PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION,
                PDO::ATTR_DEFAULT_FETCH_MODE => PDO::FETCH_ASSOC,
                PDO::ATTR_EMULATE_PREPARES => false,
            ]);
        }
        return self::$pdo;
    }

    /**
     * Идемпотентные миграции, от которых зависит веб (аналог _ensure_columns).
     * Запускается при старте; ошибки «колонка уже есть» молча игнорируются.
     */
    public static function ensureColumns(): void
    {
        $pdo = self::pdo();
        $alters = [
            'ALTER TABLE messages ADD COLUMN deleted_at TIMESTAMP NULL',
            'ALTER TABLE media ADD COLUMN local_path VARCHAR(500)',
        ];
        foreach ($alters as $sql) {
            try {
                $pdo->exec($sql);
            } catch (PDOException $e) {
                // колонка уже существует — пропускаем
            }
        }
        // daily_tips создаётся ботом; на случай свежей БД создаём здесь.
        try {
            $pdo->exec(
                'CREATE TABLE IF NOT EXISTS daily_tips (
                    tip_id INT AUTO_INCREMENT PRIMARY KEY,
                    chat_id BIGINT,
                    model VARCHAR(64),
                    prompt LONGTEXT,
                    response LONGTEXT,
                    sent_to_chat BOOLEAN DEFAULT FALSE,
                    error LONGTEXT,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    INDEX idx_chat_date (chat_id, created_at)
                ) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci'
            );
        } catch (PDOException $e) {
            // игнорируем
        }
    }

    /** @return array<int,array<string,mixed>>|array<string,mixed>|null */
    public static function query(string $sql, array $params = [], bool $one = false)
    {
        $stmt = self::pdo()->prepare($sql);
        $stmt->execute($params);
        $rows = $stmt->fetchAll();
        if ($one) {
            return $rows[0] ?? null;
        }
        return $rows;
    }

    public static function execute(string $sql, array $params = []): int
    {
        $stmt = self::pdo()->prepare($sql);
        $stmt->execute($params);
        return $stmt->rowCount();
    }

    // ---- Сообщения --------------------------------------------------------

    public static function listMessages(
        ?int $chatId = null,
        ?int $userId = null,
        ?string $messageType = null,
        ?string $dateFrom = null,
        ?string $dateTo = null,
        ?string $search = null,
        int $limit = 50,
        int $offset = 0
    ): array {
        $where = [];
        $params = [];

        if ($chatId !== null) { $where[] = 'm.chat_id = ?'; $params[] = $chatId; }
        if ($userId !== null) { $where[] = 'm.user_id = ?'; $params[] = $userId; }
        if ($messageType) { $where[] = 'm.message_type = ?'; $params[] = $messageType; }
        if ($dateFrom) { $where[] = 'm.created_at >= ?'; $params[] = $dateFrom; }
        if ($dateTo) { $where[] = 'm.created_at <= ?'; $params[] = $dateTo; }
        if ($search) { $where[] = 'm.text LIKE ?'; $params[] = '%' . $search . '%'; }

        $whereSql = $where ? ('WHERE ' . implode(' AND ', $where)) : '';
        $limit = max(0, $limit);
        $offset = max(0, $offset);

        $sql = "
            SELECT m.*,
                   COALESCE(u.first_name, m.user_first_name) AS display_first_name,
                   COALESCE(u.username, m.user_username) AS display_username
            FROM messages m
            LEFT JOIN users u ON u.user_id = m.user_id
            $whereSql
            ORDER BY m.created_at DESC
            LIMIT $limit OFFSET $offset
        ";
        $messages = self::query($sql, $params);

        $msgIds = array_column($messages, 'message_id');
        if ($msgIds) {
            $placeholders = implode(',', array_fill(0, count($msgIds), '?'));

            $mediaRows = self::query(
                "SELECT * FROM media WHERE message_id IN ($placeholders) ORDER BY media_id",
                $msgIds
            );
            $mediaByMsg = [];
            foreach ($mediaRows as $m) {
                $mediaByMsg[$m['message_id']][] = $m;
            }

            $corrRows = self::query(
                "SELECT message_id, errors FROM spelling_corrections
                 WHERE message_id IN ($placeholders) ORDER BY created_at",
                $msgIds
            );
            $hintsByMsg = [];
            foreach ($corrRows as $c) {
                foreach (self::correctionPairs($c['errors'] ?? null) as $pair) {
                    if (!isset($hintsByMsg[$c['message_id']])) {
                        $hintsByMsg[$c['message_id']] = [];
                    }
                    if (!in_array($pair, $hintsByMsg[$c['message_id']], true)) {
                        $hintsByMsg[$c['message_id']][] = $pair;
                    }
                }
            }

            foreach ($messages as &$msg) {
                $msg['media_list'] = $mediaByMsg[$msg['message_id']] ?? [];
                $pairs = $hintsByMsg[$msg['message_id']] ?? null;
                $msg['spelling_hint'] = $pairs ? implode('; ', $pairs) : null;
            }
            unset($msg);
        }

        return $messages;
    }

    /** Разбирает spelling_corrections.errors в ['ашибка → ошибка', ...]. */
    private static function correctionPairs($errors): array
    {
        if (is_string($errors)) {
            $decoded = json_decode($errors, true);
            if ($decoded === null && json_last_error() !== JSON_ERROR_NONE) {
                return [];
            }
            $errors = $decoded;
        }
        if (!is_array($errors)) {
            return [];
        }
        $pairs = [];
        foreach ($errors as $err) {
            if (!is_array($err)) {
                continue;
            }
            $original = trim((string) ($err['original'] ?? ''));
            $suggested = trim((string) ($err['suggested'] ?? ''));
            if ($original !== '' && $suggested !== '') {
                $pairs[] = "$original → $suggested";
            }
        }
        return $pairs;
    }

    public static function countMessages(
        ?int $chatId = null,
        ?int $userId = null,
        ?string $messageType = null,
        ?string $dateFrom = null,
        ?string $dateTo = null,
        ?string $search = null
    ): int {
        $where = [];
        $params = [];
        if ($chatId !== null) { $where[] = 'chat_id = ?'; $params[] = $chatId; }
        if ($userId !== null) { $where[] = 'user_id = ?'; $params[] = $userId; }
        if ($messageType) { $where[] = 'message_type = ?'; $params[] = $messageType; }
        if ($dateFrom) { $where[] = 'created_at >= ?'; $params[] = $dateFrom; }
        if ($dateTo) { $where[] = 'created_at <= ?'; $params[] = $dateTo; }
        if ($search) { $where[] = 'text LIKE ?'; $params[] = '%' . $search . '%'; }

        $whereSql = $where ? ('WHERE ' . implode(' AND ', $where)) : '';
        $row = self::query("SELECT COUNT(*) AS c FROM messages $whereSql", $params, true);
        return $row ? (int) $row['c'] : 0;
    }

    public static function getMessage(int $messageId): ?array
    {
        $msg = self::query(
            "SELECT m.*,
                    COALESCE(u.first_name, m.user_first_name) AS display_first_name,
                    COALESCE(u.username, m.user_username) AS display_username
             FROM messages m
             LEFT JOIN users u ON u.user_id = m.user_id
             WHERE m.message_id = ?",
            [$messageId],
            true
        );
        if (!$msg) {
            return null;
        }
        $msg['media'] = self::query('SELECT * FROM media WHERE message_id = ?', [$messageId]);
        $msg['links'] = self::query('SELECT * FROM links WHERE message_id = ?', [$messageId]);
        $msg['corrections'] = self::query(
            'SELECT * FROM spelling_corrections WHERE message_id = ? ORDER BY created_at',
            [$messageId]
        );
        return $msg;
    }

    public static function getMediaById(int $mediaId): ?array
    {
        return self::query('SELECT * FROM media WHERE media_id = ?', [$mediaId], true);
    }

    // ---- Пользователи / чаты / типы --------------------------------------

    public static function listUsers(int $limit = 100): array
    {
        $limit = max(0, $limit);
        return self::query(
            "SELECT u.*,
                    (SELECT COUNT(*) FROM messages m WHERE m.user_id = u.user_id) AS message_count
             FROM users u
             ORDER BY message_count DESC
             LIMIT $limit"
        );
    }

    public static function listChats(): array
    {
        return self::query("
            SELECT chat_id,
                   COALESCE(
                       NULLIF(MAX(chat_title), ''),
                       NULLIF(TRIM(MAX(CASE WHEN chat_type = 'private' THEN
                           CONCAT(COALESCE(user_first_name, ''), ' ', COALESCE(user_last_name, ''))
                       END)), ''),
                       CASE
                           WHEN MAX(chat_type) = 'private' THEN CONCAT('Личный чат #', chat_id)
                           WHEN MAX(chat_type) = 'group' THEN CONCAT('Группа #', chat_id)
                           WHEN MAX(chat_type) IN ('supergroup', 'channel') THEN CONCAT(MAX(chat_type), ' #', chat_id)
                           ELSE CONCAT('Чат #', chat_id)
                       END
                   ) AS chat_title,
                   MAX(chat_type) AS chat_type,
                   COUNT(*) AS message_count
            FROM messages
            GROUP BY chat_id
            ORDER BY message_count DESC
        ");
    }

    public static function listMessageTypes(): array
    {
        return self::query("
            SELECT message_type, COUNT(*) AS c
            FROM messages
            WHERE message_type IS NOT NULL
            GROUP BY message_type
            ORDER BY c DESC
        ");
    }

    // ---- Статистика -------------------------------------------------------

    public static function statsMessagesPerDay(int $days = 30): array
    {
        return self::query(
            'SELECT DATE(created_at) AS day, COUNT(*) AS c
             FROM messages
             WHERE created_at >= DATE_SUB(NOW(), INTERVAL ? DAY)
             GROUP BY DATE(created_at)
             ORDER BY day',
            [$days]
        );
    }

    public static function statsOverview(): array
    {
        return [
            'total_messages' => (int) self::query('SELECT COUNT(*) AS c FROM messages', [], true)['c'],
            'total_users' => (int) self::query('SELECT COUNT(*) AS c FROM users', [], true)['c'],
            'total_media' => (int) self::query('SELECT COUNT(*) AS c FROM media', [], true)['c'],
            'total_links' => (int) self::query('SELECT COUNT(*) AS c FROM links', [], true)['c'],
            'total_corrections' => (int) self::query('SELECT COUNT(*) AS c FROM spelling_corrections', [], true)['c'],
        ];
    }

    public static function topUsers(int $limit = 10): array
    {
        $limit = max(0, $limit);
        return self::query("
            SELECT m.user_id,
                   COALESCE(u.first_name, MAX(m.user_first_name)) AS first_name,
                   COALESCE(u.username, MAX(m.user_username)) AS username,
                   COUNT(*) AS message_count
            FROM messages m
            LEFT JOIN users u ON u.user_id = m.user_id
            WHERE m.user_id IS NOT NULL
            GROUP BY m.user_id, u.first_name, u.username
            ORDER BY message_count DESC
            LIMIT $limit
        ");
    }

    // ---- Орфография / советы дня -----------------------------------------

    public static function listCorrections(int $limit = 50, int $offset = 0): array
    {
        $limit = max(0, $limit);
        $offset = max(0, $offset);
        return self::query("
            SELECT sc.*, m.user_first_name, m.user_username
            FROM spelling_corrections sc
            LEFT JOIN messages m ON m.message_id = sc.message_id
            ORDER BY sc.created_at DESC
            LIMIT $limit OFFSET $offset
        ");
    }

    public static function listDailyTips(int $limit = 50, int $offset = 0): array
    {
        $limit = max(0, $limit);
        $offset = max(0, $offset);
        return self::query("
            SELECT dt.*,
                   (SELECT MAX(m.chat_title) FROM messages m WHERE m.chat_id = dt.chat_id) AS chat_title
            FROM daily_tips dt
            ORDER BY dt.created_at DESC
            LIMIT $limit OFFSET $offset
        ");
    }

    public static function countDailyTips(): int
    {
        $row = self::query('SELECT COUNT(*) AS c FROM daily_tips', [], true);
        return $row ? (int) $row['c'] : 0;
    }

    // ---- Soft-delete ------------------------------------------------------

    public static function softDeleteMessage(int $messageId): int
    {
        return self::execute(
            'UPDATE messages SET deleted_at = NOW() WHERE message_id = ? AND deleted_at IS NULL',
            [$messageId]
        );
    }

    public static function restoreMessage(int $messageId): int
    {
        return self::execute('UPDATE messages SET deleted_at = NULL WHERE message_id = ?', [$messageId]);
    }

    public static function hardDeleteMessage(int $messageId): int
    {
        return self::execute('DELETE FROM messages WHERE message_id = ?', [$messageId]);
    }
}
