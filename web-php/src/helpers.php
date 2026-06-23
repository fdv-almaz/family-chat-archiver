<?php
/**
 * Вспомогательные функции рендеринга и экранирования (PHP-копия логики Jinja).
 *
 * `e()` обязателен для любого вывода данных из БД — это защита от XSS
 * (аналог Jinja autoescape=True в Python-версии).
 */

declare(strict_types=1);

/** HTML-экранирование (autoescape). */
function e(?string $s): string
{
    return htmlspecialchars($s ?? '', ENT_QUOTES | ENT_SUBSTITUTE, 'UTF-8');
}

/** Форматирование даты из БД ('YYYY-MM-DD HH:MM:SS') заданным форматом date(). */
function fdate(?string $val, string $format = 'Y-m-d H:i'): string
{
    if ($val === null || $val === '') {
        return '';
    }
    $ts = strtotime($val);
    return $ts !== false ? date($format, $ts) : (string) $val;
}

/** Обрезка строки по числу символов (UTF-8-безопасно), с многоточием. */
function truncate(?string $s, int $limit): string
{
    $s = $s ?? '';
    if (mb_strlen($s, 'UTF-8') <= $limit) {
        return e($s);
    }
    return e(mb_substr($s, 0, $limit, 'UTF-8')) . '…';
}

/** Строит query-string из массива, пропуская пустые значения. */
function qs(array $params): string
{
    $parts = [];
    foreach ($params as $k => $v) {
        if ($v === null || $v === '' || $v === false) {
            continue;
        }
        $parts[] = rawurlencode((string) $k) . '=' . rawurlencode((string) $v);
    }
    return implode('&', $parts);
}

/**
 * Рендерит шаблон внутри layout (base.php).
 * Дочерний шаблон может задать $page_title; он виден в base.php (общая область видимости).
 */
function render_page(string $template, array $vars = []): void
{
    $vars['VERSION'] = Config::$VERSION;
    extract($vars, EXTR_OVERWRITE);

    ob_start();
    include __DIR__ . '/../templates/' . $template;
    $content = ob_get_clean();

    include __DIR__ . '/../templates/base.php';
}

/** Отправляет HTTP-ошибку с простым телом и завершает выполнение. */
function abort(int $code, string $message): void
{
    http_response_code($code);
    header('Content-Type: text/plain; charset=utf-8');
    echo $message;
    exit;
}
