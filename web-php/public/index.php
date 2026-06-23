<?php
/**
 * Фронт-контроллер веб-интерфейса (PHP-копия web/app.py).
 *
 * Работает:
 *   - как router для встроенного сервера: php -S host:port -t public public/index.php
 *   - под Apache/nginx: rewrite всех запросов на index.php
 */

declare(strict_types=1);

// Для встроенного php-сервера: существующие файлы (static) отдаём как есть.
if (PHP_SAPI === 'cli-server') {
    $urlPath = parse_url($_SERVER['REQUEST_URI'], PHP_URL_PATH);
    $file = __DIR__ . $urlPath;
    if ($urlPath !== '/' && is_file($file)) {
        return false;
    }
}

require __DIR__ . '/../src/Config.php';
require __DIR__ . '/../src/helpers.php';
require __DIR__ . '/../src/Db.php';
require __DIR__ . '/../src/Telegram.php';

// Не светим трейсы и пути клиенту — пишем в лог, отдаём общий 500.
ini_set('display_errors', '0');
set_exception_handler(function (Throwable $ex): void {
    error_log('Unhandled error: ' . $ex->getMessage() . ' @ ' . $ex->getFile() . ':' . $ex->getLine());
    if (!headers_sent()) {
        http_response_code(500);
        header('Content-Type: text/plain; charset=utf-8');
    }
    echo 'Internal Server Error';
});

Config::load();
Db::ensureColumns();

// Security-заголовки для всех ответов (defense-in-depth).
// CSP строгая: только собственные ресурсы, без inline-скриптов и фреймов.
// Inline-скрипт графика и inline-обработчик confirm вынесены в app.js
// (данные передаются через data-атрибуты), поэтому 'unsafe-inline' не нужен.
header('X-Content-Type-Options: nosniff');
header('X-Frame-Options: DENY');
header('Referrer-Policy: same-origin');
header(
    "Content-Security-Policy: default-src 'self'; img-src 'self' data:; " .
    "media-src 'self'; style-src 'self'; script-src 'self'; object-src 'none'; " .
    "base-uri 'none'; frame-ancestors 'none'; form-action 'self'"
);

$method = $_SERVER['REQUEST_METHOD'] ?? 'GET';
$path = parse_url($_SERVER['REQUEST_URI'] ?? '/', PHP_URL_PATH) ?? '/';

// --- CSRF: same-origin guard для методов, меняющих состояние ---------------
if (in_array($method, ['POST', 'PUT', 'PATCH', 'DELETE'], true)) {
    $host = $_SERVER['HTTP_HOST'] ?? '';
    $origin = $_SERVER['HTTP_ORIGIN'] ?? ($_SERVER['HTTP_REFERER'] ?? '');
    if ($origin !== '') {
        $netloc = parse_url($origin, PHP_URL_HOST) ?? '';
        $port = parse_url($origin, PHP_URL_PORT);
        if ($port) {
            $netloc .= ':' . $port;
        }
        $allowed = array_merge([$host], Config::$ALLOWED_ORIGINS);
        if (!in_array($netloc, $allowed, true)) {
            http_response_code(403);
            header('Content-Type: application/json; charset=utf-8');
            echo json_encode(['detail' => 'Cross-origin request blocked']);
            exit;
        }
    }
}

const DEFAULT_PAGE_SIZE = 100;

/** @return int[] 25, 50, ..., 1000 */
function allowed_page_sizes(): array
{
    return range(25, 1000, 25);
}

/** Пустая строка / null → null, иначе int. */
function to_int_or_null($v): ?int
{
    if ($v === null || $v === '') {
        return null;
    }
    return is_numeric($v) ? (int) $v : null;
}

// --- Маршрутизация ---------------------------------------------------------

if ($path === '/' && $method === 'GET') {
    route_index();
} elseif (preg_match('#^/message/(\d+)$#', $path, $m) && $method === 'GET') {
    route_view_message((int) $m[1]);
} elseif (preg_match('#^/message/(\d+)/delete$#', $path, $m) && $method === 'POST') {
    if (Db::softDeleteMessage((int) $m[1]) === 0) {
        abort(404, 'Message not found or already deleted');
    }
    redirect('/message/' . (int) $m[1], 303);
} elseif (preg_match('#^/message/(\d+)/restore$#', $path, $m) && $method === 'POST') {
    if (Db::restoreMessage((int) $m[1]) === 0) {
        abort(404, 'Message not found');
    }
    redirect('/message/' . (int) $m[1], 303);
} elseif (preg_match('#^/message/(\d+)/hard-delete$#', $path, $m) && $method === 'POST') {
    if (Db::hardDeleteMessage((int) $m[1]) === 0) {
        abort(404, 'Message not found');
    }
    redirect('/', 303);
} elseif ($path === '/users' && $method === 'GET') {
    render_page('users.php', ['users' => Db::listUsers()]);
} elseif ($path === '/stats' && $method === 'GET') {
    render_page('stats.php', [
        'overview' => Db::statsOverview(),
        'messages_per_day' => Db::statsMessagesPerDay(30),
        'top_users' => Db::topUsers(10),
        'message_types' => Db::listMessageTypes(),
    ]);
} elseif ($path === '/corrections' && $method === 'GET') {
    $page = max(1, (int) ($_GET['page'] ?? 1));
    $offset = ($page - 1) * DEFAULT_PAGE_SIZE;
    render_page('corrections.php', [
        'corrections' => Db::listCorrections(DEFAULT_PAGE_SIZE, $offset),
        'page' => $page,
    ]);
} elseif ($path === '/tips' && $method === 'GET') {
    $page = max(1, (int) ($_GET['page'] ?? 1));
    $offset = ($page - 1) * DEFAULT_PAGE_SIZE;
    $total = Db::countDailyTips();
    render_page('tips.php', [
        'tips' => Db::listDailyTips(DEFAULT_PAGE_SIZE, $offset),
        'total' => $total,
        'page' => $page,
        'total_pages' => (int) (($total + DEFAULT_PAGE_SIZE - 1) / DEFAULT_PAGE_SIZE),
    ]);
} elseif (preg_match('#^/media/(\d+)$#', $path, $m) && $method === 'GET') {
    route_media((int) $m[1]);
} elseif ($path === '/api/stats/per-day' && $method === 'GET') {
    $days = (int) ($_GET['days'] ?? 30);
    $out = array_map(
        fn(array $r): array => ['day' => (string) $r['day'], 'count' => (int) $r['c']],
        Db::statsMessagesPerDay($days)
    );
    header('Content-Type: application/json; charset=utf-8');
    echo json_encode($out);
} else {
    abort(404, 'Not Found');
}

// --- Контроллеры -----------------------------------------------------------

function route_index(): void
{
    $chatId = to_int_or_null($_GET['chat_id'] ?? null);
    $userId = to_int_or_null($_GET['user_id'] ?? null);
    $messageType = ($_GET['message_type'] ?? '') ?: null;
    $dateFrom = ($_GET['date_from'] ?? '') ?: null;
    $dateTo = ($_GET['date_to'] ?? '') ?: null;
    // DoS-защита: ограничение длины поиска 200 символов (как max_length в Python).
    $q = isset($_GET['q']) && $_GET['q'] !== '' ? mb_substr((string) $_GET['q'], 0, 200, 'UTF-8') : null;

    $page = max(1, (int) ($_GET['page'] ?? 1));
    $pageSize = (int) ($_GET['page_size'] ?? DEFAULT_PAGE_SIZE);
    if (!in_array($pageSize, allowed_page_sizes(), true)) {
        $pageSize = DEFAULT_PAGE_SIZE;
    }
    $offset = ($page - 1) * $pageSize;

    $messages = Db::listMessages($chatId, $userId, $messageType, $dateFrom, $dateTo, $q, $pageSize, $offset);
    $total = Db::countMessages($chatId, $userId, $messageType, $dateFrom, $dateTo, $q);

    render_page('index.php', [
        'messages' => $messages,
        'total' => $total,
        'page' => $page,
        'page_size' => $pageSize,
        'page_size_options' => allowed_page_sizes(),
        'total_pages' => (int) (($total + $pageSize - 1) / $pageSize),
        'chats' => Db::listChats(),
        'message_types' => Db::listMessageTypes(),
        'filters' => [
            'chat_id' => $chatId,
            'user_id' => $userId,
            'message_type' => $messageType,
            'date_from' => $dateFrom,
            'date_to' => $dateTo,
            'q' => $q,
            'page_size' => $pageSize !== DEFAULT_PAGE_SIZE ? $pageSize : null,
        ],
    ]);
}

function route_view_message(int $messageId): void
{
    $msg = Db::getMessage($messageId);
    if (!$msg) {
        abort(404, 'Message not found');
    }
    render_page('message.php', ['msg' => $msg]);
}

/** Отдаёт медиа: local_path бота → веб-кеш → скачивание из Telegram. */
function route_media(int $mediaId): void
{
    $media = Db::getMediaById($mediaId);
    if (!$media) {
        abort(404, 'Media not found');
    }

    // 1. Локальный файл, сохранённый ботом (приоритет). Проверка whitelist
    //    (защита от path traversal, если local_path подменили в БД).
    $localPath = $media['local_path'] ?? null;
    if ($localPath) {
        $absLocal = Config::normalizePath($localPath);
        $inWhitelist = false;
        foreach (Config::$ALLOWED_MEDIA_DIRS as $dir) {
            if ($absLocal === $dir || str_starts_with($absLocal, $dir . DIRECTORY_SEPARATOR)) {
                $inWhitelist = true;
                break;
            }
        }
        if (!$inWhitelist) {
            error_log("Refusing to serve media $mediaId: path $absLocal outside ALLOWED_MEDIA_DIRS");
            abort(403, 'Media path not allowed');
        }
        if (is_file($absLocal)) {
            $mime = $media['mime_type'] ?? Telegram::mimeFromExt($absLocal);
            stream_file($absLocal, $mime);
            return;
        }
    }

    // 2. Fallback: скачать из Telegram (и закешировать в media_cache).
    $fileId = $media['file_id'] ?? null;
    $fileUniqueId = $media['file_unique_id'] ?? null;
    if (!$fileId || !$fileUniqueId) {
        abort(404, 'No file stored or referenced');
    }

    $suggestedExt = '';
    if (!empty($media['file_name'])) {
        $ext = pathinfo((string) $media['file_name'], PATHINFO_EXTENSION);
        $suggestedExt = $ext !== '' ? '.' . $ext : '';
    }

    $path = Telegram::fetchMedia((string) $fileId, (string) $fileUniqueId, $suggestedExt);
    if (!$path) {
        abort(502, 'Failed to fetch media from Telegram (file may have expired)');
    }
    $mime = $media['mime_type'] ?? Telegram::mimeFromExt($path);
    stream_file($path, $mime);
}

function stream_file(string $path, string $mime): void
{
    $size = (int) filesize($path);

    // Защита от MIME-sniffing XSS: запрещаем браузеру «угадывать» тип и
    // отдаём inline только заведомо безопасные для воспроизведения типы.
    // Остальное (в т.ч. SVG, который может нести скрипт) — только как вложение,
    // чтобы исключить исполнение HTML/JS в нашем origin.
    header('X-Content-Type-Options: nosniff');
    $inline = $mime !== 'image/svg+xml'
        && (preg_match('#^(image|audio|video)/#', $mime) === 1 || $mime === 'application/pdf');
    header('Content-Disposition: ' . ($inline ? 'inline' : 'attachment'));
    header('Content-Type: ' . $mime);
    header('Accept-Ranges: bytes');

    // Поддержка Range-запросов (перемотка аудио/видео), как в FastAPI FileResponse.
    $start = 0;
    $end = $size - 1;
    $range = $_SERVER['HTTP_RANGE'] ?? '';
    if ($range !== '' && preg_match('/^bytes=(\d*)-(\d*)$/', $range, $mm)) {
        if ($mm[1] !== '') {
            $start = (int) $mm[1];
        }
        if ($mm[2] !== '') {
            $end = (int) $mm[2];
        }
        if ($size === 0 || $start > $end || $start >= $size) {
            http_response_code(416);
            header("Content-Range: bytes */$size");
            return;
        }
        $end = min($end, $size - 1);
        http_response_code(206);
        header("Content-Range: bytes $start-$end/$size");
    }

    $length = $end - $start + 1;
    header('Content-Length: ' . $length);

    $fp = fopen($path, 'rb');
    if ($fp === false) {
        return;
    }
    if ($start > 0) {
        fseek($fp, $start);
    }
    $remaining = $length;
    while ($remaining > 0 && !feof($fp)) {
        $chunk = fread($fp, (int) min(8192, $remaining));
        if ($chunk === false) {
            break;
        }
        echo $chunk;
        $remaining -= strlen($chunk);
    }
    fclose($fp);
}

function redirect(string $location, int $code = 303): void
{
    http_response_code($code);
    header('Location: ' . $location);
    exit;
}
