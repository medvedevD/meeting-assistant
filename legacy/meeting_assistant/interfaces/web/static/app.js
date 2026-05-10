// ============================================================
// Error-kind UX banners
// ============================================================
const ERROR_KIND_UI = {
  audio_corrupt:      { msg: 'Файл повреждён и не может быть обработан.',
                        actions: [{ label: '↑ Загрузить заново', cb: () => openNewMeetingDialog('upload') }] },
  audio_too_short:    { msg: 'Запись слишком короткая (менее 5 сек) — нечего транскрибировать.',
                        actions: [] },
  audio_silent:       { msg: 'В записи только тишина. Проверьте источники звука в настройках.',
                        actions: [{ label: '⚙ Настройки', cb: showSettings }] },
  transcription_oom:  { msg: 'Недостаточно памяти для модели. Выберите модель меньшего размера.',
                        actions: [{ label: '⚙ Настройки', cb: showSettings }] },
  api_quota:          { msg: 'Превышен лимит API. Задача будет повторена автоматически.',
                        actions: [] },
  api_auth:           { msg: 'Неверный API-ключ. Проверьте настройки.',
                        actions: [{ label: '⚙ Настройки', cb: showSettings }] },
  network_timeout:    { msg: 'Таймаут сети. Задача будет повторена автоматически.',
                        actions: [] },
  worker_killed:      { msg: 'Процесс был прерван. Задача автоматически перезапустится.',
                        actions: [] },
  template_render:    { msg: 'Ошибка в шаблоне протокола.',
                        actions: [{ label: '⚙ Настройки', cb: showSettings }] },
};

function showErrorBanner(bannerId, error_kind, raw_message, retryFn) {
  const banner  = document.getElementById(bannerId + '-banner');
  const msgEl   = document.getElementById(bannerId + '-msg');
  const actEl   = document.getElementById(bannerId + '-actions');
  if (!banner) return;

  const ui = ERROR_KIND_UI[error_kind];
  msgEl.textContent = ui ? ui.msg : (raw_message || 'Произошла ошибка.');
  actEl.innerHTML = '';

  const actions = ui ? ui.actions : [];
  if (retryFn) actions.push({ label: '↺ Повторить', cb: retryFn });
  actions.forEach(a => {
    const btn = document.createElement('button');
    btn.className = 'btn btn-ghost';
    btn.textContent = a.label;
    if (a.cb) btn.onclick = a.cb;
    actEl.appendChild(btn);
  });

  banner.style.display = 'block';
}

function hideErrorBanner(bannerId) {
  const el = document.getElementById(bannerId + '-banner');
  if (el) el.style.display = 'none';
}

// ============================================================
// Toast notifications
// ============================================================
function showToast(message, type = 'info', durationMs = 4000) {
  const container = document.getElementById('toast-container');
  if (!container) return;
  const el = document.createElement('div');
  el.className = 'toast' + (type === 'error' ? ' toast-error' : type === 'success' ? ' toast-success' : '');
  el.textContent = message;
  container.appendChild(el);
  setTimeout(() => {
    el.classList.add('toast-out');
    el.addEventListener('animationend', () => el.remove(), { once: true });
  }, durationMs);
}

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
  _stopMeetingSSE();
  _stopRecPolling();
  document.querySelectorAll('.meeting-item').forEach(i => i.classList.remove('active'));
  showView('home');
}

function showSettings() {
  closeNewMeetingDialog();
  showView('settings');
}

function closeViewer() { showHome(); }

// ============================================================
// New Meeting Dialog
// ============================================================
let _dialogTab = 'record';
let _dlgUploadFile = null;

function openNewMeetingDialog(tab) {
  _dlgUploadFile = null;
  _dialogTab = tab || 'record';
  switchDialogTab(_dialogTab);
  _populateDlgSources();
  _populateDlgFolders();
  _updateDlgPipeControls();
  document.getElementById('new-meeting-overlay').classList.add('open');
  document.getElementById('dlg-warning').style.display = 'none';

  // Set start button label based on tab
  _updateDlgStartBtn();
}

function closeNewMeetingDialog() {
  document.getElementById('new-meeting-overlay').classList.remove('open');
}

function switchDialogTab(tab) {
  _dialogTab = tab;
  ['record', 'upload', 'folder'].forEach(t => {
    document.getElementById('dialog-tab-' + t).style.display = t === tab ? '' : 'none';
    document.getElementById('msrc-' + t).classList.toggle('active', t === tab);
  });
  // Pipeline section only shown for record/upload; for "from folder" the pipeline is always full
  _updateDlgStartBtn();
}

function _updateDlgStartBtn() {
  const btn = document.getElementById('dlg-btn-start');
  if (_dialogTab === 'record') btn.textContent = '● Начать запись';
  else if (_dialogTab === 'upload') btn.textContent = '↑ Загрузить';
  else btn.textContent = '▶ Обработать';
}

async function _populateDlgSources() {
  try {
    const r = await fetch('/api/sources');
    const d = await r.json();
    const ms = document.getElementById('dlg-mic-src');
    const ss = document.getElementById('dlg-sys-src');
    ms.innerHTML = '<option value="">— по умолчанию —</option>';
    ss.innerHTML = '<option value="">— по умолчанию —</option>';
    d.mics.forEach(s => ms.add(new Option(s, s)));
    d.sinks.forEach(s => ss.add(new Option(s, s)));
    if (window._savedMic && ms.querySelector(`option[value="${window._savedMic}"]`)) ms.value = window._savedMic;
    if (window._savedSys && ss.querySelector(`option[value="${window._savedSys}"]`)) ss.value = window._savedSys;
    checkDlgMicMute();
  } catch (_) {}
}

async function _populateDlgFolders() {
  try {
    const r = await fetch('/api/recordings');
    const d = await r.json();
    const sel = document.getElementById('dlg-folder-select');
    sel.innerHTML = '<option value="">— выбери встречу —</option>';
    d.folders.forEach(f => sel.add(new Option(f.replace(/_/g, ' '), f)));
  } catch (_) {}
}

function _updateDlgPipeControls() {
  const t = _lastConfig.transcription || {};
  const p = _lastConfig.protocol || {};

  const modelSel = document.getElementById('dlg-whisper-model');
  if (modelSel && t.model) modelSel.value = t.model;

  const providerSel = document.getElementById('dlg-provider');
  if (providerSel) {
    providerSel.value = p.provider || 'claude';
    _populateDlgLlmModels(providerSel.value);
  }

  const tmplSel = document.getElementById('dlg-template');
  if (tmplSel) {
    const active = p.active_template || _activeTmpl || '';
    tmplSel.innerHTML = '<option value="">— без шаблона —</option>';
    _templates.forEach(tmpl => {
      const opt = new Option(tmpl.name, tmpl.name);
      opt.selected = tmpl.name === active;
      tmplSel.appendChild(opt);
    });
  }
}

function _populateDlgLlmModels(provider) {
  const sel = document.getElementById('dlg-llm-model');
  if (!sel) return;
  const cloud = _lastModels.cloud || {};
  const p = _lastConfig.protocol || {};
  const list = cloud[provider] || [];
  const currentModel = p[provider + '_model'] || '';
  sel.innerHTML = '';
  if (list.length === 0) {
    sel.style.display = 'none';
    return;
  }
  sel.style.display = '';
  list.forEach(m => {
    const opt = new Option(m, m);
    opt.selected = m === currentModel;
    sel.appendChild(opt);
  });
  if (!sel.value && list.length) sel.value = list[0];
}

function onDlgProviderChange() {
  const provider = document.getElementById('dlg-provider').value;
  _populateDlgLlmModels(provider);
}

function _getDlgPipeParams() {
  const model = (document.getElementById('dlg-whisper-model') || {}).value || '';
  const template = (document.getElementById('dlg-template') || {}).value || '';
  const provider = (document.getElementById('dlg-provider') || {}).value || '';
  const llm_model = (document.getElementById('dlg-llm-model') || {}).value || '';
  return {
    model: model || null,
    template: template || null,
    provider: provider || null,
    llm_model: llm_model || null,
  };
}

async function checkDlgMicMute() {
  const source = document.getElementById('dlg-mic-src').value;
  try {
    const r = await fetch('/api/record/mic-mute?source=' + encodeURIComponent(source));
    const d = await r.json();
    document.getElementById('dlg-mic-mute-dot').style.display = d.muted ? '' : 'none';
  } catch (_) {}
}

function toggleDlgPipeStep(step) {
  const chk = document.getElementById('dlg-chk-' + step);
  const stepEl = document.getElementById('dlg-pipe-' + step);
  if (step === 'transcribe' && !chk.checked) {
    document.getElementById('dlg-chk-protocol').checked = false;
    document.getElementById('dlg-pipe-toggle-protocol').classList.add('disabled');
    document.getElementById('dlg-pipe-protocol').classList.add('skipped');
    stepEl.classList.add('skipped');
  } else if (step === 'transcribe' && chk.checked) {
    document.getElementById('dlg-pipe-toggle-protocol').classList.remove('disabled');
    stepEl.classList.remove('skipped');
  } else if (step === 'protocol') {
    document.getElementById('dlg-pipe-protocol').classList.toggle('skipped', !chk.checked);
  }
}

function onDlgDropOver(e) {
  e.preventDefault();
  document.getElementById('dlg-drop-zone').classList.add('dragover');
}

function onDlgDropLeave() {
  document.getElementById('dlg-drop-zone').classList.remove('dragover');
}

function onDlgDrop(e) {
  e.preventDefault();
  document.getElementById('dlg-drop-zone').classList.remove('dragover');
  const f = e.dataTransfer.files[0];
  if (f) _setDlgFile(f);
}

function onDlgFileSelected(input) {
  if (input.files[0]) _setDlgFile(input.files[0]);
}

function _setDlgFile(f) {
  _dlgUploadFile = f;
  const mb = (f.size / 1048576).toFixed(1);
  document.getElementById('dlg-drop-text').innerHTML =
    `<strong>${f.name}</strong><br><span style="font-size:.75rem;color:var(--text-muted)">${mb} МБ</span>`;
}

// Dispatch to appropriate action based on active tab
function dialogStartAction() {
  if (_dialogTab === 'record') startRecFromDialog();
  else if (_dialogTab === 'upload') startUploadFromDialog();
  else startFromFolderFromDialog();
}

// ============================================================
// Recording (from dialog)
// ============================================================
let _recFolder = null;
let _recPollTimer = null;

async function startRecFromDialog() {
  const name = document.getElementById('dlg-rec-name').value.trim();
  const prependDate = document.getElementById('dlg-prepend-date').checked;

  // save sources to config
  await fetch('/api/config', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ recording: {
      mic_source: document.getElementById('dlg-mic-src').value,
      system_source: document.getElementById('dlg-sys-src').value,
      prepend_date: prependDate,
    }}),
  });

  const r = await fetch('/api/record/start', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name, prepend_date: prependDate }),
  });
  const d = await r.json();
  if (d.error) { showToast(d.error, 'error'); return; }

  _recFolder = d.folder;
  closeNewMeetingDialog();

  // Update sidebar button to show recording state
  const btnNew = document.getElementById('btn-new-meeting');
  btnNew.classList.add('recording');
  document.getElementById('new-meeting-icon').textContent = '■';
  document.getElementById('new-meeting-label').textContent = 'Запись...';
  btnNew.onclick = () => { if (_recFolder) selectMeeting(_recFolder); };

  // Navigate to meeting card
  await fetchMeetings();
  await selectMeeting(_recFolder);
}

async function stopRecFromCard() {
  _stopRecPolling();
  const stopResp = await fetch('/api/record/stop', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name: _recFolder || '' }),
  });
  const stopData = await stopResp.json();
  const slug = stopData.folder || _recFolder;  // capture before _resetRecordBtn clears it

  _resetRecordBtn();  // sets _recFolder = null

  // Hide recording bar
  document.getElementById('meeting-rec-bar').style.display = 'none';

  // Auto-pipeline based on dialog toggles
  const doTranscribe = document.getElementById('dlg-chk-transcribe').checked;
  const doProtocol = document.getElementById('dlg-chk-protocol').checked;

  if (!doTranscribe) {
    await fetchMeetings();
    await selectMeeting(slug);
    return;
  }

  // Start pipeline and show progress on the card
  await _startPipelineOnCard(slug, { no_protocol: !doProtocol, ..._getDlgPipeParams() });
}

function _resetRecordBtn() {
  const btnNew = document.getElementById('btn-new-meeting');
  btnNew.classList.remove('recording');
  document.getElementById('new-meeting-icon').textContent = '+';
  document.getElementById('new-meeting-label').textContent = 'Новая встреча';
  btnNew.onclick = () => openNewMeetingDialog();
  _recFolder = null;
}

function _startRecPolling(_slug) {
  _stopRecPolling();
  const timerEl = document.getElementById('meeting-rec-timer');
  _recPollTimer = setInterval(async () => {
    const r = await fetch('/api/record/status');
    const d = await r.json();
    if (!d.running) {
      _stopRecPolling();
      document.getElementById('meeting-rec-bar').style.display = 'none';
      return;
    }
    const m = String(Math.floor(d.elapsed / 60)).padStart(2, '0');
    const s = String(d.elapsed % 60).padStart(2, '0');
    if (timerEl) timerEl.textContent = `Запись: ${m}:${s}`;
  }, 1000);
}

function _stopRecPolling() {
  if (_recPollTimer) { clearInterval(_recPollTimer); _recPollTimer = null; }
}

// ============================================================
// Upload (from dialog)
// ============================================================
async function startUploadFromDialog() {
  if (!_dlgUploadFile) {
    showToast('Выбери файл', 'info');
    return;
  }
  const title = document.getElementById('dlg-upload-title').value.trim();
  const doTranscribe = document.getElementById('dlg-chk-transcribe').checked;
  const doProtocol = document.getElementById('dlg-chk-protocol').checked;

  closeNewMeetingDialog();

  const fd = new FormData();
  fd.append('file', _dlgUploadFile);
  fd.append('title', title);
  fd.append('auto_process', 'false');  // we'll trigger pipeline from card
  fd.append('no_protocol', 'true');

  showToast('Загрузка файла...', 'info', 60000);

  let uploadSlug = null;
  try {
    uploadSlug = await new Promise((resolve, reject) => {
      const xhr = new XMLHttpRequest();
      xhr.open('POST', '/api/upload');
      xhr.onload = () => {
        if (xhr.status >= 200 && xhr.status < 300) {
          resolve(JSON.parse(xhr.responseText).slug);
        } else {
          let msg = 'Ошибка загрузки';
          try { msg = JSON.parse(xhr.responseText).detail || msg; } catch {}
          reject(new Error(msg));
        }
      };
      xhr.onerror = () => reject(new Error('Ошибка соединения'));
      xhr.send(fd);
    });
  } catch (err) {
    showToast(err.message, 'error');
    return;
  }

  // dismiss upload toast
  document.querySelectorAll('.toast:not(.toast-error)').forEach(t => t.remove());

  await fetchMeetings();
  await selectMeeting(uploadSlug);

  if (!doTranscribe) return;
  await _startPipelineOnCard(uploadSlug, { no_protocol: !doProtocol, ..._getDlgPipeParams() });
}

// ============================================================
// From folder (from dialog)
// ============================================================
async function startFromFolderFromDialog() {
  const slug = document.getElementById('dlg-folder-select').value;
  if (!slug) { showToast('Выбери встречу из списка', 'info'); return; }
  closeNewMeetingDialog();
  await selectMeeting(slug);
  await _startPipelineOnCard(slug, { no_protocol: false, ..._getDlgPipeParams() });
}

// ============================================================
// Sidebar meetings list
// ============================================================
let _meetingData = { protocol: null, transcript: null, status: 'COMPLETED' };
let _activeTab = 'protocol';
let _lastModels = {};
let _meetings = [];
let _lastConfig = {};

async function fetchMeetings() {
  const r = await fetch('/api/meetings');
  const d = await r.json();
  _meetings = d.folders || [];
  renderList(_meetings);
}

const STATUS_LABEL = {
  CREATED:            'создана',
  AUDIO_READY:        'готова к обработке',
  TRANSCRIBING:       'транскрибирую...',
  TRANSCRIBED:        'транскрибирована',
  GENERATING:         'генерирую протокол...',
  COMPLETED:          '',
  ARCHIVED:           'архив',
  TRANSCRIBE_FAILED:  'ошибка транскрипции',
  PROTOCOL_FAILED:    'ошибка протокола',
  CANCELLED:          'отменена',
};

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
    const status = (typeof f === 'object' && f.status) ? f.status : 'COMPLETED';
    const isProcessing = ['TRANSCRIBING', 'GENERATING'].includes(status);
    const isFailed = status.endsWith('_FAILED');
    let meta = '';
    if (isProcessing) {
      meta = `<span class="has" style="color:var(--accent-text)">· ${STATUS_LABEL[status] || status}</span>`;
    } else if (isFailed) {
      meta = `<span class="has" style="color:var(--warning)">· ${STATUS_LABEL[status] || status}</span>`;
    } else if (!hasTr && !hasPr) {
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
      <button class="del-btn" onclick="openDeleteConfirm(event,'${slug}')" title="Удалить">✕</button>
    </div>`;
  }).join('');
  list.innerHTML = sectionHtml + itemsHtml;
  if (_selectedSlug) {
    const active = list.querySelector(`[data-slug="${_selectedSlug}"]`);
    if (active) active.classList.add('active');
  }
}

// ============================================================
// Meeting viewer
// ============================================================
let _meetingSSE = null;
let _currentJobId = null;

async function selectMeeting(slug) {
  _stopMeetingSSE();
  _stopRecPolling();
  _selectedSlug = slug;
  document.querySelectorAll('.meeting-item').forEach(i =>
    i.classList.toggle('active', i.dataset.slug === slug));

  const r = await fetch('/api/meeting?folder=' + encodeURIComponent(slug));
  _meetingData = await r.json();
  const title = slug.replace(/_/g, ' ');
  document.getElementById('viewer-title').textContent = title;

  const status = _meetingData.status || 'COMPLETED';
  _renderMeetingCard(status);
  showView('meeting');
}

function _renderMeetingCard(status) {
  const hasProtocol = !!_meetingData.protocol;
  const hasTranscript = !!_meetingData.transcript;

  // Status badge
  _setStatusBadge(status);

  // Recording bar – shown when this slug is the actively recording folder
  const recBarEl = document.getElementById('meeting-rec-bar');
  if (_recFolder && _recFolder === _selectedSlug) {
    recBarEl.style.display = '';
    _startRecPolling(_selectedSlug);
  } else {
    recBarEl.style.display = 'none';
  }

  // Progress panel
  const isProcessing = ['TRANSCRIBING', 'GENERATING', 'AUDIO_READY'].includes(status);
  const isFailed = status.endsWith('_FAILED');
  const progressPanel = document.getElementById('meeting-progress-panel');

  if (isProcessing && status !== 'AUDIO_READY') {
    progressPanel.style.display = '';
    hideErrorBanner('meeting-error');
    _resetMeetingProgress();
    if (status === 'TRANSCRIBING') {
      _setMeetingStep('tr', 'active');
      _setMeetingBar('indeterminate');
      _setMeetingSubStage('Транскрибирую...');
    } else if (status === 'GENERATING') {
      _setMeetingStep('tr', 'done');
      document.getElementById('mpconn-1').style.width = '100%';
      _setMeetingStep('pr', 'active');
      _setMeetingBar('indeterminate');
      _setMeetingSubStage('Генерирую протокол...');
    }
    // Restore job ID so cancel/retry work after page reload or re-navigation
    _currentJobId = _meetingData.current_job_id || null;
    _connectMeetingSSE(_selectedSlug);
    document.getElementById('btn-cancel-job').style.display = '';
    document.getElementById('btn-retry-job').style.display = 'none';
  } else if (isFailed) {
    progressPanel.style.display = '';
    _resetMeetingProgress();
    if (status === 'TRANSCRIBE_FAILED') _setMeetingStep('tr', 'done');
    if (status === 'PROTOCOL_FAILED') {
      _setMeetingStep('tr', 'done');
      document.getElementById('mpconn-1').style.width = '100%';
      _setMeetingStep('pr', 'done');
    }
    document.getElementById('btn-cancel-job').style.display = 'none';
    document.getElementById('btn-retry-job').style.display = '';
    showErrorBanner('meeting-error',
      _meetingData.error_kind || 'unknown',
      _meetingData.error_message || (status === 'TRANSCRIBE_FAILED' ? 'Ошибка транскрипции' : 'Ошибка генерации протокола'),
      retryCurrentJob);
  } else {
    progressPanel.style.display = 'none';
  }

  // Tabs visibility
  document.getElementById('tab-protocol').style.display = hasProtocol ? '' : 'none';
  document.getElementById('tab-transcript').style.display = hasTranscript ? '' : 'none';

  // Params block
  _renderViewerParams(hasTranscript, hasProtocol);

  // Context menu items
  _updateViewerMenu(status, hasProtocol, hasTranscript);

  // Render content
  const tab = hasProtocol ? 'protocol' : (hasTranscript ? 'transcript' : null);
  if (tab) showTab(tab);
  else {
    // Nothing to show yet
    document.getElementById('viewer-body').innerHTML =
      `<div style="color:var(--text-muted);font-size:.83rem;padding:24px">
        ${_getStatusMessage(status)}
      </div>`;
  }
}

function _getStatusMessage(status) {
  const msgs = {
    CREATED:       'Встреча создана. Запустите запись или загрузите аудио.',
    AUDIO_READY:   'Аудио готово. Запустите транскрипцию.',
    TRANSCRIBING:  'Транскрибирование в процессе...',
    GENERATING:    'Генерация протокола в процессе...',
    TRANSCRIBED:   'Транскрипция готова. Можно сгенерировать протокол.',
    COMPLETED:     '',
    TRANSCRIBE_FAILED: 'Транскрипция завершилась с ошибкой.',
    PROTOCOL_FAILED:   'Генерация протокола завершилась с ошибкой.',
    CANCELLED:     'Обработка была отменена.',
    ARCHIVED:      'Встреча архивирована.',
  };
  return msgs[status] || '';
}

function _setStatusBadge(status) {
  const badge = document.getElementById('viewer-status-badge');
  if (!badge) return;
  const processing = ['TRANSCRIBING', 'GENERATING'].includes(status);
  const failed = status.endsWith('_FAILED');
  const completed = status === 'COMPLETED';
  const recording = status === 'AUDIO_READY' && _recFolder === _selectedSlug;

  if (completed) { badge.style.display = 'none'; return; }

  badge.style.display = '';
  badge.className = 'meeting-status-badge';
  if (recording) {
    badge.classList.add('status-recording');
    badge.textContent = '● Запись';
  } else if (processing) {
    badge.classList.add('status-processing');
    badge.textContent = STATUS_LABEL[status] || status;
  } else if (failed) {
    badge.classList.add('status-failed');
    badge.textContent = STATUS_LABEL[status] || status;
  } else {
    badge.classList.add('status-neutral');
    badge.textContent = STATUS_LABEL[status] || status;
  }
}

function _updateViewerMenu(status, _hasProtocol, hasTranscript) {
  const canRetranscribe = ['TRANSCRIBED', 'COMPLETED', 'TRANSCRIBE_FAILED', 'PROTOCOL_FAILED', 'AUDIO_READY'].includes(status);
  const canRegen = ['TRANSCRIBED', 'COMPLETED', 'PROTOCOL_FAILED'].includes(status) && hasTranscript;
  document.getElementById('vmenu-retranscribe').disabled = !canRetranscribe;
  document.getElementById('vmenu-regen').disabled = !canRegen;
}

function _renderViewerParams(hasTranscript, hasProtocol) {
  const paramsEl = document.getElementById('viewer-params');
  const bodyEl = document.getElementById('viewer-params-body');
  if (!paramsEl || !bodyEl) return;
  if (!hasTranscript && !hasProtocol) { paramsEl.style.display = 'none'; return; }

  // Prefer per-meeting metadata saved at job completion; fall back to current config.
  const perMeeting = {
    transcription_model: _meetingData.transcription_model,
    protocol_model: _meetingData.protocol_model,
    template_name: _meetingData.template_name,
  };
  const t = _lastConfig.transcription || {};
  const p = _lastConfig.protocol || {};
  const items = [];

  if (hasTranscript) {
    if (perMeeting.transcription_model) {
      items.push(`<span class="viewer-param-item"><strong>Транскрипция:</strong> ${perMeeting.transcription_model}</span>`);
    } else {
      const model = t.model || '—';
      const device = (t.device || 'cpu').toUpperCase();
      const compute = t.compute_type || 'int8';
      items.push(`<span class="viewer-param-item"><strong>Транскрипция:</strong> faster-whisper · ${model} · ${device} · ${compute}</span>`);
    }
    if (t.language) items.push(`<span class="viewer-param-item"><strong>Язык:</strong> ${t.language}</span>`);
  }
  if (hasProtocol) {
    if (perMeeting.protocol_model) {
      items.push(`<span class="viewer-param-item"><strong>Протокол:</strong> ${perMeeting.protocol_model}</span>`);
    } else {
      const provider = p.provider || 'claude';
      const model = p[provider + '_model'] || '—';
      items.push(`<span class="viewer-param-item"><strong>Протокол:</strong> ${provider} · ${model}</span>`);
    }
    const tmpl = perMeeting.template_name || p.active_template || '—';
    items.push(`<span class="viewer-param-item"><strong>Шаблон:</strong> ${tmpl}</span>`);
  }
  bodyEl.innerHTML = items.join('');
  paramsEl.style.display = '';
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
// "..." viewer menu
// ============================================================
function toggleViewerMenu(e) {
  if (e) e.stopPropagation();
  const dd = document.getElementById('viewer-menu-dropdown');
  dd.classList.toggle('open');
}

// Close menu when clicking outside
document.addEventListener('click', () => {
  document.getElementById('viewer-menu-dropdown')?.classList.remove('open');
});

// ============================================================
// SSE connection for meeting card
// ============================================================
function _connectMeetingSSE(slug) {
  _stopMeetingSSE();
  // Try new RQ endpoint; fall back to legacy on failure
  _meetingSSE = new EventSource('/api/meetings/' + encodeURIComponent(slug) + '/stream');
  _meetingSSE.onmessage = e => {
    try {
      const ev = JSON.parse(e.data);
      _handleMeetingSSEEvent(ev);
    } catch (_) {}
  };
  _meetingSSE.addEventListener('done', async () => {
    _stopMeetingSSE();
    // Reload meeting data to show content
    await _reloadMeetingContent(_selectedSlug);
    showToast(`Встреча «${_selectedSlug.replace(/_/g, ' ')}» готова`, 'success');
    if (document.visibilityState === 'hidden') _maybeNotifyCompletion(_selectedSlug, true);
    _pollQueue();
  });
  _meetingSSE.onerror = () => {
    // If new endpoint fails, fall back to legacy stream
    _stopMeetingSSE();
    _connectLegacySSE(slug);
  };
}

function _connectLegacySSE(slug) {
  _stopMeetingSSE();
  _meetingSSE = new EventSource('/api/process/stream?slug=' + encodeURIComponent(slug));
  _meetingSSE.onmessage = e => {
    try {
      const ev = JSON.parse(e.data);
      _handleMeetingSSEEvent(ev);
    } catch (_) {}
  };
  _meetingSSE.addEventListener('done', async () => {
    _stopMeetingSSE();
    await _reloadMeetingContent(_selectedSlug);
    showToast(`Встреча «${_selectedSlug.replace(/_/g, ' ')}» готова`, 'success');
    if (document.visibilityState === 'hidden') _maybeNotifyCompletion(_selectedSlug, true);
    _pollQueue();
  });
  _meetingSSE.onerror = () => { _stopMeetingSSE(); };
}

function _stopMeetingSSE() {
  if (_meetingSSE) { _meetingSSE.close(); _meetingSSE = null; }
}

function _handleMeetingSSEEvent(ev) {
  const evType = ev.type || ev.stage;
  if (evType === 'error') {
    showErrorBanner('meeting-error', ev.error_kind, ev.message, retryCurrentJob);
    _setMeetingSubStage('');
    document.getElementById('btn-cancel-job').style.display = 'none';
    document.getElementById('btn-retry-job').style.display = '';
    document.getElementById('meeting-progress-panel').style.display = '';
    if (document.visibilityState === 'hidden') _maybeNotifyCompletion(_selectedSlug, false);
    _pollQueue();
    fetchMeetings();
  } else if (evType === 'progress' || (ev.stage === 'progress' && ev.progress != null)) {
    const val = ev.value ?? ev.progress;
    if (val != null) {
      const pct = Math.round(val * 100);
      _setMeetingBar(pct);
      _setMeetingSubStage(`Транскрипция ${pct}%`);
    } else {
      _setMeetingBar('indeterminate');
    }
  } else if (evType === 'save' || ev.stage === 'save') {
    _setMeetingSubStage('Сохранение...');
  } else if (evType === 'transcribe' || ev.stage === 'transcribe') {
    _setMeetingStep('tr', 'active');
    _setMeetingBar('indeterminate');
    _setMeetingSubStage('Транскрибирую...');
    fetchMeetings();
  } else if (evType === 'protocol' || ev.stage === 'protocol') {
    _setMeetingStep('tr', 'done');
    document.getElementById('mpconn-1').style.width = '100%';
    _setMeetingStep('pr', 'active');
    _setMeetingBar('indeterminate');
    _setMeetingSubStage('Генерирую протокол...');
    fetchMeetings();
  } else if (evType === 'job_done') {
    _setMeetingStep('pr', 'done');
    document.getElementById('mpconn-2').style.width = '100%';
    _setMeetingBar(100);
    fetchMeetings();
  }
}

async function _reloadMeetingContent(slug) {
  const r = await fetch('/api/meeting?folder=' + encodeURIComponent(slug));
  _meetingData = await r.json();
  _renderMeetingCard(_meetingData.status || 'COMPLETED');
  await fetchMeetings();
}

// ============================================================
// Pipeline progress helpers (meeting card)
// ============================================================
function _resetMeetingProgress() {
  ['tr', 'pr'].forEach(id => {
    const c = document.getElementById('mpstep-' + id);
    const l = document.getElementById('mpstep-' + id + '-lbl');
    if (c) { c.className = 'pipe-step-circle'; c.textContent = id === 'tr' ? '2' : '3'; }
    if (l) { l.className = 'pipe-step-label'; }
  });
  document.getElementById('mpconn-1').style.width = '0%';
  document.getElementById('mpconn-2').style.width = '0%';
  _setMeetingBar(0);
  _setMeetingSubStage('');
  hideErrorBanner('meeting-error');
}

function _setMeetingStep(id, state) {
  const c = document.getElementById('mpstep-' + id);
  const l = document.getElementById('mpstep-' + id + '-lbl');
  if (!c) return;
  if (state === 'done') {
    c.className = 'pipe-step-circle done'; c.textContent = '✓';
    if (l) l.className = 'pipe-step-label done';
  } else if (state === 'active') {
    c.className = 'pipe-step-circle active';
    if (l) l.className = 'pipe-step-label active';
  }
}

function _setMeetingBar(pctOrMode) {
  const bar = document.getElementById('meeting-pipe-bar');
  const pctEl = document.getElementById('meeting-pipe-pct');
  if (!bar) return;
  if (pctOrMode === 'indeterminate') {
    bar.classList.add('indeterminate');
    if (pctEl) pctEl.textContent = '';
  } else {
    bar.classList.remove('indeterminate');
    bar.style.width = pctOrMode + '%';
    if (pctEl) pctEl.textContent = pctOrMode > 0 && pctOrMode < 100 ? pctOrMode + '%' : '';
  }
}

function _setMeetingSubStage(text) {
  const el = document.getElementById('meeting-pipe-sub');
  if (el) el.textContent = text || '';
}

// ============================================================
// Pipeline trigger helpers
// ============================================================
async function _startPipelineOnCard(slug, opts = {}) {
  // Show progress panel
  const progressPanel = document.getElementById('meeting-progress-panel');
  progressPanel.style.display = '';
  hideErrorBanner('meeting-error');
  _resetMeetingProgress();
  _setMeetingStep('tr', 'active');
  _setMeetingBar('indeterminate');
  _setMeetingSubStage('Запуск...');
  document.getElementById('btn-cancel-job').style.display = '';
  document.getElementById('btn-retry-job').style.display = 'none';

  _currentJobId = null;

  // Connect SSE first (so we don't miss early events)
  _connectMeetingSSE(slug);

  // Start pipeline
  const r = await fetch('/api/process/start', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      folder: slug,
      no_protocol: opts.no_protocol || false,
      model: opts.model || '',
      template: opts.template || '',
      provider: opts.provider || '',
      llm_model: opts.llm_model || '',
    }),
  });
  const d = await r.json();
  if (!d.ok) {
    _stopMeetingSSE();
    showToast(typeof d.detail === 'string' ? d.detail : 'Ошибка запуска', 'error');
    progressPanel.style.display = 'none';
  } else {
    _currentJobId = d.job_id || null;
  }
  fetchMeetings();
}

// ============================================================
// Meeting card actions
// ============================================================
async function retranscribeMeeting() {
  document.getElementById('viewer-menu-dropdown').classList.remove('open');
  if (!_selectedSlug) return;
  // Try new RQ endpoint
  let ok = false;
  _currentJobId = null;
  try {
    const r = await fetch('/api/meetings/' + encodeURIComponent(_selectedSlug) + '/transcribe', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({}),
    });
    if (r.ok) {
      const d = await r.json();
      _currentJobId = d.job_id || null;
      ok = true;
    }
  } catch (_) {}

  if (!ok) {
    // Fallback: use legacy process/start
    await _startPipelineOnCard(_selectedSlug, { no_protocol: true });
    return;
  }

  // RQ accepted - show progress and connect SSE
  const progressPanel = document.getElementById('meeting-progress-panel');
  progressPanel.style.display = '';
  _resetMeetingProgress();
  _setMeetingStep('tr', 'active');
  _setMeetingBar('indeterminate');
  _setMeetingSubStage('Транскрибирую...');
  document.getElementById('btn-cancel-job').style.display = '';
  _connectMeetingSSE(_selectedSlug);
  fetchMeetings();
}

async function regenProtocol() {
  document.getElementById('viewer-menu-dropdown').classList.remove('open');
  if (!_selectedSlug) return;

  // Try RQ endpoint
  let ok = false;
  _currentJobId = null;
  try {
    const r = await fetch('/api/meetings/' + encodeURIComponent(_selectedSlug) + '/generate', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({}),
    });
    if (r.ok) {
      const d = await r.json();
      _currentJobId = d.job_id || null;
      ok = true;
    }
  } catch (_) {}

  if (!ok) {
    // Fallback: legacy
    _connectLegacySSE(_selectedSlug);
    await fetch('/api/process/start', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ folder: _selectedSlug, from_transcript: true }),
    });
  }

  const progressPanel = document.getElementById('meeting-progress-panel');
  progressPanel.style.display = '';
  _resetMeetingProgress();
  _setMeetingStep('tr', 'done');
  document.getElementById('mpconn-1').style.width = '100%';
  _setMeetingStep('pr', 'active');
  _setMeetingBar('indeterminate');
  _setMeetingSubStage('Генерирую протокол...');
  document.getElementById('btn-cancel-job').style.display = '';
  if (!ok) _connectLegacySSE(_selectedSlug);
  else _connectMeetingSSE(_selectedSlug);
  fetchMeetings();
}

async function deleteAudioOnly() {
  document.getElementById('viewer-menu-dropdown').classList.remove('open');
  if (!_selectedSlug) return;
  const title = _selectedSlug.replace(/_/g, ' ');
  if (!confirm(`Удалить аудиозапись встречи «${title}»? Транскрипция и протокол сохранятся.`)) return;
  const r = await fetch('/api/meeting/audio?folder=' + encodeURIComponent(_selectedSlug), { method: 'DELETE' });
  if (r.ok) {
    showToast('Аудио удалено', 'success');
    fetchMeetings();
  } else {
    showToast('Не удалось удалить аудио', 'error');
  }
}

async function cancelCurrentJob() {
  _stopMeetingSSE();
  document.getElementById('btn-cancel-job').style.display = 'none';

  if (!_currentJobId) {
    // Legacy subprocess mode — ask server to terminate the process
    try {
      await fetch('/api/process/cancel', { method: 'POST' });
    } catch (_) {}
    _setMeetingSubStage('Отменено');
    document.getElementById('btn-retry-job').style.display = '';
    showToast('Задача отменена', 'info');
    fetchMeetings();
    return;
  }

  try {
    const r = await fetch('/api/jobs/' + _currentJobId + '/cancel', { method: 'POST' });
    if (r.ok) {
      document.getElementById('btn-retry-job').style.display = '';
      _setMeetingSubStage('Отменено');
      showToast('Задача отменена', 'info');
      fetchMeetings();
    }
  } catch (_) {}
}

async function retryCurrentJob() {
  if (!_currentJobId) {
    // Fallback: restart pipeline
    await _startPipelineOnCard(_selectedSlug, {});
    return;
  }
  try {
    const r = await fetch('/api/jobs/' + _currentJobId + '/retry', { method: 'POST' });
    const d = await r.json();
    if (r.ok) {
      _currentJobId = d.job_id;
      document.getElementById('btn-retry-job').style.display = 'none';
      document.getElementById('btn-cancel-job').style.display = '';
      hideErrorBanner('meeting-error');
      _resetMeetingProgress();
      _setMeetingStep('tr', 'active');
      _setMeetingBar('indeterminate');
      _connectMeetingSSE(_selectedSlug);
      fetchMeetings();
    }
  } catch (_) {
    await _startPipelineOnCard(_selectedSlug, {});
  }
}

// ============================================================
// Delete confirmation overlay
// ============================================================
let _pendingDeleteSlug = null;

function openDeleteConfirm(e, slug) {
  e.stopPropagation();
  document.getElementById('viewer-menu-dropdown')?.classList.remove('open');
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
  const res = await fetch('/api/meeting/delete', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ folder: slug, what: 'all' }),
  });
  if (!res.ok) {
    showToast('Не удалось удалить встречу', 'error');
    return;
  }
  if (slug === _selectedSlug) showHome();
  fetchMeetings();
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
  else showToast('Ошибка: ' + (data.detail || data.error || 'unknown'), 'error');
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
  _lastModels = await mr.json();
  const models = _lastModels;
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
  _lastConfig = c;
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
  document.getElementById('dlg-prepend-date').checked = rec.prepend_date !== false;
  _updateDlgPipeDesc();
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
        mic_source: document.getElementById('dlg-mic-src').value,
        system_source: document.getElementById('dlg-sys-src').value,
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
    showToast('Ошибка: ' + (d.detail || d.error || 'unknown'), 'error');
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
  if (!name) { showToast('Нечего удалять', 'info'); return; }
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
// Queue badge & panel
// ============================================================
let _queuePollTimer = null;
let _queuePanelOpen = false;
let _lastQueueActive = 0;  // used to detect new completions for notifications

const QUEUE_STAGE_LABEL = {
  loading_model: 'загрузка модели',
  transcribing: 'транскрибирую',
  loading_transcript: 'загрузка',
  generating: 'генерирую',
  saving: 'сохранение',
  queued: 'в очереди',
  running: 'обработка',
};

function _queueJobKindLabel(kind) {
  return kind === 'transcribe' ? 'Транскрипция' : kind === 'protocol' ? 'Протокол' : kind;
}

async function _pollQueue() {
  try {
    const r = await fetch('/api/jobs/active');
    if (!r.ok) { _updateQueueBadge([]); return; }
    const data = await r.json();
    const jobs = data.jobs || [];
    _updateQueueBadge(jobs);
    if (_queuePanelOpen) _renderQueuePanel(jobs);
    _checkNotificationsFromQueue(jobs);
  } catch (_) {
    _updateQueueBadge([]);
  }
}

function _updateQueueBadge(jobs) {
  const badge = document.getElementById('queue-badge');
  if (!badge) return;
  const active = jobs.filter(j => j.status === 'running').length;
  const queued = jobs.filter(j => j.status === 'queued').length;
  const total = active + queued;
  if (total === 0) {
    badge.style.display = 'none';
    if (_queuePanelOpen) _closeQueuePanel();
  } else {
    badge.style.display = '';
    badge.textContent = active > 0
      ? `⚙ ${active} обрабатывается${queued > 0 ? ` · ${queued} в очереди` : ''}`
      : `⏳ ${queued} в очереди`;
  }
}

function _renderQueuePanel(jobs) {
  const list = document.getElementById('queue-panel-list');
  if (!list) return;
  if (!jobs.length) { list.innerHTML = '<div style="padding:10px 12px;font-size:.75rem;color:var(--text-muted)">Нет активных задач</div>'; return; }
  list.innerHTML = jobs.map(j => {
    const title = (j.meeting_slug || '').replace(/_/g, ' ');
    const stageKey = j.progress_stage || j.status;
    const stageLabel = QUEUE_STAGE_LABEL[stageKey] || stageKey;
    const pct = j.progress_value != null ? Math.round(j.progress_value * 100) + '%' : '';
    const kindLabel = _queueJobKindLabel(j.kind);
    const eta = j.progress_eta_sec != null && j.progress_eta_sec > 0
      ? ` · ~${Math.ceil(j.progress_eta_sec / 60)} мин` : '';
    return `<div class="queue-job-item" onclick="selectMeeting('${j.meeting_slug}')" style="cursor:pointer">
      <div class="queue-job-title">${title}</div>
      <div class="queue-job-stage">${kindLabel} · ${stageLabel}${eta}</div>
      <div class="queue-job-pct">${pct}</div>
    </div>`;
  }).join('');
}

function toggleQueuePanel(e) {
  if (e) e.stopPropagation();
  const panel = document.getElementById('queue-panel');
  if (!panel) return;
  _queuePanelOpen = !_queuePanelOpen;
  panel.style.display = _queuePanelOpen ? '' : 'none';
  if (_queuePanelOpen) _pollQueue();
}

function _closeQueuePanel() {
  _queuePanelOpen = false;
  const panel = document.getElementById('queue-panel');
  if (panel) panel.style.display = 'none';
}

document.addEventListener('click', (e) => {
  if (_queuePanelOpen && !e.target.closest('#queue-panel') && !e.target.closest('#queue-badge')) {
    _closeQueuePanel();
  }
});

function _startQueuePolling() {
  _pollQueue();
  _queuePollTimer = setInterval(_pollQueue, 4000);
}

// ============================================================
// Browser Notifications API
// ============================================================
let _notifPermission = Notification?.permission || 'default';
let _longJobSlugs = new Set();  // slugs of jobs where ETA > 15 min — notify on completion

function _maybeRequestNotifPermission() {
  if (!('Notification' in window)) return;
  if (Notification.permission === 'default') {
    Notification.requestPermission().then(p => { _notifPermission = p; });
  }
}

function _sendNotification(title, body) {
  if (!('Notification' in window) || Notification.permission !== 'granted') return;
  try {
    new Notification(title, { body, icon: '/static/favicon.ico' });
  } catch (_) {}
}

function _checkNotificationsFromQueue(jobs) {
  // Track slugs that have high ETA; notify when they finish
  jobs.forEach(j => {
    if (j.progress_eta_sec != null && j.progress_eta_sec > 900) {
      _longJobSlugs.add(j.meeting_slug);
      _maybeRequestNotifPermission();
    }
  });
}

// Called when SSE signals job completion
function _maybeNotifyCompletion(slug, success) {
  if (!_longJobSlugs.has(slug)) return;
  _longJobSlugs.delete(slug);
  const label = slug.replace(/_/g, ' ');
  if (success) {
    _sendNotification(`Встреча готова`, `«${label}» обработана`);
  } else {
    _sendNotification(`Ошибка обработки`, `«${label}» завершилась с ошибкой`);
  }
}

// ============================================================
// Init
// ============================================================
fetchConfig().then(() => {});
fetchMeetings();
_startQueuePolling();
fetchTemplates();
