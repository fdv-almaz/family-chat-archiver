<?php $page_title = 'Сообщение #' . (int) $msg['message_id']; ?>

<p><a href="javascript:history.back()">← Назад</a></p>

<article class="message-detail <?= $msg['deleted_at'] ? 'deleted-row' : '' ?>">
  <header>
    <h1>
      <?= e($msg['display_first_name'] ?? '—') ?>
      <?php if (!empty($msg['display_username'])): ?><span class="muted">@<?= e($msg['display_username']) ?></span><?php endif; ?>
    </h1>
    <p class="muted">
      <?= e(fdate($msg['created_at'] ?? null, 'Y-m-d H:i:s')) ?> ·
      Чат: <?= e($msg['chat_title'] ?? (string) ($msg['chat_id'] ?? '')) ?> ·
      Тип: <span class="badge type-<?= e($msg['message_type']) ?>"><?= e($msg['message_type']) ?></span>
    </p>
    <?php if ($msg['deleted_at']): ?>
      <p class="deleted-banner">🗑 Сообщение помечено как удалённое <?= e(fdate($msg['deleted_at'], 'Y-m-d H:i:s')) ?></p>
    <?php endif; ?>
  </header>

  <?php if (!empty($msg['text'])): ?>
    <div class="text-block"><?= e($msg['text']) ?></div>
  <?php endif; ?>

  <?php if (!empty($msg['media'])): ?>
    <div class="media-block">
      <?php foreach ($msg['media'] as $m): $src = '/media/' . (int) $m['media_id']; ?>
        <div class="media-item">
          <?php if (in_array($m['type'], ['photo', 'sticker', 'animation'], true)): ?>
            <img src="<?= e($src) ?>" alt="<?= e($m['type']) ?>" loading="lazy">
          <?php elseif (in_array($m['type'], ['video', 'video_note'], true)): ?>
            <video controls src="<?= e($src) ?>"></video>
          <?php elseif (in_array($m['type'], ['audio', 'voice'], true)): ?>
            <audio controls src="<?= e($src) ?>"></audio>
            <?php if (!empty($m['duration'])): ?><span class="muted"><?= (int) $m['duration'] ?>s</span><?php endif; ?>
            <?php if (!empty($m['file_name'])): ?><div class="muted"><?= e($m['file_name']) ?></div><?php endif; ?>
          <?php elseif ($m['type'] === 'document'): ?>
            <a href="<?= e($src) ?>" download="<?= e($m['file_name'] ?? 'file') ?>">
              📎 <?= e($m['file_name'] ?? 'document') ?>
              <?php if (!empty($m['file_size'])): ?><span class="muted">(<?= e((string) round($m['file_size'] / 1024, 1)) ?> KB)</span><?php endif; ?>
            </a>
          <?php endif; ?>
        </div>
      <?php endforeach; ?>
    </div>
  <?php endif; ?>

  <?php if (!empty($msg['links'])): ?>
    <h3>Ссылки</h3>
    <ul>
      <?php foreach ($msg['links'] as $l): ?><li><a href="<?= e($l['url']) ?>" rel="noopener" target="_blank"><?= e($l['url']) ?></a></li><?php endforeach; ?>
    </ul>
  <?php endif; ?>

  <?php if (!empty($msg['corrections'])): ?>
    <h3>Орфографические подсказки</h3>
    <?php foreach ($msg['corrections'] as $c): ?>
      <div class="correction">
        <div><strong>Было:</strong> <code><?= e($c['original_text']) ?></code></div>
        <div><strong>Стало:</strong> <code><?= e($c['corrected_text']) ?></code></div>
      </div>
    <?php endforeach; ?>
  <?php endif; ?>

  <footer class="actions">
    <?php if ($msg['deleted_at']): ?>
      <form method="post" action="/message/<?= (int) $msg['message_id'] ?>/restore">
        <button type="submit">↩ Восстановить</button>
      </form>
      <form method="post" action="/message/<?= (int) $msg['message_id'] ?>/hard-delete"
            onsubmit="return confirm('Удалить #<?= (int) $msg['message_id'] ?> БЕЗВОЗВРАТНО? Связанные медиа, ссылки и подсказки тоже удалятся.')">
        <button type="submit" class="danger">⚠ Удалить окончательно</button>
      </form>
    <?php else: ?>
      <form method="post" action="/message/<?= (int) $msg['message_id'] ?>/delete">
        <button type="submit" class="danger">🗑 Пометить как удалённое</button>
      </form>
    <?php endif; ?>
  </footer>
</article>
