// Simple bar chart for stats page (no external deps).
// Данные берём из data-атрибута канваса (CSP запрещает inline-скрипты).
(function() {
  const canvas = document.getElementById('chart');
  if (!canvas) return;

  let data = [];
  try {
    data = JSON.parse(canvas.dataset.chart || '[]');
  } catch (e) {
    return;
  }
  if (!data.length) return;

  const ctx = canvas.getContext('2d');
  const dpr = window.devicePixelRatio || 1;
  const cssW = canvas.clientWidth, cssH = canvas.clientHeight;
  canvas.width = cssW * dpr;
  canvas.height = cssH * dpr;
  ctx.scale(dpr, dpr);

  const pad = { l: 40, r: 10, t: 10, b: 30 };
  const w = cssW - pad.l - pad.r;
  const h = cssH - pad.t - pad.b;
  const maxV = Math.max(1, ...data.map(d => d.count));
  const barW = w / data.length;

  ctx.fillStyle = '#888'; ctx.font = '11px sans-serif'; ctx.textAlign = 'right';
  // y-axis ticks
  for (let i = 0; i <= 4; i++) {
    const v = Math.round(maxV * i / 4);
    const y = pad.t + h - (h * i / 4);
    ctx.fillText(v, pad.l - 6, y + 3);
    ctx.strokeStyle = '#eee';
    ctx.beginPath(); ctx.moveTo(pad.l, y); ctx.lineTo(pad.l + w, y); ctx.stroke();
  }

  // bars
  ctx.fillStyle = '#2962ff';
  data.forEach((d, i) => {
    const x = pad.l + i * barW + 2;
    const bh = (d.count / maxV) * h;
    ctx.fillRect(x, pad.t + h - bh, barW - 4, bh);
  });

  // x-axis labels (sparse)
  ctx.fillStyle = '#888'; ctx.textAlign = 'center';
  const step = Math.ceil(data.length / 8);
  data.forEach((d, i) => {
    if (i % step !== 0 && i !== data.length - 1) return;
    const x = pad.l + i * barW + barW / 2;
    ctx.fillText(d.day.slice(5), x, pad.t + h + 18);
  });
})();

// Подтверждение для опасных форм (вместо inline onsubmit — требование CSP).
document.addEventListener('submit', function(ev) {
  const form = ev.target;
  if (form.classList && form.classList.contains('js-confirm')) {
    if (!window.confirm(form.dataset.confirm || 'Подтвердите действие')) {
      ev.preventDefault();
    }
  }
});

// Ссылка «Назад» (вместо javascript:history.back() — требование CSP).
document.addEventListener('click', function(ev) {
  const a = ev.target.closest && ev.target.closest('a.js-back');
  if (a) {
    if (window.history.length > 1) {
      ev.preventDefault();
      window.history.back();
    }
    // иначе — обычный переход по href="/"
  }
});
