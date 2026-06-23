<?php
/**
 * Скачивание медиа из Telegram по запросу с локальным кешем
 * (PHP-копия web/telegram.py). Использует cURL.
 */

declare(strict_types=1);

final class Telegram
{
    private const API = 'https://api.telegram.org';

    /** Резолвит file_id в относительный путь через getFile. */
    private static function getFilePath(string $fileId): ?string
    {
        $url = self::API . '/bot' . Config::$TELEGRAM_BOT_TOKEN . '/getFile';
        $ch = curl_init();
        curl_setopt_array($ch, [
            CURLOPT_URL => $url . '?' . http_build_query(['file_id' => $fileId]),
            CURLOPT_RETURNTRANSFER => true,
            CURLOPT_TIMEOUT => 15,
        ]);
        $body = curl_exec($ch);
        $status = curl_getinfo($ch, CURLINFO_HTTP_CODE);
        curl_close($ch);

        if ($body === false || $status !== 200) {
            error_log("getFile failed: $status");
            return null;
        }
        $data = json_decode((string) $body, true);
        if (!is_array($data) || empty($data['ok'])) {
            error_log('getFile not ok');
            return null;
        }
        return $data['result']['file_path'] ?? null;
    }

    /**
     * Возвращает абсолютный путь к кешированному файлу, скачивая из Telegram
     * при отсутствии в кеше. null — при ошибке.
     */
    public static function fetchMedia(string $fileId, string $fileUniqueId, string $suggestedExt = ''): ?string
    {
        if (Config::$TELEGRAM_BOT_TOKEN === '') {
            return null;
        }

        $cacheDir = Config::$MEDIA_CACHE_DIR;
        // Уже в кеше? (ищем по префиксу file_unique_id)
        foreach (scandir($cacheDir) ?: [] as $fname) {
            if ($fname !== '.' && $fname !== '..' && str_starts_with($fname, $fileUniqueId)) {
                return $cacheDir . '/' . $fname;
            }
        }

        $filePath = self::getFilePath($fileId);
        if (!$filePath) {
            return null;
        }

        // Расширение из file_path Telegram, иначе из suggestedExt.
        $ext = '';
        $dot = strrpos($filePath, '.');
        if ($dot !== false) {
            $ext = substr($filePath, $dot);
        }
        if ($ext === '' && $suggestedExt !== '') {
            $ext = str_starts_with($suggestedExt, '.') ? $suggestedExt : '.' . $suggestedExt;
        }

        $cacheFile = $cacheDir . '/' . $fileUniqueId . $ext;
        $downloadUrl = self::API . '/file/bot' . Config::$TELEGRAM_BOT_TOKEN . '/' . $filePath;

        $fp = fopen($cacheFile, 'wb');
        if ($fp === false) {
            return null;
        }
        $ch = curl_init();
        curl_setopt_array($ch, [
            CURLOPT_URL => $downloadUrl,
            CURLOPT_FILE => $fp,
            CURLOPT_TIMEOUT => 60,
            CURLOPT_FOLLOWLOCATION => true,
        ]);
        $ok = curl_exec($ch);
        $status = curl_getinfo($ch, CURLINFO_HTTP_CODE);
        curl_close($ch);
        fclose($fp);

        if ($ok === false || $status !== 200) {
            @unlink($cacheFile);
            error_log("download failed: $status");
            return null;
        }
        return $cacheFile;
    }

    public static function mimeFromExt(string $path): string
    {
        $ext = strtolower((string) pathinfo($path, PATHINFO_EXTENSION));
        $map = [
            'jpg' => 'image/jpeg', 'jpeg' => 'image/jpeg',
            'png' => 'image/png', 'gif' => 'image/gif', 'webp' => 'image/webp',
            'mp4' => 'video/mp4', 'mov' => 'video/quicktime', 'webm' => 'video/webm',
            'mp3' => 'audio/mpeg', 'm4a' => 'audio/mp4', 'ogg' => 'audio/ogg',
            'oga' => 'audio/ogg', 'opus' => 'audio/ogg',
            'pdf' => 'application/pdf',
        ];
        return $map[$ext] ?? 'application/octet-stream';
    }
}
