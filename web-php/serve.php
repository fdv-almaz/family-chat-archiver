<?php
/**
 * Запуск встроенного PHP-сервера с учётом WEB_HOST/WEB_PORT из .env
 * (аналог uvicorn.run в web/app.py, включая предупреждение о небезопасном биндинге).
 *
 *   php serve.php
 */

declare(strict_types=1);

require __DIR__ . '/src/Config.php';
Config::load();

$host = Config::$WEB_HOST;
$port = Config::$WEB_PORT;

fwrite(STDERR, sprintf("Family Chat Archiver — Web (PHP) v%s\n", Config::$VERSION));
fwrite(STDERR, sprintf("Binding to http://%s:%d\n", $host, $port));

// У интерфейса нет встроенной авторизации — предупреждаем при выходе за loopback.
if (!in_array($host, ['127.0.0.1', 'localhost', '::1'], true)) {
    fwrite(STDERR, sprintf(
        "WARNING: WEB_HOST=%s is not loopback — the interface has NO authentication. " .
        "Put it behind a reverse-proxy (nginx) with auth + HTTPS, or restrict access " .
        "at the firewall level.\n",
        $host
    ));
}

$docroot = __DIR__ . '/public';
$cmd = sprintf(
    'php -S %s:%d -t %s %s',
    $host,
    $port,
    escapeshellarg($docroot),
    escapeshellarg($docroot . '/index.php')
);
passthru($cmd, $exitCode);
exit($exitCode);
