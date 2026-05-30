// Sidebar — meeting library. Persistent left rail.
// Pass-2: less noise per row (template type instead of redundant "готов"),
// exception-state for in-progress recording and missing protocol, bigger
// active state with hairline-glow.

function Sidebar({ meetings, selectedId, onSelect, onNew, onShowSettings, onOpenPalette, sidebarStyle, density, query, setQuery }) {
  const filtered = React.useMemo(() => {
    if (!query.trim()) return meetings;
    const q = query.toLowerCase();
    return meetings.filter(m => m.name.toLowerCase().includes(q));
  }, [meetings, query]);

  const groups = React.useMemo(() => groupByDate(filtered), [filtered]);

  if (sidebarStyle === 'compact') {
    return (
      <aside className="sidebar sidebar-compact">
        <div style={{ height: 34 }} />
        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 4, padding: '6px 0', flex: 1, minHeight: 0 }}>
          <Wordmark size={32} />
          <div style={{ height: 10 }} />
          <button className="btn-icon btn-ghost" title="Новая запись · ⌘N" onClick={onNew}><Icon name="plus" /></button>
          <button className="btn-icon btn-ghost" title="Поиск · ⌘K" onClick={onOpenPalette}><Icon name="search" /></button>
          <div style={{ height: 8 }} />
          <div style={{ flex: 1, overflowY: 'auto', width: '100%', padding: '8px 0', display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 2 }}>
            {filtered.slice(0, 16).map(m => (
              <button key={m.id}
                onClick={() => onSelect(m.id)}
                title={m.name}
                className={`compact-row ${selectedId === m.id ? 'is-selected' : ''} ${m.recording ? 'is-recording' : ''}`}
              >
                <span className="compact-glyph">{m.recording ? <Icon name="mic" size={14} /> : initials(m.name)}</span>
              </button>
            ))}
          </div>
          <button className="btn-icon btn-ghost" title="Настройки" onClick={onShowSettings}><Icon name="gear" /></button>
        </div>
      </aside>
    );
  }

  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <Wordmark size={30} />
        <div className="brand">meety</div>
        <div style={{ flex: 1 }} />
        <button className="btn-icon btn-ghost" title="Новая запись · ⌘N" onClick={onNew}><Icon name="plus" /></button>
      </div>

      <button className="sidebar-search" onClick={onOpenPalette} type="button">
        <Icon name="search" size={14} />
        <span style={{ flex: 1, textAlign: 'left' }}>Поиск встреч…</span>
        <kbd>⌘K</kbd>
      </button>

      <div className="sidebar-list">
        {groups.map(({ label, items }) => (
          <div key={label} className="sidebar-group">
            <div className="sidebar-section">
              <span>{label}</span>
              <span style={{ fontSize: 11, color: 'var(--ink-4)', fontWeight: 500, letterSpacing: 0 }}>{items.length}</span>
            </div>
            {items.map(m => (
              <MeetingRow
                key={m.id}
                m={m}
                selected={selectedId === m.id}
                density={density}
                onClick={() => onSelect(m.id)}
              />
            ))}
          </div>
        ))}
        {filtered.length === 0 && (
          <div className="sidebar-empty">Ничего не найдено</div>
        )}
      </div>

      <div className="sidebar-footer">
        <SyncIndicator />
        <div style={{ flex: 1 }} />
        <button className="btn-icon btn-ghost" title="Настройки" onClick={onShowSettings}><Icon name="gear" size={15} /></button>
      </div>
    </aside>
  );
}

function MeetingRow({ m, selected, density, onClick }) {
  // What to show as secondary?
  //  - if recording: live timer + "идёт запись"
  //  - if no protocol yet: "транскрипт готов" or "нет аудио" — the exception
  //  - default: just template / duration
  let meta = null;
  if (m.recording) {
    meta = <span className="row-meta is-rec">●&nbsp;идёт запись</span>;
  } else if (!m.has_protocol) {
    meta = <span className="row-meta is-warn">черновик · {m.has_transcript ? 'нужен протокол' : 'нет транскрипта'}</span>;
  } else {
    const parts = [];
    if (m.template) parts.push(m.template.toLowerCase());
    if (m.duration) parts.push(formatDuration(m.duration));
    meta = <span className="row-meta">{parts.join(' · ')}</span>;
  }

  const rowClasses = [
    'meeting-row',
    selected ? 'is-selected' : '',
    m.recording ? 'is-recording' : '',
    !m.has_protocol && !m.recording ? 'is-draft' : '',
  ].filter(Boolean).join(' ');

  return (
    <div className={rowClasses} onClick={onClick}>
      <div className="row-body">
        <div className="row-name">{m.name}</div>
        {density !== 'compact' && meta}
      </div>
      <div className="row-time">{m.recording ? formatRecTime(m) : formatTime(m.date)}</div>
    </div>
  );
}

function SyncIndicator() {
  return (
    <div className="sync-indicator" title="Все данные хранятся локально">
      <span className="sync-dot" />
      <span>Локально</span>
    </div>
  );
}

function initials(s) {
  return s.trim().split(/\s+/).slice(0, 2).map(w => w[0]?.toUpperCase() || '').join('');
}

function formatDuration(sec) {
  if (!sec) return '';
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  if (h > 0) return `${h} ч ${m} мин`;
  return `${m} мин`;
}

function formatRecTime(m) {
  // live elapsed if set, otherwise just "сейчас"
  if (m.elapsed != null) {
    const mn = Math.floor(m.elapsed / 60);
    const s = m.elapsed % 60;
    return `${mn}:${String(s).padStart(2, '0')}`;
  }
  return 'сейчас';
}

function formatTime(d) {
  const now = new Date();
  const date = new Date(d);
  if (sameDay(date, now)) return date.toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' });
  const diff = (now - date) / (1000 * 60 * 60 * 24);
  if (diff < 7) return date.toLocaleDateString('ru-RU', { weekday: 'short' });
  return date.toLocaleDateString('ru-RU', { day: 'numeric', month: 'short' });
}

function sameDay(a, b) {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
}

function groupByDate(meetings) {
  const today = [], yesterday = [], week = [], earlier = [];
  const now = new Date();
  meetings.forEach(m => {
    const date = new Date(m.date);
    if (sameDay(date, now)) today.push(m);
    else if (sameDay(date, new Date(now - 86400000))) yesterday.push(m);
    else if ((now - date) / 86400000 < 7) week.push(m);
    else earlier.push(m);
  });
  const groups = [];
  if (today.length) groups.push({ label: 'Сегодня', items: today });
  if (yesterday.length) groups.push({ label: 'Вчера', items: yesterday });
  if (week.length) groups.push({ label: 'На этой неделе', items: week });
  if (earlier.length) groups.push({ label: 'Ранее', items: earlier });
  return groups;
}

Object.assign(window, { Sidebar });
