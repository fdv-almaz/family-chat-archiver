<?php $page_title = 'Сообщения — Family Chat'; ?>

<h1>Сообщения <small class="muted">(<?= (int) $total ?>)</small></h1>

<form class="filters" method="get">
  <input type="search" name="q" placeholder="🔍 Поиск по тексту…" value="<?= e($filters['q'] ?? '') ?>">

  <select name="chat_id">
    <option value="">— все чаты —</option>
    <?php foreach ($chats as $c): ?>
      <option value="<?= e((string) $c['chat_id']) ?>" <?= ($filters['chat_id'] !== null && (int) $filters['chat_id'] === (int) $c['chat_id']) ? 'selected' : '' ?>>
        <?= e($c['chat_title'] ?? (string) $c['chat_id']) ?> (<?= (int) $c['message_count'] ?>)
      </option>
    <?php endforeach; ?>
  </select>

  <select name="message_type">
    <option value="">— все типы —</option>
    <?php foreach ($message_types as $t): ?>
      <option value="<?= e($t['message_type']) ?>" <?= ($filters['message_type'] === $t['message_type']) ? 'selected' : '' ?>>
        <?= e($t['message_type']) ?> (<?= (int) $t['c'] ?>)
      </option>
    <?php endforeach; ?>
  </select>

  <input type="date" name="date_from" value="<?= e($filters['date_from'] ?? '') ?>">
  <input type="date" name="date_to" value="<?= e($filters['date_to'] ?? '') ?>">

  <select name="page_size" title="Строк на странице">
    <?php foreach ($page_size_options as $n): ?>
      <option value="<?= (int) $n ?>" <?= ($page_size === $n) ? 'selected' : '' ?>><?= (int) $n ?> / стр.</option>
    <?php endforeach; ?>
  </select>

  <?php if ($filters['user_id'] !== null): ?>
    <input type="hidden" name="user_id" value="<?= (int) $filters['user_id'] ?>">
  <?php endif; ?>

  <button type="submit">Применить</button>
  <a href="/" class="btn-link">Сброс</a>
</form>

<table class="messages">
  <thead>
    <tr>
      <th class="num">№</th>
      <th>Когда</th>
      <th>Кто</th>
      <th>Чат</th>
      <th>Тип</th>
      <th>Текст</th>
      <th></th>
    </tr>
  </thead>
  <tbody>
    <?php $i = 0; foreach ($messages as $m): $i++; ?>
      <tr class="<?= $m['deleted_at'] ? 'deleted-row' : '' ?>">
        <td class="num"><?= ($page - 1) * $page_size + $i ?></td>
        <td class="nowrap">
          <?= e(fdate($m['created_at'] ?? null, 'Y-m-d H:i')) ?>
          <?php if ($m['deleted_at']): ?>
            <div class="muted" title="Удалено <?= e(fdate($m['deleted_at'], 'Y-m-d H:i')) ?>">🗑 удалено</div>
          <?php endif; ?>
        </td>
        <td>
          <a href="/?user_id=<?= e((string) ($m['user_id'] ?? '')) ?>">
            <?= e($m['display_first_name'] ?? '—') ?>
            <?php if (!empty($m['display_username'])): ?><span class="muted">@<?= e($m['display_username']) ?></span><?php endif; ?>
          </a>
        </td>
        <td class="muted"><?= e($m['chat_title'] ?? (string) ($m['chat_id'] ?? '')) ?></td>
        <td><span class="badge type-<?= e($m['message_type']) ?>"><?= e($m['message_type']) ?></span></td>
        <td class="text-cell">
          <?php if (!empty($m['text'])): ?>
            <div<?= $m['spelling_hint'] ? ' title="Исправлено: ' . e($m['spelling_hint']) . '"' : '' ?>>
              <?php if ($m['spelling_hint']): ?><span class="spell-flag" title="Исправлено: <?= e($m['spelling_hint']) ?>">✏️</span><?php endif; ?><?= truncate($m['text'], 200) ?>
            </div>
          <?php endif; ?>
          <?php foreach (($m['media_list'] ?? []) as $media): $src = '/media/' . (int) $media['media_id']; ?>
            <?php if (in_array($media['type'], ['audio', 'voice'], true)): ?>
              <audio controls preload="none" src="<?= e($src) ?>"></audio>
            <?php elseif (in_array($media['type'], ['photo', 'sticker'], true)): ?>
              <a href="/message/<?= (int) $m['message_id'] ?>"><img class="thumb" src="<?= e($src) ?>" alt="" loading="lazy"></a>
            <?php elseif (in_array($media['type'], ['video', 'video_note', 'animation'], true)): ?>
              <video class="thumb" src="<?= e($src) ?>" controls preload="none"></video>
            <?php elseif ($media['type'] === 'document'): ?>
              <a href="<?= e($src) ?>" download="<?= e($media['file_name'] ?? 'file') ?>" class="file-link">
                📎 <?= e($media['file_name'] ?? 'document') ?>
              </a>
            <?php endif; ?>
          <?php endforeach; ?>
        </td>
        <td><a href="/message/<?= (int) $m['message_id'] ?>">→</a></td>
      </tr>
    <?php endforeach; ?>
  </tbody>
</table>

<?php if ($total_pages > 1): ?>
<?php $baseFilters = array_filter($filters, fn($v) => $v !== null && $v !== '' && $v !== false); ?>
<nav class="pagination">
  <?php if ($page > 1): ?>
    <a href="?<?= e(qs($baseFilters + ['page' => $page - 1])) ?>">← Назад</a>
  <?php endif; ?>
  <span>Стр. <?= (int) $page ?> / <?= (int) $total_pages ?></span>
  <?php if ($page < $total_pages): ?>
    <a href="?<?= e(qs($baseFilters + ['page' => $page + 1])) ?>">Вперёд →</a>
  <?php endif; ?>
</nav>
<?php endif; ?>
