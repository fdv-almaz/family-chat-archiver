<!doctype html>
<html lang="ru">
<head>
<meta charset="utf-8">
<title><?= e($page_title ?? 'Family Chat Archiver') ?></title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<link rel="icon" href="data:image/svg+xml,<svg xmlns=%22http://www.w3.org/2000/svg%22 viewBox=%220 0 100 100%22><text y=%22.9em%22 font-size=%2290%22>📚</text></svg>">
<link rel="stylesheet" href="/static/style.css">
</head>
<body>
<header>
  <nav>
    <a href="/" class="brand">📚 Family Chat Archive</a>
    <a href="/">Сообщения</a>
    <a href="/users">Пользователи</a>
    <a href="/stats">Статистика</a>
    <a href="/corrections">Орфография</a>
    <a href="/tips">Советы дня</a>
  </nav>
</header>
<main>
  <?= $content ?>
</main>
<footer>
  <span>Family Chat Archiver — Web (PHP) v<?= e($VERSION) ?></span>
</footer>
<script src="/static/app.js" defer></script>
</body>
</html>
