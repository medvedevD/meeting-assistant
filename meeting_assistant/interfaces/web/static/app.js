// ============================================================
// View router
// ============================================================
let _view = 'home';
let _selectedSlug = '';

function showView(id) {
  document.querySelectorAll('.view').forEach(v => v.classList.remove('active'));
  const el = document.getElementById('view-' + id);
  if (el) el.classList.add('active');
  _view = id;
  document.getElementById('settings-link').classList.toggle('active', id === 'settings');
}

function showHome() {
  _selectedSlug = '';
  document.querySelectorAll('.meeting-item').forEach(i => i.classList.remove('active'));
  showView('home');
}

function showSettings() { showView('settings'); }

function showProcess() {
  fetchRecordings();
  showView('process');
}

function closeViewer() { showHome(); }

let _recordMode = 'record'; // 'record' | 'upload'

function _setRecordMode(mode) {
  _recordMode = mode;
  const isUpload = mode === 'upload';
  document.getElementById('record-panel-title').textContent = isUpload ? 'Загрузить аудио' : 'Новая запись';
  document.getElementById('rec-controls').style.display = isUpload ? 'none' : '';
  document.getElementById('upload-controls').style.display = isUpload ? '' : 'none';
  document.getElementById('rec-action-buttons').style.display = isUpload ? 'none' : '';
  document.getElementById('upload-action-buttons').style.display = isUpload ? '' : 'none';
  document.getElementById('pipeline-preview').style.display = '';
}

function toggleRecord() {
  if (_view === 'record' && _recordMode === 'record') return;
  _setRecordMode('record');
  showView('record');
}

function toggleUpload() {
  if (_view === 'record' && _recordMode === 'upload') return;
  _setRecordMode('upload');
  showView('record');
}

// ============================================================
// Sidebar meetings list
// ============================================================
let _meetingData = { protocol: null, transcript: null };
let _activeTab = 'protocol';
let _meetings = [];

async function fetchMeetings() {
  const r = await fetch('/api/meetings');
  const d = await r.json();
  _meetings = d.folders || [];
  renderList(_meetings);
}

function renderList(folders) {
  const list = document.getElementById('meetings-list');
  if (!folders || !folders.length) {
    list.innerHTML = '<div class="sidebar-section">Встречи</div><div class="no-meetings">Встреч пока нет</div>';
    return;
  }
  const sectionHtml = '<div class="sidebar-section">Встречи</div>';
  const itemsHtml = folders.map(f => {
    const slug = typeof f === 'string' ? f : f.slug || f;
    const title = slug.replace(/_/g, ' ');
    const hasTr = f.has_transcript;
    const hasPr = f.has_protocol;
    let meta = '';
    if (!hasTr && !hasPr) {
      meta = '<span class="has">· только запись</span>';
    } else {
      const parts = [];
      if (hasPr) parts.push('протокол ✓');
      if (hasTr) parts.push('транскрипция ✓');
      meta = `<span class="has">· ${parts.join(' · ')}</span>`;
    }
    return `<div class="meeting-item" data-slug="${slug}" onclick="selectMeeting('${slug}')">
      <div class="meeting-item-body">
        <div class="title">${title}</div>
        <div class="meta">${meta}</div>
      </div>
      <button class="del-btn" onclick="openDeleteConfirm(event,'${slug}')">✕</button>
    </div>`;
  }).join('');
  list.innerHTML = sectionHtml + itemsHtml;
  if (_selectedSlug) {
    const active = list.querySelector(`[data-slug="${_selectedSlug}"]`);
    if (active) active.classList.add('active');
  }
}

async function selectMeeting(slug) {
  _selectedSlug = slug;
  document.querySelectorAll('.meeting-item').forEach(i =>
    i.classList.toggle('active', i.dataset.slug === slug));
  const r = await fetch('/api/meeting?folder=' + encodeURIComponent(slug));
  _meetingData = await r.json();
  const title = slug.replace(/_/g, ' ');
  document.getElementById('viewer-title').textContent = title;
  const hasProtocol = !!_meetingData.protocol;
  const hasTranscript = !!_meetingData.transcript;
  document.getElementById('btn-regen').style.display = hasTranscript ? '' : 'none';
  document.getElementById('tab-protocol').style.display = hasProtocol ? '' : 'none';
  document.getElementById('tab-transcript').style.display = hasTranscript ? '' : 'none';
  const tab = hasProtocol ? 'protocol' : (hasTranscript ? 'transcript' : 'protocol');
  showTab(tab);
  showView('meeting');
}

// ============================================================
// Meeting viewer tabs
// ============================================================
function showTab(tab) {
  _activeTab = tab;
  document.getElementById('tab-protocol').classList.toggle('active', tab === 'protocol');
  document.getElementById('tab-transcript').classList.toggle('active', tab === 'transcript');
  const body = document.getElementById('viewer-body');
  if (tab === 'protocol') {
    body.innerHTML = _meetingData.protocol
      ? `<div class="md-content">${renderMarkdown(_meetingData.protocol)}</div>`
      : '<div style="color:var(--text-muted);font-size:.83rem;padding:12px 0">Протокол не сгенерирован</div>';
  } else {
    body.innerHTML = _meetingData.transcript
      ? `<div class="transcript-wrap">${renderTranscript(_meetingData.transcript)}</div>`
      : '<div style="color:var(--text-muted);font-size:.83rem;padding:12px 0">Транскрипция отсутствует</div>';
  }
}

function renderMarkdown(md) {
  const esc = s => s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
  const inline = s => s
    .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    .replace(/`([^`]+)`/g, '<code>$1</code>');
  const lines = esc(md).split('\n');
  const out = [];
  let inUl = false, inOl = false, para = [];
  const flushPara = () => {
    if (para.length) { out.push('<p>' + para.join('<br>') + '</p>'); para = []; }
  };
  const flushList = () => {
    if (inUl) { out.push('</ul>'); inUl = false; }
    if (inOl) { out.push('</ol>'); inOl = false; }
  };
  for (const raw of lines) {
    const line = raw.trimEnd();
    let m;
    if ((m = line.match(/^(#{1,3}) (.+)/))) {
      flushPara(); flushList();
      const tag = 'h' + m[1].length;
      out.push(`<${tag}>${inline(m[2])}</${tag}>`);
    } else if ((m = line.match(/^&gt; (.+)/))) {
      flushPara(); flushList();
      out.push(`<blockquote>${inline(m[1])}</blockquote>`);
    } else if ((m = line.match(/^\s*[-*] (.+)/))) {
      flushPara();
      if (inOl) { out.push('</ol>'); inOl = false; }
      if (!inUl) { out.push('<ul>'); inUl = true; }
      out.push(`<li>${inline(m[1])}</li>`);
    } else if ((m = line.match(/^\d+\. (.+)/))) {
      flushPara();
      if (inUl) { out.push('</ul>'); inUl = false; }
      if (!inOl) { out.push('<ol>'); inOl = true; }
      out.push(`<li>${inline(m[1])}</li>`);
    } else if (line === '') {
      flushPara(); flushList();
    } else {
      if (inUl || inOl) flushList();
      para.push(inline(line));
    }
  }
  flushPara(); flushList();
  return out.join('\n');
}

function renderTranscript(text) {
  return text
    .replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;')
    .replace(/(\[\d{2}:\d{2}:\d{2}(?:\.\d+)?\])/g, '<span class="ts">$1</span>');
}

function copyContent() {
  const content = _meetingData[_activeTab];
  if (content) navigator.clipboard.writeText(content);
}

// ============================================================
// Delete confirmation overlay
// ============================================================
let _pendingDeleteSlug = null;

function openDeleteConfirm(e, slug) {
  e.stopPropagation();
  _pendingDeleteSlug = slug;
  const title = slug.replace(/_/g, ' ');
  document.getElementById('confirm-msg').textContent =
    `Встреча «${title}» будет удалена целиком (запись, транскрипция, протокол). Это действие нельзя отменить.`;
  document.getElementById('confirm-overlay').classList.add('visible');
}

function closeConfirm() {
  document.getElementById('confirm-overlay').classList.remove('visible');
  _pendingDeleteSlug = null;
}

async function confirmDelete() {
  const slug = _pendingDeleteSlug;
  closeConfirm();
  if (!slug) return;
  await fetch('/api/meeting/delete', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ folder: slug, what: 'all' }),
  });
  if (slug === _selectedSlug) showHome();
  fetchMeetings();
}

// ============================================================
// Pipeline preview toggles
// ============================================================
function togglePipeStep(step) {
  const chk = document.getElementById('pipe-chk-' + step);
  const stepEl = document.getElementById('pipe-' + step);
  if (step === 'transcribe' && !chk.checked) {
    document.getElementById('pipe-chk-protocol').checked = false;
    document.getElementById('pipe-toggle-protocol').classList.add('disabled');
    document.getElementById('pipe-protocol').classList.add('skipped');
    stepEl.classList.add('skipped');
  } else if (step === 'transcribe' && chk.checked) {
    document.getElementById('pipe-toggle-protocol').classList.remove('disabled');
    stepEl.classList.remove('skipped');
  } else if (step === 'protocol') {
    document.getElementById('pipe-protocol').classList.toggle('skipped', !chk.checked);
  }
}

// ============================================================
// Regenerate protocol from viewer
// ============================================================
let evtSrc = null;

async function regenProtocol() {
  if (!_selectedSlug) return;
  if (evtSrc) evtSrc.close();
  document.getElementById('btn-regen').disabled = true;
  if (evtSrc) evtSrc.close();
  evtSrc = new EventSource('/api/process/stream');
  evtSrc.onmessage = () => {};
  evtSrc.addEventListener('done', async () => {
    evtSrc.close();
    document.getElementById('btn-regen').disabled = false;
    const r = await fetch('/api/meeting?folder=' + encodeURIComponent(_selectedSlug));
    _meetingData = await r.json();
    showTab('protocol');
    fetchMeetings();
  });
  evtSrc.onerror = () => {
    evtSrc.close();
    document.getElementById('btn-regen').disabled = false;
  };
  await fetch('/api/process/start', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ folder: _selectedSlug, from_transcript: true }),
  });
}

// ============================================================
// UI helpers (settings accordion)
// ============================================================
function toggleSection(head) {
  head.classList.toggle('open');
  head.nextElementSibling.classList.toggle('open');
}


function toggleVad() {
  document.getElementById('vad-params').style.display =
    document.getElementById('cfg-vad').checked ? '' : 'none';
}

function toggleKey(id) {
  const el = document.getElementById(id || 'cfg-key');
  el.type = el.type === 'password' ? 'text' : 'password';
}

const _KEY_NAMES = {
  'cfg-key': 'anthropic_api_key',
  'cfg-gemini-key': 'gemini_api_key',
  'cfg-mistral-key': 'mistral_api_key',
};
const _SOURCE_LABELS = { env: 'из ENV', keyring: 'из keyring', none: 'не задан' };
const _SOURCE_MOD = { env: 'env', keyring: 'keyring', none: 'none' };

function setApiKeyField(id, info) {
  const input = document.getElementById(id);
  const container = document.getElementById(id + '-info');
  input.value = '';
  if (!container) return;
  if (!info || info.source === 'none') {
    container.innerHTML = '<span class="key-chip key-chip--none">не задан</span>'
      + '<span class="key-actions">'
      + '<button class="key-act-btn" onclick="testKey(\'' + id + '\')" disabled>Проверить</button>'
      + '</span>';
    return;
  }
  const mod = _SOURCE_MOD[info.source] || 'none';
  const label = _SOURCE_LABELS[info.source] || info.source;
  const masked = info.masked ? `<span class="key-masked">${info.masked}</span>` : '';
  const canDelete = info.source === 'keyring';
  const delBtn = `<button class="key-act-btn danger" onclick="deleteKey('${id}')"${canDelete ? '' : ' disabled'}>Удалить</button>`;
  const testBtn = `<button class="key-act-btn" onclick="testKey('${id}')">Проверить</button>`;
  const warn = info.env_shadows_keyring
    ? `<div class="key-shadow-warn">⚠ env-переменная перекрывает keyring</div>` : '';
  container.innerHTML = `<span class="key-chip key-chip--${mod}">${label}</span>${masked}`
    + `<span class="key-actions">${testBtn}${delBtn}</span>${warn}`;
}

function clearKeyInfo(id) {
  const container = document.getElementById(id + '-info');
  if (container) container.innerHTML = '';
}

async function deleteKey(id) {
  const name = _KEY_NAMES[id];
  if (!name) return;
  if (!confirm('Удалить ключ из keyring?')) return;
  const r = await fetch(`/api/config/api_key/${name}`, { method: 'DELETE' });
  const data = await r.json();
  if (data.ok) { clearKeyInfo(id); fetchConfig(); }
  else alert('Ошибка: ' + (data.detail || data.error || 'unknown'));
}

async function testKey(id) {
  const name = _KEY_NAMES[id];
  if (!name) return;
  const container = document.getElementById(id + '-info');
  const btn = container && container.querySelector('.key-act-btn:not(.danger)');
  if (btn) { btn.textContent = '…'; btn.disabled = true; }
  try {
    const r = await fetch(`/api/config/api_key/${name}/test`, { method: 'POST' });
    const data = await r.json();
    const msg = data.ok
      ? '<span class="key-test-ok">✓ работает</span>'
      : `<span class="key-test-fail">✗ ${data.error || 'ошибка'}</span>`;
    if (container) {
      const existing = container.querySelector('.key-test-ok,.key-test-fail');
      if (existing) existing.remove();
      container.insertAdjacentHTML('beforeend', msg);
    }
  } finally {
    if (btn) { btn.textContent = 'Проверить'; btn.disabled = false; }
  }
}

function toggleProviderFields() {
  const p = document.getElementById('cfg-provider').value;
  document.getElementById('claude-fields').style.display = p === 'claude' ? '' : 'none';
  document.getElementById('gemini-fields').style.display = p === 'gemini' ? '' : 'none';
  document.getElementById('mistral-fields').style.display = p === 'mistral' ? '' : 'none';
}

function switchKind(kind) {
  document.getElementById('kind-cloud').style.display = kind === 'cloud' ? '' : 'none';
  document.getElementById('kind-local').style.display = kind === 'local' ? '' : 'none';
  document.getElementById('tab-cloud').classList.toggle('active', kind === 'cloud');
  document.getElementById('tab-local').classList.toggle('active', kind === 'local');
  if (kind === 'local') checkOllamaStatus();
}

async function checkOllamaStatus() {
  const base = document.getElementById('cfg-ollama-url').value || 'http://localhost:11434/v1';
  document.getElementById('ollama-status-text').textContent = 'Проверяю...';
  const r = await fetch('/api/ollama/status?base_url=' + encodeURIComponent(base));
  const d = await r.json();
  const dot = document.getElementById('ollama-dot');
  const txt = document.getElementById('ollama-status-text');
  if (d.ok) {
    dot.className = 'dot-ok';
    txt.textContent = `Ollama запущена · моделей установлено: ${d.models.length}`;
    const dl = document.getElementById('ollama-datalist');
    dl.innerHTML = d.models.map(m => `<option value="${m}">`).join('');
  } else {
    dot.className = 'dot-err';
    txt.textContent = 'Ollama не отвечает — запусти: ollama serve';
  }
}

function flashSave(id) {
  const el = document.getElementById(id);
  if (!el) return;
  el.style.display = 'inline';
  setTimeout(() => el.style.display = 'none', 2000);
}

// ============================================================
// Compute types
// ============================================================
const COMPUTE_TYPES = {
  cpu:  [['int8', 'int8 (быстро)'], ['float32', 'float32']],
  cuda: [['float16', 'float16 (рекомендуется)'], ['int8_float16', 'int8_float16'], ['float32', 'float32']],
};

function updateComputeTypes() {
  const device = document.getElementById('cfg-device').value;
  const ct = document.getElementById('cfg-compute');
  const prev = ct.value;
  ct.innerHTML = '';
  COMPUTE_TYPES[device].forEach(([v, l]) => ct.add(new Option(l, v)));
  if (COMPUTE_TYPES[device].find(([v]) => v === prev)) ct.value = prev;
}

function populateSelect(id, items) {
  const el = document.getElementById(id);
  el.innerHTML = items.map(m => `<option value="${m}">${m}</option>`).join('');
}

function setSelectValue(id, val) {
  const el = document.getElementById(id);
  if (val) el.value = val;
}

// ============================================================
// Load config
// ============================================================
async function fetchConfig() {
  const [mr, cr] = await Promise.all([fetch('/api/models'), fetch('/api/config')]);
  const models = await mr.json();
  const c = await cr.json();
  const t = c.transcription || {}, p = c.protocol || {}, a = c.api || {}, rec = c.recording || {}, st = c.storage || {};
  document.getElementById('storage-dir').value = st.meetings_dir || _DEFAULT_STORAGE_DIR;
  validateStoragePath();
  document.getElementById('cfg-model').value = t.model || 'medium';
  document.getElementById('cfg-lang').value = t.language || 'ru';
  document.getElementById('cfg-device').value = t.device || 'cpu';
  updateComputeTypes();
  document.getElementById('cfg-compute').value = t.compute_type || 'int8';
  document.getElementById('cfg-beam').value = t.beam_size || 5;
  document.getElementById('cfg-vad').checked = t.vad_filter !== false;
  document.getElementById('cfg-silence').value = t.min_silence_duration_ms || 500;
  document.getElementById('cfg-pad').value = t.speech_pad_ms || 200;
  const cloud = models.cloud || models;
  const local = models.local || {};
  populateSelect('cfg-claude', cloud.claude || []);
  populateSelect('cfg-gemini', cloud.gemini || []);
  populateSelect('cfg-mistral', cloud.mistral || []);
  document.getElementById('cfg-provider').value = p.provider || 'claude';
  setSelectValue('cfg-claude', p.claude_model);
  setSelectValue('cfg-gemini', p.gemini_model);
  setSelectValue('cfg-mistral', p.mistral_model);
  const tokensEl = document.getElementById('cfg-tokens');
  tokensEl.value = p.max_tokens || 8192;
  if (!tokensEl.value) tokensEl.value = 8192;
  setApiKeyField('cfg-key', a.anthropic_api_key);
  setApiKeyField('cfg-gemini-key', a.gemini_api_key);
  setApiKeyField('cfg-mistral-key', a.mistral_api_key);
  const ollamaModels = local.ollama || [];
  const dl = document.getElementById('ollama-datalist');
  dl.innerHTML = ollamaModels.map(m => `<option value="${m}">`).join('');
  document.getElementById('cfg-ollama-model').value = p.ollama_model || ollamaModels[0] || '';
  document.getElementById('cfg-ollama-url').value = p.ollama_base_url || 'http://localhost:11434/v1';
  switchKind(p.kind || 'cloud');
  toggleProviderFields();
  toggleVad();
  window._savedMic = rec.mic_source || '';
  window._savedSys = rec.system_source || '';
  document.getElementById('prepend-date').checked = rec.prepend_date !== false;
  updatePipeDesc(t, p);
}

function updatePipeDesc(t, p) {
  const trDesc = `faster-whisper · ${t.model || 'medium'} · ${t.language || 'ru'} · ${(t.device || 'cpu').toUpperCase()}`;
  const prEl = document.getElementById('pipe-transcribe-desc');
  if (prEl) prEl.textContent = trDesc;
  const protoEl = document.getElementById('pipe-protocol-desc');
  if (protoEl) {
    const provider = p.provider || 'claude';
    const model = p[provider + '_model'] || '';
    const tmpl = p.active_template || '—';
    protoEl.textContent = `${provider} · ${model} · шаблон: ${tmpl}`;
  }
}

// ============================================================
// Audio sources
// ============================================================
async function fetchSources() {
  const r = await fetch('/api/sources');
  const d = await r.json();
  const ms = document.getElementById('mic-src');
  const ss = document.getElementById('sys-src');
  d.mics.forEach(s => ms.add(new Option(s, s)));
  d.sinks.forEach(s => ss.add(new Option(s, s)));
  if (window._savedMic) ms.value = window._savedMic;
  if (window._savedSys) ss.value = window._savedSys;
}

// ============================================================
// Recordings list (for process view)
// ============================================================
async function fetchRecordings() {
  const r = await fetch('/api/recordings');
  const d = await r.json();
  const sel = document.getElementById('proc-folder');
  sel.innerHTML = '<option value="">— выбери встречу —</option>';
  d.folders.forEach(f => sel.add(new Option(f, f)));
}

// ============================================================
// Recording
// ============================================================
let recTimer = null;
let _recFolder = null;

async function startRec() {
  const name = document.getElementById('rec-name').value.trim();
  const prependDate = document.getElementById('prepend-date').checked;
  await fetch('/api/config', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ recording: {
      mic_source: document.getElementById('mic-src').value,
      system_source: document.getElementById('sys-src').value,
      prepend_date: prependDate,
    }}),
  });
  const r = await fetch('/api/record/start', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name, prepend_date: prependDate }),
  });
  const d = await r.json();
  if (d.error) { alert(d.error); return; }
  _recFolder = d.folder;
  document.getElementById('btn-rec-start').disabled = true;
  document.getElementById('btn-rec-stop').disabled = false;
  const btnRec = document.getElementById('btn-record');
  btnRec.classList.add('recording');
  document.getElementById('rec-btn-icon').textContent = '■';
  document.getElementById('rec-btn-label').textContent = 'Запись...';
  btnRec.onclick = () => showView('record');
  recTimer = setInterval(pollRec, 1000);
}

async function stopRec() {
  await fetch('/api/record/stop', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name: _recFolder || '' }),
  });
  clearInterval(recTimer);
  document.getElementById('btn-rec-start').disabled = false;
  document.getElementById('btn-rec-stop').disabled = true;
  const btnRec = document.getElementById('btn-record');
  btnRec.classList.remove('recording');
  document.getElementById('rec-btn-icon').textContent = '●';
  document.getElementById('rec-btn-label').textContent = 'Новая запись';
  btnRec.onclick = toggleRecord;
  document.getElementById('rec-st').textContent = 'Остановлено';
  if (recTimer) { clearInterval(recTimer); recTimer = null; }

  // Auto-pipeline based on toggles
  const doTranscribe = document.getElementById('pipe-chk-transcribe').checked;
  const doProtocol = document.getElementById('pipe-chk-protocol').checked;

  if (!doTranscribe) {
    setStep('save', 'done');
    setStep('tr', 'skip');
    setStep('pr', 'skip');
    document.getElementById('log-wrap').style.display = '';
    setBar(100);
    fetchMeetings();
    return;
  }

  document.getElementById('log-wrap').style.display = '';
  resetProgress();
  setStep('save', 'active');
  setBar(5);

  const folder = _recFolder;
  if (evtSrc) evtSrc.close();
  evtSrc = new EventSource('/api/process/stream');
  evtSrc.onmessage = e => {
    const ev = JSON.parse(e.data);
    _appendLogLine(document.getElementById('log-output'), ev);
    handleStageEvent(ev.stage);
  };
  evtSrc.addEventListener('done', () => {
    evtSrc.close();
    if (doProtocol) {
      setStep('pr', 'done');
      document.getElementById('pconn-2').style.width = '100%';
    } else {
      setStep('tr', 'done');
      setStep('pr', 'skip');
    }
    setBar(100);
    setTimeout(() => fetchMeetings(), 500);
  });
  evtSrc.onerror = () => {
    evtSrc.close();
    document.getElementById('rec-st').textContent = 'Ошибка';
    document.getElementById('rec-st').className = 'rec-status';
  };

  await fetch('/api/process/start', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      folder,
      no_protocol: !doProtocol,
    }),
  });
}

function handleStageEvent(stage) {
  if (stage === 'save') {
    setStep('save', 'active'); setBar(5);
  } else if (stage === 'transcribe') {
    setStep('save', 'done'); document.getElementById('pconn-1').style.width = '100%';
    setStep('tr', 'active'); setBar(30);
  } else if (stage === 'protocol') {
    setStep('tr', 'done'); document.getElementById('pconn-2').style.width = '100%';
    setStep('pr', 'active'); setBar(70);
  }
}

async function pollRec() {
  const r = await fetch('/api/record/status');
  const d = await r.json();
  const st = document.getElementById('rec-st');
  if (d.running) {
    const m = String(Math.floor(d.elapsed / 60)).padStart(2, '0');
    const s = String(d.elapsed % 60).padStart(2, '0');
    st.innerHTML = `<span class="dot"></span><span class="on">Запись идёт ${m}:${s}</span>`;
    st.className = 'rec-status on';
  } else {
    st.textContent = d.folder ? `Записано: ${d.folder}` : 'Готово к записи';
    st.className = 'rec-status';
    if (recTimer) { clearInterval(recTimer); recTimer = null; }
  }
}

// ============================================================
// Pipeline progress helpers
// ============================================================
function resetProgress() {
  ['save', 'tr', 'pr'].forEach(id => {
    const c = document.getElementById('pstep-' + id);
    const l = document.getElementById('pstep-' + id + '-lbl');
    if (c) { c.className = 'pipe-step-circle'; c.textContent = id === 'save' ? '1' : id === 'tr' ? '2' : '3'; }
    if (l) { l.className = 'pipe-step-label'; }
  });
  document.getElementById('pconn-1').style.width = '0%';
  document.getElementById('pconn-2').style.width = '0%';
  setBar(0);
}

function setStep(id, state) {
  const c = document.getElementById('pstep-' + id);
  const l = document.getElementById('pstep-' + id + '-lbl');
  if (!c) return;
  if (state === 'done') {
    c.className = 'pipe-step-circle done'; c.textContent = '✓';
    if (l) l.className = 'pipe-step-label done';
  } else if (state === 'active') {
    c.className = 'pipe-step-circle active';
    if (l) l.className = 'pipe-step-label active';
  } else if (state === 'skip') {
    c.className = 'pipe-step-circle skip';
    if (l) l.className = 'pipe-step-label skip';
  }
}

function setBar(pct) {
  document.getElementById('pipe-bar').style.width = pct + '%';
  document.getElementById('pipe-bar-pct').textContent = pct + '%';
}

// ============================================================
// Processing (process view)
// ============================================================
async function startProc() {
  const sel = document.getElementById('proc-folder');
  const folder = sel.value;
  const errEl = document.getElementById('proc-folder-err');
  if (!folder) {
    sel.classList.add('input-err');
    if (errEl) errEl.style.display = '';
    setTimeout(() => { sel.classList.remove('input-err'); if (errEl) errEl.style.display = 'none'; }, 3000);
    return;
  }
  sel.classList.remove('input-err');
  if (errEl) errEl.style.display = 'none';

  const name = document.getElementById('proc-name').value.trim();
  const model = document.getElementById('cfg-model').value;
  const logWrap = document.getElementById('proc-log-wrap');
  const out = document.getElementById('proc-out');
  const st = document.getElementById('proc-st');
  const btn = document.getElementById('btn-proc');

  logWrap.style.display = '';
  out.innerHTML = '';
  btn.disabled = true;
  st.textContent = 'Обработка...'; st.className = 'rec-status on';

  if (evtSrc) evtSrc.close();
  evtSrc = new EventSource('/api/process/stream');
  evtSrc.onmessage = e => { _appendLogLine(out, JSON.parse(e.data)); };
  evtSrc.addEventListener('done', () => {
    evtSrc.close();
    btn.disabled = false;
    st.textContent = 'Готово ✓'; st.className = 'rec-status';
    setTimeout(() => { st.textContent = ''; st.className = 'rec-status'; }, 3000);
    fetchMeetings();
  });
  evtSrc.onerror = () => {
    st.textContent = 'Ошибка соединения'; st.className = 'rec-status';
    btn.disabled = false;
  };

  const noProtocol = document.getElementById('only-transcript').checked;
  const r = await fetch('/api/process/start', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ folder, name, model, no_protocol: noProtocol }),
  });
  const d = await r.json();
  if (!d.ok) {
    evtSrc.close();
    btn.disabled = false;
    st.textContent = d.detail || 'Ошибка запуска'; st.className = 'rec-status';
    logWrap.style.display = 'none';
  }
}

// ============================================================
// SSE log helper
// ============================================================
function _appendLogLine(out, ev) {
  const line = ev.message || '';
  if (!line.trim()) return;
  const div = document.createElement('div');
  if (ev.stage === 'error' || line.startsWith('✗') || line.startsWith('Ошибка')) div.className = 'log-err';
  else if (line.startsWith('✓')) div.className = 'log-ok';
  div.textContent = line;
  out.appendChild(div);
  out.scrollTop = out.scrollHeight;
}

// ============================================================
// Save config
// ============================================================
async function saveConfig() {
  await fetch('/api/config', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      transcription: {
        model: document.getElementById('cfg-model').value,
        language: document.getElementById('cfg-lang').value,
        device: document.getElementById('cfg-device').value,
        compute_type: document.getElementById('cfg-compute').value,
        beam_size: +document.getElementById('cfg-beam').value,
        vad_filter: document.getElementById('cfg-vad').checked,
        min_silence_duration_ms: +document.getElementById('cfg-silence').value,
        speech_pad_ms: +document.getElementById('cfg-pad').value,
      },
      recording: {
        mic_source: document.getElementById('mic-src').value,
        system_source: document.getElementById('sys-src').value,
      },
    }),
  });
  flashSave('save-flash-t');
  fetchConfig();
}

async function saveProtocolConfig() {
  const kind = document.getElementById('tab-cloud').classList.contains('active') ? 'cloud' : 'local';
  const key = document.getElementById('cfg-key').value;
  const gkey = document.getElementById('cfg-gemini-key').value;
  await fetch('/api/config', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      protocol: {
        kind,
        provider: document.getElementById('cfg-provider').value,
        claude_model: document.getElementById('cfg-claude').value,
        gemini_model: document.getElementById('cfg-gemini').value,
        mistral_model: document.getElementById('cfg-mistral').value,
        ollama_model: document.getElementById('cfg-ollama-model').value,
        ollama_base_url: document.getElementById('cfg-ollama-url').value || 'http://localhost:11434/v1',
        max_tokens: +document.getElementById('cfg-tokens').value,
        active_template: _activeTmpl,
      },
      api: {
        anthropic_api_key: key || '***',
        gemini_api_key: gkey || '***',
        mistral_api_key: document.getElementById('cfg-mistral-key').value || '***',
      },
    }),
  });
  flashSave('save-flash-p');
}

// ============================================================
// Storage settings
// ============================================================
const _DEFAULT_STORAGE_DIR = '~/Documents/meetings';
let _storageValidateTimer = null;

function onStorageDirInput() {
  clearTimeout(_storageValidateTimer);
  _storageValidateTimer = setTimeout(validateStoragePath, 300);
}

async function validateStoragePath() {
  const val = document.getElementById('storage-dir').value.trim();
  const resolved = document.getElementById('storage-resolved');
  const status = document.getElementById('storage-status');
  if (!val) { resolved.textContent = ''; status.textContent = ''; return; }
  try {
    const r = await fetch('/api/storage/validate?path=' + encodeURIComponent(val));
    const d = await r.json();
    resolved.textContent = 'Резолв: ' + (d.resolved || '');
    if (d.error) {
      status.innerHTML = '<span style="color:var(--error)">✗ ' + d.error + '</span>';
    } else if (d.exists) {
      status.innerHTML = '<span style="color:var(--success)">✓ путь существует, доступен на запись</span>';
    } else {
      status.innerHTML = '<span style="color:var(--success)">✓ будет создан</span>';
    }
  } catch {
    status.innerHTML = '<span style="color:var(--error)">✗ ошибка проверки</span>';
  }
}

function resetStorageDir() {
  document.getElementById('storage-dir').value = _DEFAULT_STORAGE_DIR;
  validateStoragePath();
}

async function saveStorageDir() {
  const val = document.getElementById('storage-dir').value.trim() || _DEFAULT_STORAGE_DIR;
  const r = await fetch('/api/config', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ storage: { meetings_dir: val } }),
  });
  const d = await r.json();
  if (d.ok) {
    flashSave('save-flash-storage');
    fetchMeetings();
  } else {
    alert('Ошибка: ' + (d.detail || d.error || 'unknown'));
  }
}

// ============================================================
// Templates
// ============================================================
let _templates = [], _activeTmpl = '';

async function fetchTemplates() {
  const r = await fetch('/api/templates');
  const d = await r.json();
  _templates = d.templates || [];
  _activeTmpl = d.active || '';
  renderTmplSelect();
  loadTmpl(_activeTmpl);
}

function renderTmplSelect() {
  const sel = document.getElementById('tmpl-select');
  sel.innerHTML = '';
  sel.add(new Option('— без шаблона —', '', '', _activeTmpl === ''));
  _templates.forEach(t => sel.add(new Option(t.name, t.name, t.name === _activeTmpl, t.name === _activeTmpl)));
}

function loadTmpl(name) {
  const t = _templates.find(t => t.name === name);
  const area = document.getElementById('tmpl-prompt');
  area.value = t ? t.prompt : '';
  area.disabled = !name;
  _activeTmpl = name;
}

function onTmplSelect() { loadTmpl(document.getElementById('tmpl-select').value); }

async function saveTemplate() {
  const name = document.getElementById('tmpl-select').value;
  const prompt = document.getElementById('tmpl-prompt').value;
  const idx = _templates.findIndex(t => t.name === name);
  if (idx >= 0) _templates[idx].prompt = prompt;
  await fetch('/api/template/save', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name, prompt, active: _activeTmpl }),
  });
  flashSave('save-flash-tmpl');
}

async function addTemplate() {
  const name = prompt('Название нового шаблона:');
  if (!name || !name.trim()) return;
  const base = _templates[0]?.prompt || '';
  _templates.push({ name: name.trim(), prompt: base });
  _activeTmpl = name.trim();
  await fetch('/api/template/save', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name: name.trim(), prompt: base, active: _activeTmpl }),
  });
  renderTmplSelect();
  document.getElementById('tmpl-select').value = _activeTmpl;
  loadTmpl(_activeTmpl);
}

async function deleteTemplate() {
  const name = document.getElementById('tmpl-select').value;
  if (!name) { alert('Нечего удалять'); return; }
  await fetch('/api/template/delete', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name }),
  });
  _templates = _templates.filter(t => t.name !== name);
  _activeTmpl = _templates[0]?.name || '';
  renderTmplSelect();
  loadTmpl(_activeTmpl);
}

// ============================================================
// Search
// ============================================================
let _searchTimer = null;

function onSearch(q) {
  clearTimeout(_searchTimer);
  if (!q.trim()) { fetchMeetings(); return; }
  _searchTimer = setTimeout(() => _runSearch(q), 300);
}

async function _runSearch(q) {
  const r = await fetch('/api/search?q=' + encodeURIComponent(q));
  if (!r.ok) return;
  const results = await r.json();
  const list = document.getElementById('meetings-list');
  if (!results.length) {
    list.innerHTML = '<div class="sidebar-section">Встречи</div><div class="no-meetings">Ничего не найдено</div>';
    return;
  }
  const sectionHtml = '<div class="sidebar-section">Встречи</div>';
  const itemsHtml = results.map(res => {
    const snippet = res.snippet
      ? `<div style="font-size:.68rem;color:var(--text-muted);white-space:nowrap;overflow:hidden;text-overflow:ellipsis">${res.snippet}</div>`
      : '';
    return `<div class="meeting-item" data-slug="${res.slug}" onclick="selectMeeting('${res.slug}')">
      <div class="meeting-item-body">
        <div class="title">${res.title}</div>
        ${snippet}
      </div>
      <button class="del-btn" onclick="openDeleteConfirm(event,'${res.slug}')">✕</button>
    </div>`;
  }).join('');
  list.innerHTML = sectionHtml + itemsHtml;
  if (_selectedSlug) {
    const active = list.querySelector(`[data-slug="${_selectedSlug}"]`);
    if (active) active.classList.add('active');
  }
}

// ============================================================
// Upload
// ============================================================
let _uploadFile = null;

function onDropOver(e) {
  e.preventDefault();
  document.getElementById('upload-drop-zone').classList.add('dragover');
}

function onDropLeave() {
  document.getElementById('upload-drop-zone').classList.remove('dragover');
}

function onDrop(e) {
  e.preventDefault();
  document.getElementById('upload-drop-zone').classList.remove('dragover');
  const f = e.dataTransfer.files[0];
  if (f) _setUploadFile(f);
}

function onFileSelected(input) {
  if (input.files[0]) _setUploadFile(input.files[0]);
}

function _setUploadFile(f) {
  _uploadFile = f;
  const mb = (f.size / 1048576).toFixed(1);
  document.getElementById('upload-drop-text').innerHTML =
    `<strong>${f.name}</strong><br><span style="font-size:.75rem;color:var(--text-muted)">${mb} МБ</span>`;
  document.getElementById('btn-upload-start').disabled = false;
}

async function startUpload() {
  if (!_uploadFile) {
    document.getElementById('upload-st').textContent = 'Выбери файл';
    return;
  }
  const title = document.getElementById('upload-title').value.trim();
  const doProtocol = document.getElementById('pipe-chk-protocol').checked;
  const doTranscribe = document.getElementById('pipe-chk-transcribe').checked;

  const btn = document.getElementById('btn-upload-start');
  const st = document.getElementById('upload-st');
  btn.disabled = true;
  st.textContent = 'Загрузка...';

  const fd = new FormData();
  fd.append('file', _uploadFile);
  fd.append('title', title);
  fd.append('auto_process', doTranscribe ? 'true' : 'false');
  fd.append('no_protocol', doProtocol ? 'false' : 'true');

  // Show progress panel
  document.getElementById('log-wrap').style.display = '';
  resetProgress();
  setStep('save', 'active');
  setBar(5);

  let uploadSlug = null;

  // Upload progress via XHR
  await new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open('POST', '/api/upload');
    xhr.upload.onprogress = e => {
      if (e.lengthComputable) {
        const pct = Math.round((e.loaded / e.total) * 25);
        setBar(pct);
        st.textContent = `Загрузка ${Math.round(e.loaded / e.total * 100)}%`;
      }
    };
    xhr.onload = () => {
      if (xhr.status >= 200 && xhr.status < 300) {
        const d = JSON.parse(xhr.responseText);
        uploadSlug = d.slug;
        resolve(d);
      } else {
        let msg = 'Ошибка загрузки';
        try { msg = JSON.parse(xhr.responseText).detail || msg; } catch {}
        reject(new Error(msg));
      }
    };
    xhr.onerror = () => reject(new Error('Ошибка соединения'));
    xhr.send(fd);
  }).catch(err => {
    st.textContent = err.message;
    btn.disabled = false;
    return null;
  });

  if (!uploadSlug) return;

  setStep('save', 'done');
  document.getElementById('pconn-1').style.width = '100%';
  st.textContent = 'Обработка...';

  if (!doTranscribe) {
    setStep('tr', 'skip'); setStep('pr', 'skip'); setBar(100);
    st.textContent = 'Готово';
    btn.disabled = false;
    fetchMeetings();
    return;
  }

  if (evtSrc) evtSrc.close();
  evtSrc = new EventSource('/api/process/stream');
  evtSrc.onmessage = e => {
    const ev = JSON.parse(e.data);
    _appendLogLine(document.getElementById('log-output'), ev);
    handleStageEvent(ev.stage);
  };
  evtSrc.addEventListener('done', async () => {
    evtSrc.close();
    if (doProtocol) {
      setStep('pr', 'done'); document.getElementById('pconn-2').style.width = '100%';
    } else {
      setStep('tr', 'done'); setStep('pr', 'skip');
    }
    setBar(100);
    st.textContent = 'Готово ✓';
    btn.disabled = false;
    _uploadFile = null;
    document.getElementById('upload-drop-text').innerHTML =
      'Перетащите аудиофайл сюда<br><span style="font-size:.75rem;color:var(--text-muted)">или нажмите для выбора</span>';
    await fetchMeetings();
    selectMeeting(uploadSlug);
  });
  evtSrc.onerror = () => {
    evtSrc.close();
    st.textContent = 'Ошибка SSE';
    btn.disabled = false;
  };
}

// ============================================================
// Init
// ============================================================
fetchConfig().then(fetchSources);
fetchMeetings();
fetchTemplates();
