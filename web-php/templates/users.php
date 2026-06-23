<?php $page_title = 'Пользователи'; ?>

<h1>Пользователи <small class="muted">(<?= count($users) ?>)</small></h1>

<table class="users">
  <thead>
    <tr><th>Имя</th><th>Username</th><th>ID</th><th>Сообщений</th><th></th></tr>
  </thead>
  <tbody>
    <?php foreach ($users as $u): ?>
      <tr>
        <td><?= e($u['first_name'] ?? '—') ?> <?= e($u['last_name'] ?? '') ?></td>
        <td><?= !empty($u['username']) ? '@' . e($u['username']) : '' ?></td>
        <td class="muted"><?= e((string) $u['user_id']) ?></td>
        <td><?= (int) $u['message_count'] ?></td>
        <td><a href="/?user_id=<?= e((string) $u['user_id']) ?>">Сообщения →</a></td>
      </tr>
    <?php endforeach; ?>
  </tbody>
</table>
