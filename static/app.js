// 字帖生成器前端:原生 ES module,无构建、无依赖。
// 状态结构与服务端 GenerateParams 一一对应。

const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];

const form = $('#panel');
const fontSelect = $('#font-select');
const preview = $('#preview');
const statusEl = $('#status');
const downloadBtn = $('#download-btn');

// ---------- i18n ----------
const I18N = {
  zh: {
    app_title: '字帖生成器',
    subtitle: '汉字 · 英文 · 任意语言 | 描红 / 抄写字帖,支持自定义字体',
    paper: '纸型', mode: '模式', font: '字体', content: '练习内容', header: '页眉', size_color: '尺寸与颜色',
    p_mizi: '米字格', p_tianzi: '田字格', p_huigong: '回宫格', p_zuowen: '作文纸', p_sixian: '四线三格', p_hengxian: '单横线',
    m_miaohong: '描红', m_chaoxie: '抄写', m_hunhe: '描红+抄写', m_chuntie: '纯字帖',
    trace_count: '描红格数', copy_count: '抄写格数', row_demo: '抄写模式行首给示范字',
    pinyin_label: '汉字上方标注拼音',
    direction: '排列方向', dir_auto: '自动(按文字判断)', dir_ltr: '从左到右', dir_rtl: '从右到左',
    upload_font: '上传字体',
    font_hint: '支持 .ttf / .otf(≤20MB)。内置字体均为开源协议(霞鹜文楷 OFL 等);衡水体等商用字体请自行上传。',
    content_ph: '输入要练习的文字,例如:永和九年 岁在癸丑\n换行会另起一行;空格占一个格',
    s_lanting: '《兰亭集序》', s_jingye: '《静夜思》', s_abc: '字母表',
    title_label: '标题', title_ph: '如:三年级语文生字练习',
    field_label: '字段名', field_value: '预填值(留空画横线)',
    add_field: '+ 添加字段', page_number: '显示页码',
    cell_size: '格子大小 pt', line_height: '行高 pt', cell_width: '格宽 pt', cols: '每行格数',
    col_gap: '格间距 pt', row_gap: '行间距 pt',
    margin_top: '上边距 mm', margin_bottom: '下边距 mm', margin_lr: '左右边距 mm',
    c_grid: '格线', c_light: '描红', c_demo: '示范', c_solid: '实字',
    auto: '自动',
    history: '历史', print: '打印', download: '下载 PDF',
    ready: '就绪', generating: '生成中…', failed: '生成失败',
    history_empty: '还没有生成记录',
    restore: '恢复', del: '删除', pages_unit: '页',
    uploaded_tag: '(已上传)',
    char_unit: '字',
    mobile_hint: '当前浏览器可能不支持内嵌 PDF 预览,请使用「下载 PDF」按钮。',
    err_font_gone: '原字体已不可用,已切换为默认字体',
    toast_font_deleted: (f) => `已删除字体 ${f}`,
    toast_font_uploaded: (f) => `字体「${f.family}」上传成功`,
    toast_restored: '已恢复历史设置',
  },
  en: {
    app_title: 'Copybook Generator',
    subtitle: 'Chinese · English · Any language | Tracing & copying sheets, custom fonts',
    paper: 'Paper', mode: 'Mode', font: 'Font', content: 'Practice text', header: 'Header', size_color: 'Size & colors',
    p_mizi: 'Mi-cell', p_tianzi: 'Tian-cell', p_huigong: 'Hui-gong', p_zuowen: 'Composition', p_sixian: 'Four-line', p_hengxian: 'Ruled',
    m_miaohong: 'Trace', m_chaoxie: 'Copy', m_hunhe: 'Trace+Copy', m_chuntie: 'Reference',
    trace_count: 'Trace cells', copy_count: 'Copy cells', row_demo: 'Leading example cell when copying',
    pinyin_label: 'Pinyin above each character',
    direction: 'Direction', dir_auto: 'Auto (per script)', dir_ltr: 'Left to right', dir_rtl: 'Right to left',
    upload_font: 'Upload font',
    font_hint: 'Accepts .ttf / .otf (≤20MB). Built-in fonts are open source (OFL); upload commercial fonts yourself.',
    content_ph: 'Text to practice, e.g. 永和九年\nNewlines start a new row; spaces take one cell',
    s_lanting: 'Lanting Xu', s_jingye: 'Jing Ye Si', s_abc: 'A–Z',
    title_label: 'Title', title_ph: 'e.g. Grade 3 vocabulary',
    field_label: 'Label', field_value: 'Value (blank → underline)',
    add_field: '+ Add field', page_number: 'Show page numbers',
    cell_size: 'Cell size pt', line_height: 'Line height pt', cell_width: 'Cell width pt', cols: 'Cells per row',
    col_gap: 'Col gap pt', row_gap: 'Row gap pt',
    margin_top: 'Top margin mm', margin_bottom: 'Bottom margin mm', margin_lr: 'L/R margin mm',
    c_grid: 'Grid', c_light: 'Trace', c_demo: 'Example', c_solid: 'Solid',
    auto: 'auto',
    history: 'History', print: 'Print', download: 'Download PDF',
    ready: 'Ready', generating: 'Generating…', failed: 'Failed',
    history_empty: 'No generations yet',
    restore: 'Restore', del: 'Delete', pages_unit: 'pages',
    uploaded_tag: '(uploaded)',
    char_unit: 'chars',
    mobile_hint: 'Your browser may not support inline PDF preview. Use the Download button.',
    err_font_gone: 'Previous font unavailable, switched to default',
    toast_font_deleted: (f) => `Deleted font ${f}`,
    toast_font_uploaded: (f) => `Font "${f.family}" uploaded`,
    toast_restored: 'Restored from history',
  },
};

let lang = localStorage.getItem('zitie-lang')
  || ((navigator.language || 'zh').toLowerCase().startsWith('zh') ? 'zh' : 'en');

const t = (key) => {
  const v = I18N[lang][key];
  return typeof v === 'function' ? v : (v ?? I18N.zh[key] ?? key);
};

function applyI18n() {
  document.documentElement.lang = lang === 'zh' ? 'zh-CN' : 'en';
  $$('[data-i18n]').forEach((el) => { el.textContent = t(el.dataset.i18n); });
  $$('[data-i18n-ph]').forEach((el) => { el.placeholder = t(el.dataset.i18nPh); });
  $('#lang-btn').textContent = lang === 'zh' ? 'EN' : '中文';
  updateCharCount();
}

/** 服务端错误消息在 EN 界面下的通用兜底(保留具体中文消息)。 */
function errText(e) {
  return e.message;
}

/** 服务端默认偏好(也用于字段回退) */
const DEFAULTS = {
  text: '',
  paper: 'mizi',
  mode: 'miaohong',
  trace_count: 3,
  copy_count: 3,
  row_leading_demo: true,
  pinyin: false,
  direction: 'auto',
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
  let t2;
  return (...args) => {
    clearTimeout(t2);
    t2 = setTimeout(() => fn(...args), ms);
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
    let msg = `HTTP ${resp.status}`;
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
  state.pinyin = fd.get('pinyin') === 'on';
  state.direction = fd.get('direction') ?? 'auto';
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
  form.querySelector('[name="pinyin"]').checked = !!state.pinyin;
  fillInput('direction', state.direction || 'auto');
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
  $('[data-for="pinyin"]').style.display = ruled ? 'none' : '';
}

function updateCharCount() {
  $('#char-count').textContent = `${countChars($('#text').value)} ${t('char_unit')}`;
}

// ---------- 页眉字段 ----------
function renderHeaderFields() {
  const wrap = $('#header-fields');
  wrap.innerHTML = '';
  const fields = [...(state.header?.fields ?? [])];
  while (fields.length < 2 && fields.length < 3) {
    const preset = lang === 'zh'
      ? ['', '姓名', '班级'][fields.length + 1] ?? ''
      : ['', 'Name', 'Class'][fields.length + 1] ?? '';
    fields.push({ label: preset, value: '' });
  }
  fields.slice(0, 3).forEach((f) => {
    const row = document.createElement('div');
    row.className = 'field-pair';
    row.innerHTML = `
      <input data-k="label" data-i18n-ph="field_label" placeholder="字段名" maxlength="20" value="${f.label.replace(/"/g, '&quot;')}">
      <input data-k="value" data-i18n-ph="field_value" placeholder="预填值(留空画横线)" maxlength="30" value="${f.value.replace(/"/g, '&quot;')}">
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
      o.textContent = f.builtin ? f.family : `${f.family}${t('uploaded_tag')}`;
      g.appendChild(o);
    }
    fontSelect.appendChild(g);
  };
  optgroup(lang === 'zh' ? '内置字体' : 'Built-in', fontsCache.builtin);
  optgroup(lang === 'zh' ? '我的字体' : 'My fonts', fontsCache.mine);
  if (![...fontSelect.options].some((o) => o.value === state.font_id)) {
    state.font_id = fontsCache.builtin[0]?.id ?? '';
    toast(t('err_font_gone'));
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
    row.innerHTML = `<span>${f.family} · ${f.subfamily}</span><button type="button" class="del" title="${t('del')}">${t('del')}</button>`;
    row.querySelector('.del').addEventListener('click', async () => {
      try {
        await api(`/api/fonts/${f.id}`, { method: 'DELETE' });
        toast(t('toast_font_deleted')(f.family));
        await refreshFonts();
      } catch (e) {
        toast(errText(e), true);
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
        let family = 'font';
        try { family = JSON.parse(xhr.responseText).family; } catch { /* noop */ }
        resolve({ family });
      } else {
        let msg = `HTTP ${xhr.status}`;
        try {
          const body = JSON.parse(xhr.responseText);
          if (body.error?.message) msg = body.error.message;
        } catch { /* noop */ }
        reject(new Error(msg));
      }
    };
    xhr.onerror = () => { progress.hidden = true; reject(new Error('network error')); };
    xhr.send(fd);
  });
}

// ---------- 生成历史 ----------
let lastHistoryItems = [];

async function refreshHistory() {
  try {
    const items = await api('/api/history');
    lastHistoryItems = items;
    renderHistory(items);
  } catch { /* 历史加载失败不打扰 */ }
}

function renderHistory(items) {
  const panel = $('#history-panel');
  panel.innerHTML = '';
  if (!items?.length) {
    const empty = document.createElement('div');
    empty.className = 'history-empty';
    empty.textContent = t('history_empty');
    panel.appendChild(empty);
    return;
  }
  for (const it of items.slice(0, 20)) {
    const row = document.createElement('div');
    row.className = 'history-item';
    const date = new Date(it.created_at).toLocaleString(lang === 'zh' ? 'zh-CN' : 'en-US', {
      month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit',
    });
    row.innerHTML = `
      <div class="hi-main">
        <span class="hi-preview"></span>
        <span class="hi-meta">${it.pages} ${t('pages_unit')} · ${date}</span>
      </div>
      <div class="hi-ops">
        <button type="button" class="linklike" data-op="restore">${t('restore')}</button>
        <button type="button" class="linklike danger" data-op="del">${t('del')}</button>
      </div>`;
    row.querySelector('.hi-preview').textContent = it.preview || '( )';
    row.querySelector('[data-op="restore"]').addEventListener('click', async () => {
      try {
        const params = await api(`/api/history/${it.id}`);
        state = { ...structuredClone(DEFAULTS), ...params };
        state.margins_mm = { ...DEFAULTS.margins_mm, ...params.margins_mm };
        state.header = { ...DEFAULTS.header, ...params.header };
        state.colors = { ...DEFAULTS.colors, ...params.colors };
        writeForm();
        generateNow();
        panel.hidden = true;
        toast(t('toast_restored'));
      } catch (e) {
        toast(errText(e), true);
      }
    });
    row.querySelector('[data-op="del"]').addEventListener('click', async () => {
      try {
        await api(`/api/history/${it.id}`, { method: 'DELETE' });
        refreshHistory();
      } catch (e) {
        toast(errText(e), true);
      }
    });
    panel.appendChild(row);
  }
}

// ---------- 生成与偏好 ----------
const generateNow = async () => {
  readForm();
  updateCharCount();
  try {
    statusEl.className = 'status';
    statusEl.textContent = t('generating');
    const result = await api('/api/generate', {
      method: 'POST',
      body: JSON.stringify(state),
    });
    currentUrl = result.url;
    preview.src = result.url;
    statusEl.textContent = `${result.pages} ${t('pages_unit')} · ${lang === 'zh' ? '已更新' : 'updated'}`;
    downloadBtn.href = `${result.url}?dl=1`;
    refreshHistory();
  } catch (e) {
    statusEl.className = 'status err';
    statusEl.textContent = t('failed');
    toast(errText(e), true);
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
    <input data-k="label" data-i18n-ph="field_label" placeholder="字段名" maxlength="20">
    <input data-k="value" data-i18n-ph="field_value" placeholder="预填值" maxlength="30">
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
    const info = await uploadFont(file);
    toast(t('toast_font_uploaded')(info));
    await refreshFonts();
  } catch (err) {
    toast(errText(err), true);
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

// 历史下拉开合
$('#history-btn').addEventListener('click', (e) => {
  e.stopPropagation();
  const panel = $('#history-panel');
  panel.hidden = !panel.hidden;
  if (!panel.hidden) refreshHistory();
});
document.addEventListener('click', (e) => {
  if (!e.target.closest('.history-wrap')) $('#history-panel').hidden = true;
});

// 语言切换
$('#lang-btn').addEventListener('click', () => {
  lang = lang === 'zh' ? 'en' : 'zh';
  localStorage.setItem('zitie-lang', lang);
  applyI18n();
  renderFonts();
  renderHistory(lastHistoryItems);
});

// ---------- 启动 ----------
(async function init() {
  applyI18n();
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
    refreshHistory();
    generateNow();
  } catch (e) {
    toast(errText(e), true);
  }
})();
