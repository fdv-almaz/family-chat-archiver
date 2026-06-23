<?php $page_title = 'Советы дня — Family Chat'; ?>

<h1>Советы дня <small class="muted">(<?= (int) $total ?>)</small></h1>

<table>
  <thead>
    <tr>
      <th>Когда</th>
      <th>Чат</th>
      <th>Модель</th>
      <th>Совет</th>
      <th>Статус</th>
    </tr>
  </thead>
  <tbody>
    <?php if (empty($tips)): ?>
      <tr><td colspan="5" class="muted">Пока нет советов дня. Они появятся после рассылки или команды /check_tip.</td></tr>
    <?php else: ?>
      <?php foreach ($tips as $t): ?>
        <tr>
          <td class="nowrap"><?= e(fdate($t['created_at'] ?? null, 'Y-m-d H:i')) ?></td>
          <td class="muted"><?= e($t['chat_title'] ?? (string) ($t['chat_id'] ?? '')) ?></td>
          <td class="muted"><?= e($t['model']) ?></td>
          <td class="text-cell">
            <?php if (!empty($t['response'])): ?><div><?= e($t['response']) ?></div><?php endif; ?>
            <?php if (!empty($t['error'])): ?><div class="muted" title="<?= e($t['error']) ?>">⚠️ <?= truncate($t['error'], 200) ?></div><?php endif; ?>
          </td>
          <td class="nowrap"><?= $t['sent_to_chat'] ? '✅ отправлен' : '❌ не отправлен' ?></td>
        </tr>
      <?php endforeach; ?>
    <?php endif; ?>
  </tbody>
</table>

<?php if (($total_pages ?? 1) > 1): ?>
<nav class="pagination">
  <?php if ($page > 1): ?><a href="?page=<?= (int) $page - 1 ?>">← Назад</a><?php endif; ?>
  <span>Стр. <?= (int) $page ?> / <?= (int) $total_pages ?></span>
  <?php if ($page < $total_pages): ?><a href="?page=<?= (int) $page + 1 ?>">Вперёд →</a><?php endif; ?>
</nav>
<?php endif; ?>
