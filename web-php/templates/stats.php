<?php $page_title = 'Статистика'; ?>

<h1>Статистика</h1>

<div class="cards">
  <div class="card"><div class="big"><?= (int) $overview['total_messages'] ?></div><div>сообщений</div></div>
  <div class="card"><div class="big"><?= (int) $overview['total_users'] ?></div><div>пользователей</div></div>
  <div class="card"><div class="big"><?= (int) $overview['total_media'] ?></div><div>медиа-файлов</div></div>
  <div class="card"><div class="big"><?= (int) $overview['total_links'] ?></div><div>ссылок</div></div>
  <div class="card"><div class="big"><?= (int) $overview['total_corrections'] ?></div><div>исправлений</div></div>
</div>

<h2>Сообщения за 30 дней</h2>
<canvas id="chart" height="180"></canvas>
<script>
window.__chartData = [
<?php foreach ($messages_per_day as $i => $r): ?>
  { day: <?= json_encode((string) $r['day']) ?>, count: <?= (int) $r['c'] ?> }<?= $i < count($messages_per_day) - 1 ? ',' : '' ?>
<?php endforeach; ?>
];
</script>

<div class="two-col">
  <section>
    <h2>Топ-10 авторов</h2>
    <table>
      <thead><tr><th>Имя</th><th>Сообщений</th></tr></thead>
      <tbody>
        <?php foreach ($top_users as $u): ?>
        <tr>
          <td><a href="/?user_id=<?= e((string) $u['user_id']) ?>"><?= e($u['first_name'] ?? '—') ?><?php if (!empty($u['username'])): ?> <span class="muted">@<?= e($u['username']) ?></span><?php endif; ?></a></td>
          <td><?= (int) $u['message_count'] ?></td>
        </tr>
        <?php endforeach; ?>
      </tbody>
    </table>
  </section>

  <section>
    <h2>Типы сообщений</h2>
    <table>
      <thead><tr><th>Тип</th><th>Кол-во</th></tr></thead>
      <tbody>
        <?php foreach ($message_types as $t): ?>
        <tr><td><?= e($t['message_type']) ?></td><td><?= (int) $t['c'] ?></td></tr>
        <?php endforeach; ?>
      </tbody>
    </table>
  </section>
</div>
