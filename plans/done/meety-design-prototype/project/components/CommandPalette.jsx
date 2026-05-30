// Command palette — ⌘K. Lists actions + meetings, fuzzy match, keyboard nav.

function CommandPalette({ open, meetings, onClose, onSelect, onNavigate }) {
  const [q, setQ] = React.useState('');
  const [idx, setIdx] = React.useState(0);
  const inputRef = React.useRef(null);

  React.useEffect(() => {
    if (open) {
      setQ('');
      setIdx(0);
      setTimeout(() => inputRef.current?.focus(), 30);
    }
  }, [open]);

  const actions = [
    { kind: 'action', id: 'new', label: 'Новая запись', icon: 'mic', shortcut: '⌘N', run: () => onNavigate('newrec') },
    { kind: 'action', id: 'import', label: 'Импортировать аудиофайл', icon: 'doc', run: () => onNavigate('newrec') },
    { kind: 'action', id: 'settings', label: 'Открыть настройки', icon: 'gear', shortcut: '⌘,', run: () => onNavigate('settings') },
    { kind: 'action', id: 'welcome', label: 'Показать первый запуск', icon: 'sparkle', run: () => onNavigate('welcome') },
  ];

  const items = React.useMemo(() => {
    const norm = q.trim().toLowerCase();
    const meetingItems = meetings
      .filter(m => !m.recording)
      .map(m => ({ kind: 'meeting', id: m.id, label: m.name, sub: m.template || '', date: m.date, m }));
    const all = [...actions, ...meetingItems];
    if (!norm) return all.slice(0, 12);
    return all.filter(i => i.label.toLowerCase().includes(norm)).slice(0, 20);
  }, [q, meetings]);

  React.useEffect(() => { setIdx(0); }, [q]);

  React.useEffect(() => {
    if (!open) return;
    const onKey = (e) => {
      if (e.key === 'Escape') { e.preventDefault(); onClose(); }
      else if (e.key === 'ArrowDown') { e.preventDefault(); setIdx(i => Math.min(items.length - 1, i + 1)); }
      else if (e.key === 'ArrowUp') { e.preventDefault(); setIdx(i => Math.max(0, i - 1)); }
      else if (e.key === 'Enter') {
        e.preventDefault();
        const it = items[idx];
        if (!it) return;
        if (it.kind === 'action') it.run();
        else if (it.kind === 'meeting') onSelect(it.id);
        onClose();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, items, idx]);

  if (!open) return null;

  return (
    <div className="palette-scrim" onClick={onClose}>
      <div className="palette" onClick={(e) => e.stopPropagation()}>
        <div className="palette-input">
          <Icon name="search" size={16} />
          <input
            ref={inputRef}
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Команда или название встречи…"
          />
          <kbd>ESC</kbd>
        </div>
        <div className="palette-list">
          {items.length === 0 && (
            <div className="palette-empty">Ничего не найдено</div>
          )}
          {groupItems(items).map((g, gi) => (
            <div key={gi}>
              <div className="palette-section">{g.label}</div>
              {g.items.map((it) => {
                const flatIdx = items.indexOf(it);
                const active = flatIdx === idx;
                return (
                  <div
                    key={it.id}
                    className={`palette-item ${active ? 'is-active' : ''}`}
                    onMouseEnter={() => setIdx(flatIdx)}
                    onClick={() => {
                      if (it.kind === 'action') it.run();
                      else if (it.kind === 'meeting') onSelect(it.id);
                      onClose();
                    }}
                  >
                    <span className="palette-icn">
                      {it.kind === 'action' && <Icon name={it.icon} size={15} />}
                      {it.kind === 'meeting' && (
                        <span className="palette-glyph">{(it.label || '').trim()[0]?.toUpperCase()}</span>
                      )}
                    </span>
                    <span className="palette-label">{it.label}</span>
                    <span className="palette-sub">{it.kind === 'meeting' ? (it.sub || '') : ''}</span>
                    {it.shortcut && <kbd>{it.shortcut}</kbd>}
                  </div>
                );
              })}
            </div>
          ))}
        </div>
        <div className="palette-footer">
          <span><kbd>↑</kbd><kbd>↓</kbd> навигация</span>
          <span><kbd>↵</kbd> открыть</span>
          <span><kbd>ESC</kbd> закрыть</span>
        </div>
      </div>
    </div>
  );
}

function groupItems(items) {
  const actions = items.filter(i => i.kind === 'action');
  const meetings = items.filter(i => i.kind === 'meeting');
  const groups = [];
  if (actions.length) groups.push({ label: 'Действия', items: actions });
  if (meetings.length) groups.push({ label: 'Встречи', items: meetings });
  return groups;
}

Object.assign(window, { CommandPalette });
