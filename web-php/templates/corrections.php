<?php $page_title = 'Орфография'; ?>

<h1>Орфографические исправления</h1>

<table>
  <thead>
    <tr><th>Когда</th><th>Кто</th><th>Было</th><th>Стало</th><th></th></tr>
  </thead>
  <tbody>
    <?php foreach ($corrections as $c): ?>
      <tr>
        <td class="nowrap"><?= e(fdate($c['created_at'] ?? null, 'Y-m-d H:i')) ?></td>
        <td><?= e($c['user_first_name'] ?? '—') ?></td>
        <td><code><?= e(mb_substr($c['original_text'] ?? '', 0, 120, 'UTF-8')) ?></code></td>
        <td><code><?= e(mb_substr($c['corrected_text'] ?? '', 0, 120, 'UTF-8')) ?></code></td>
        <td><a href="/message/<?= (int) $c['message_id'] ?>">→</a></td>
      </tr>
    <?php endforeach; ?>
  </tbody>
</table>
