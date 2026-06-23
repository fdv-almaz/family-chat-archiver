<?php
/**
 * Конфигурация веб-интерфейса (PHP-копия web/config.py).
 *
 * Значения читаются из переменных окружения и файла .env в корне web-php/.
 * Поведение зеркалит Python-версию: те же ключи, те же значения по умолчанию.
 */

declare(strict_types=1);

final class Config
{
    public static string $MYSQL_HOST = 'localhost';
    public static int $MYSQL_PORT = 3306;
    public static string $MYSQL_USER = 'root';
    public static string $MYSQL_PASSWORD = '';
    public static string $MYSQL_DATABASE = 'family_chat';

    public static string $TELEGRAM_BOT_TOKEN = '';

    public static string $WEB_HOST = '127.0.0.1';
    public static int $WEB_PORT = 8000;

    /** @var string[] Доп. хосты Origin/Referer для POST/DELETE (CSRF). */
    public static array $ALLOWED_ORIGINS = [];

    public static string $MEDIA_CACHE_DIR = '';

    /** @var string[] Whitelist каталогов, из которых /media/{id} может отдавать файлы. */
    public static array $ALLOWED_MEDIA_DIRS = [];

    public static string $VERSION = 'unknown';

    private static bool $loaded = false;

    public static function load(): void
    {
        if (self::$loaded) {
            return;
        }
        self::$loaded = true;

        self::loadDotEnv(__DIR__ . '/../.env');

        self::$MYSQL_HOST = self::env('MYSQL_HOST', 'localhost');
        self::$MYSQL_PORT = (int) self::env('MYSQL_PORT', '3306');
        self::$MYSQL_USER = self::env('MYSQL_USER', 'root');
        self::$MYSQL_PASSWORD = self::env('MYSQL_PASSWORD', '');
        self::$MYSQL_DATABASE = self::env('MYSQL_DATABASE', 'family_chat');

        self::$TELEGRAM_BOT_TOKEN = self::env('TELEGRAM_BOT_TOKEN', '');

        self::$WEB_HOST = self::env('WEB_HOST', '127.0.0.1');
        self::$WEB_PORT = (int) self::env('WEB_PORT', '8000');

        self::$ALLOWED_ORIGINS = array_values(array_filter(array_map(
            'trim',
            explode(',', self::env('ALLOWED_ORIGINS', ''))
        )));

        // Кеш скачанных из Telegram файлов (относительно web-php/).
        $cache = self::env('MEDIA_CACHE_DIR', 'media_cache');
        if (!self::isAbsolute($cache)) {
            $cache = __DIR__ . '/../' . $cache;
        }
        if (!is_dir($cache)) {
            @mkdir($cache, 0775, true);
        }
        self::$MEDIA_CACHE_DIR = self::normalizePath($cache);

        // Whitelist путей для /media/{id} (защита от path traversal).
        $defaultAllowed = implode(',', [
            self::$MEDIA_CACHE_DIR,
            self::normalizePath(__DIR__ . '/../../python/storage'),
            self::normalizePath(__DIR__ . '/../../rust/storage'),
        ]);
        $allowed = self::env('ALLOWED_MEDIA_DIRS', $defaultAllowed);
        self::$ALLOWED_MEDIA_DIRS = array_values(array_filter(array_map(
            fn(string $p): string => self::normalizePath(trim($p)),
            explode(',', $allowed)
        )));

        // Версия проекта из VERSION в корне репозитория.
        $versionFile = __DIR__ . '/../../VERSION';
        if (is_readable($versionFile)) {
            self::$VERSION = trim((string) file_get_contents($versionFile));
        }
    }

    private static function env(string $key, string $default): string
    {
        $v = getenv($key);
        if ($v === false || $v === '') {
            return $default;
        }
        return $v;
    }

    /** Минимальный парсер .env (KEY=VALUE, # — комментарии, кавычки снимаются). */
    private static function loadDotEnv(string $path): void
    {
        if (!is_readable($path)) {
            return;
        }
        foreach (file($path, FILE_IGNORE_NEW_LINES | FILE_SKIP_EMPTY_LINES) as $line) {
            $line = trim($line);
            if ($line === '' || $line[0] === '#') {
                continue;
            }
            $pos = strpos($line, '=');
            if ($pos === false) {
                continue;
            }
            $key = trim(substr($line, 0, $pos));
            $val = trim(substr($line, $pos + 1));
            if (strlen($val) >= 2 && ($val[0] === '"' || $val[0] === "'") && $val[-1] === $val[0]) {
                $val = substr($val, 1, -1);
            }
            // Не перетираем уже заданное окружение процесса.
            if (getenv($key) === false) {
                putenv("$key=$val");
            }
        }
    }

    private static function isAbsolute(string $path): bool
    {
        return $path !== '' && ($path[0] === '/' || preg_match('#^[A-Za-z]:[\\\\/]#', $path) === 1);
    }

    /** Нормализует путь (убирает . и ..), не требуя существования файла. */
    public static function normalizePath(string $path): string
    {
        $real = realpath($path);
        if ($real !== false) {
            return $real;
        }
        // Файла нет — нормализуем вручную.
        $isAbs = self::isAbsolute($path);
        $parts = preg_split('#[\\\\/]+#', $path);
        $stack = [];
        foreach ($parts as $part) {
            if ($part === '' || $part === '.') {
                continue;
            }
            if ($part === '..') {
                array_pop($stack);
            } else {
                $stack[] = $part;
            }
        }
        return ($isAbs ? '/' : '') . implode('/', $stack);
    }
}
