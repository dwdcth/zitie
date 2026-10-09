// 字帖生成器前端:原生 ES module,无构建、无依赖。
// 状态结构与服务端 GenerateParams 一一对应。

const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];

const form = $('#panel');
const fontSelect = $('#font-select');
const preview = $('#preview');
const statusEl = $('#status');
const downloadBtn = $('#download-btn');

/** 服务端默认偏好(也用于字段回退) */
const DEFAULTS = {
  text: '',
  paper: 'mizi',
  mode: 'miaohong',
  trace_count: 3,
  copy_count: 3,
  row_leading_demo: true,
  font_id: 'lxgw-wenkai',
  cell_size_pt: null,
  line_height_pt: null,
  cell_width_pt: null,
  cols_per_row: null,
  col_gap_pt: 4,
  row_gap_pt: 6,
  margins_mm: { top: 15, right: 15, bottom: 15, left: 15 },
  header: { title: null, fields: [], page_number: true },
  colors: { grid: '#9e9e9e', solid: '#2b2b2b', light: '#d68c8c', demo: '#575757' },
};

let state = structuredClone(DEFAULTS);
let fontsCache = { builtin: [], mine: [] };
let currentUrl = null;

// ---------- 工具 ----------
function toast(msg, err = false) {
  const el = document.createElement('div');
  el.className = 'toast' + (err ? ' err' : '');
  el.textContent = msg;
  $('#toasts').appendChild(el);
  setTimeout(() => el.remove(), err ? 5000 : 2600);
}

function debounce(fn, ms) {
  let t;
  return (...args) => {
    clearTimeout(t);
    t = setTimeout(() => fn(...args), ms);
  };
}

function countChars(text) {
  if (!text) return 0;
  if (Intl.Segmenter) {
    let n = 0;
    for (const _ of new Intl.Segmenter('zh', { granularity: 'grapheme' }).segment(text)) n++;
    return n;
  }
  return Array.from(text).length;
}

async function api(path, opts = {}) {
  const resp = await fetch(path, {
    headers: { 'Content-Type': 'application/json' },
    ...opts,
  });
  if (!resp.ok) {
    let msg = `请求失败(${resp.status})`;
    try {
      const body = await resp.json();
      if (body.error?.message) msg = body.error.message;
    } catch { /* keep default */ }
    throw new Error(msg);
  }
  if (resp.status === 204) return null;
  return resp.json();
}

// ---------- 表单 ↔ 状态 ----------
function numOrNull(v) {
  if (v === '' || v === null || v === undefined) return null;
  const n = Number(v);
  return Number.isFinite(n) ? n : null;
}

function readForm() {
  const fd = new FormData(form);
  state.paper = fd.get('paper') ?? state.paper;
  state.mode = fd.get('mode') ?? state.mode;
  state.text = $('#text').value;
  state.font_id = fontSelect.value || state.font_id;
  state.trace_count = Math.max(1, Math.min(10, Number(fd.get('trace_count')) || 3));
  state.copy_count = Math.max(1, Math.min(10, Number(fd.get('copy_count')) || 3));
  state.row_leading_demo = fd.get('row_leading_demo') === 'on';
  state.cell_size_pt = numOrNull(fd.get('cell_size_pt'));
  state.line_height_pt = numOrNull(fd.get('line_height_pt'));
  state.cell_width_pt = numOrNull(fd.get('cell_width_pt'));
  state.cols_per_row = numOrNull(fd.get('cols_per_row'));
  state.col_gap_pt = Number(fd.get('col_gap_pt')) || 0;
  state.row_gap_pt = Number(fd.get('row_gap_pt')) || 0;
  const mt = numOrNull(fd.get('margin_top')) ?? 15;
  const mb = numOrNull(fd.get('margin_bottom')) ?? 15;
  const mlr = numOrNull(fd.get('margin_lr')) ?? 15;
  state.margins_mm = { top: mt, bottom: mb, left: mlr, right: mlr };
  state.header = {
    title: fd.get('title')?.trim() || null,
    fields: $$('.field-pair').map((row) => ({
      label: $('input[data-k="label"]', row).value.trim(),
      value: $('input[data-k="value"]', row).value.trim(),
    })).filter((f) => f.label),
    page_number: fd.get('page_number') === 'on',
  };
  state.colors = {
    grid: fd.get('c_grid') || DEFAULTS.colors.grid,
    light: fd.get('c_light') || DEFAULTS.colors.light,
    demo: fd.get('c_demo') || DEFAULTS.colors.demo,
    solid: fd.get('c_solid') || DEFAULTS.colors.solid,
  };
}

function fillInput(name, val) {
  const el = form.querySelector(`[name="${name}"]`);
  if (!el) return;
  if (el.type === 'checkbox') el.checked = !!val;
  else el.value = val ?? '';
}

function writeForm() {
  fillInput('trace_count', state.trace_count);
  fillInput('copy_count', state.copy_count);
  form.querySelector('[name="row_leading_demo"]').checked = state.row_leading_demo;
  fillInput('cell_size_pt', state.cell_size_pt ?? '');
  fillInput('line_height_pt', state.line_height_pt ?? '');
  fillInput('cell_width_pt', state.cell_width_pt ?? '');
  fillInput('cols_per_row', state.cols_per_row ?? '');
  fillInput('col_gap_pt', state.col_gap_pt);
  fillInput('row_gap_pt', state.row_gap_pt);
  fillInput('margin_top', state.margins_mm.top);
  fillInput('margin_bottom', state.margins_mm.bottom);
  fillInput('margin_lr', (state.margins_mm.left + state.margins_mm.right) / 2);
  fillInput('title', state.header.title ?? '');
  form.querySelector('[name="page_number"]').checked = state.header.page_number;
  fillInput('c_grid', state.colors.grid);
  fillInput('c_light', state.colors.light);
  fillInput('c_demo', state.colors.demo);
  fillInput('c_solid', state.colors.solid);
  $('#text').value = state.text;
  const paper = form.querySelector(`[name="paper"][value="${state.paper}"]`);
  if (paper) paper.checked = true;
  const mode = form.querySelector(`[name="mode"][value="${state.mode}"]`);
  if (mode) mode.checked = true;
  renderHeaderFields();
  updateVisibility();
  updateCharCount();
}

/** 按纸型/模式显隐相关控件 */
function updateVisibility() {
  const ruled = state.paper === 'sixian' || state.paper === 'hengxian';
  const usesTrace = state.mode === 'miaohong' || state.mode === 'hunhe';
  const usesCopy = state.mode === 'chaoxie' || state.mode === 'hunhe';
  $('[data-for="cell-size"]').style.display = ruled ? 'none' : '';
  $('[data-for="line-height"]').style.display = ruled ? '' : 'none';
  $('[data-for="cell-width"]').style.display = ruled ? '' : 'none';
  $('[data-for="trace"]').style.display = usesTrace ? '' : 'none';
  $('[data-for="copy"]').style.display = usesCopy ? '' : 'none';
  $('[data-for="demo"]').style.display = state.mode === 'chaoxie' ? '' : 'none';
}

function updateCharCount() {
  $('#char-count').textContent = `${countChars($('#text').value)} 字`;
}

// ---------- 页眉字段 ----------
function renderHeaderFields() {
  const wrap = $('#header-fields');
  wrap.innerHTML = '';
  const fields = [...(state.header?.fields ?? [])];
  while (fields.length < 2 && fields.length < 3) {
    fields.push({ label: ['', '姓名', '班级'][fields.length + 1] ?? '', value: '' });
  }
  fields.slice(0, 3).forEach((f) => {
    const row = document.createElement('div');
    row.className = 'field-pair';
    row.innerHTML = `
      <input data-k="label" placeholder="字段名" maxlength="20" value="${f.label.replace(/"/g, '&quot;')}">
      <input data-k="value" placeholder="预填值(留空则画横线)" maxlength="30" value="${f.value.replace(/"/g, '&quot;')}">
      <button type="button" class="del" title="删除">×</button>`;
    row.querySelector('.del').addEventListener('click', () => {
      row.remove();
      readForm();
      scheduleGenerate();
      scheduleSave();
    });
    wrap.appendChild(row);
  });
  $('#add-field').style.display = $$('.field-pair').length >= 3 ? 'none' : '';
}

// ---------- 字体 ----------
function renderFonts() {
  fontSelect.innerHTML = '';
  const optgroup = (label, list) => {
    if (!list.length) return;
    const g = document.createElement('optgroup');
    g.label = label;
    for (const f of list) {
      const o = document.createElement('option');
      o.value = f.id;
      o.textContent = f.builtin ? f.family : `${f.family}(已上传)`;
      g.appendChild(o);
    }
    fontSelect.appendChild(g);
  };
  optgroup('内置字体', fontsCache.builtin);
  optgroup('我的字体', fontsCache.mine);
  if (![...fontSelect.options].some((o) => o.value === state.font_id)) {
    state.font_id = fontsCache.builtin[0]?.id ?? '';
    toast('原字体已不可用,已切换为默认字体');
  }
  fontSelect.value = state.font_id;
  renderMyFonts();
  loadFontPreview(state.font_id);
}

function renderMyFonts() {
  const wrap = $('#my-fonts');
  wrap.innerHTML = '';
  for (const f of fontsCache.mine) {
    const row = document.createElement('div');
    row.className = 'my-font-item';
    row.innerHTML = `<span>${f.family} · ${f.subfamily}</span><button type="button" class="del" title="删除字体">删除</button>`;
    row.querySelector('.del').addEventListener('click', async () => {
      try {
        await api(`/api/fonts/${f.id}`, { method: 'DELETE' });
        toast(`已删除字体 ${f.family}`);
        await refreshFonts();
      } catch (e) {
        toast(e.message, true);
      }
    });
    wrap.appendChild(row);
  }
}

async function refreshFonts() {
  fontsCache = await api('/api/fonts');
  renderFonts();
}

const loadedFontFaces = new Set();
async function loadFontPreview(fontId) {
  const box = $('#font-preview');
  if (!fontId) { box.hidden = true; return; }
  try {
    if (!loadedFontFaces.has(fontId)) {
      const buf = await (await fetch(`/api/fonts/${fontId}/file`)).arrayBuffer();
      const face = new FontFace('zitie-preview', buf);
      await face.load();
      document.fonts.add(face);
      loadedFontFaces.add(fontId);
    }
    box.hidden = false;
    box.style.fontFamily = 'zitie-preview';
    box.textContent = '永和九年 Aa Bb 123';
  } catch {
    box.hidden = true;
  }
}

function uploadFont(file) {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    const fd = new FormData();
    fd.append('font', file);
    xhr.open('POST', '/api/fonts');
    const progress = $('#upload-progress');
    const bar = $('.bar', progress);
    progress.hidden = false;
    bar.style.width = '0%';
    xhr.upload.onprogress = (e) => {
      if (e.lengthComputable) bar.style.width = `${(e.loaded / e.total) * 100}%`;
    };
    xhr.onload = () => {
      progress.hidden = true;
      if (xhr.status === 200 || xhr.status === 201) {
        let family = '新字体';
        try { family = JSON.parse(xhr.responseText).family; } catch { /* noop */ }
        resolve(family);
      } else {
        let msg = `上传失败(${xhr.status})`;
        try {
          const body = JSON.parse(xhr.responseText);
          if (body.error?.message) msg = body.error.message;
        } catch { /* noop */ }
        reject(new Error(msg));
      }
    };
    xhr.onerror = () => { progress.hidden = true; reject(new Error('网络错误')); };
    xhr.send(fd);
  });
}

// ---------- 生成与偏好 ----------
const generateNow = async () => {
  readForm();
  updateCharCount();
  try {
    statusEl.className = 'status';
    statusEl.textContent = '生成中…';
    const result = await api('/api/generate', {
      method: 'POST',
      body: JSON.stringify(state),
    });
    currentUrl = result.url;
    preview.src = result.url;
    statusEl.textContent = `${result.pages} 页 · 已更新`;
    downloadBtn.href = `${result.url}?dl=1`;
  } catch (e) {
    statusEl.className = 'status err';
    statusEl.textContent = '生成失败';
    toast(e.message, true);
  }
};

const savePrefs = async () => {
  try {
    await api('/api/preferences', { method: 'PUT', body: JSON.stringify(state) });
  } catch {
    /* 偏好保存失败不打扰用户 */
  }
};

const scheduleGenerate = debounce(generateNow, 400);
const scheduleSave = debounce(savePrefs, 800);

// ---------- 事件绑定 ----------
form.addEventListener('input', (e) => {
  if (e.target.name === 'font_id' || e.target.closest('#font-select')) return;
  scheduleGenerate();
  scheduleSave();
});
form.addEventListener('change', (e) => {
  if (e.target.name === 'paper' || e.target.name === 'mode') {
    readForm();
    updateVisibility();
  }
  if (e.target.name === 'font_id') {
    state.font_id = fontSelect.value;
    loadFontPreview(state.font_id);
  }
  scheduleGenerate();
  scheduleSave();
});
$('#text').addEventListener('input', () => {
  updateCharCount();
  scheduleGenerate();
  scheduleSave();
});

$('#add-field').addEventListener('click', () => {
  const rows = $$('.field-pair');
  if (rows.length >= 3) return;
  const row = document.createElement('div');
  row.className = 'field-pair';
  row.innerHTML = `
    <input data-k="label" placeholder="字段名" maxlength="20">
    <input data-k="value" placeholder="预填值" maxlength="30">
    <button type="button" class="del">×</button>`;
  row.querySelector('.del').addEventListener('click', () => row.remove());
  $('#header-fields').appendChild(row);
  $('#add-field').style.display = $$('.field-pair').length >= 3 ? 'none' : '';
});

$$('[data-sample]').forEach((btn) => {
  btn.addEventListener('click', () => {
    $('#text').value = btn.dataset.sample;
    updateCharCount();
    scheduleGenerate();
    scheduleSave();
  });
});

$('#font-upload-btn').addEventListener('click', () => $('#font-file').click());
$('#font-file').addEventListener('change', async (e) => {
  const file = e.target.files[0];
  e.target.value = '';
  if (!file) return;
  try {
    const family = await uploadFont(file);
    toast(`字体「${family}」上传成功`);
    await refreshFonts();
  } catch (err) {
    toast(err.message, true);
  }
});

$('#print-btn').addEventListener('click', () => {
  if (!currentUrl) return;
  try {
    preview.contentWindow.focus();
    preview.contentWindow.print();
  } catch {
    window.open(currentUrl, '_blank');
  }
});

// ---------- 启动 ----------
(async function init() {
  try {
    const [prefs, fonts] = await Promise.all([
      api('/api/preferences').catch(() => null),
      api('/api/fonts'),
    ]);
    fontsCache = fonts;
    if (prefs) {
      state = { ...structuredClone(DEFAULTS), ...prefs };
      state.margins_mm = { ...DEFAULTS.margins_mm, ...prefs.margins_mm };
      state.header = { ...DEFAULTS.header, ...prefs.header };
      state.colors = { ...DEFAULTS.colors, ...prefs.colors };
    }
    writeForm();
    renderFonts();
    generateNow();
  } catch (e) {
    toast(e.message, true);
  }
})();
