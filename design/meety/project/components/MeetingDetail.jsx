// Meeting detail — protocol render with 4 layout variants.
// editorial (long-form serif), cards (structured), split (protocol + transcript), edit (inline editable)

function MeetingDetail({ meeting, render, onBack, onRegenerate, onDelete }) {
  if (!meeting) return null;
  const protocol = meeting.protocol;
  const hasProto = !!protocol;
  const [menuOpen, setMenuOpen] = React.useState(false);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', minHeight: 0 }}>
      <div className="content-bar detail-bar">
        <div style={{ display: 'flex', flexDirection: 'column', gap: 4, flex: 1, minWidth: 0 }}>
          <div className="content-title" title={meeting.name}>{meeting.name}</div>
          <div className="content-subline">
            <span>{new Date(meeting.date).toLocaleString('ru-RU', { day: 'numeric', month: 'long', hour: '2-digit', minute: '2-digit' })}</span>
            <span className="dotsep" />
            <span>{formatLongDuration(meeting.duration)}</span>
            {meeting.template && (<><span className="dotsep" /><span>{meeting.template}</span></>)}
          </div>
        </div>
        <div className="content-actions">
          {hasProto && (
            <button className="btn btn-ghost" onClick={onRegenerate}>
              <Icon name="sparkle" size={14} /> Перегенерировать
            </button>
          )}
          <div style={{ position: 'relative' }}>
            <button className="btn-icon btn-ghost" onClick={() => setMenuOpen(o => !o)} aria-label="Действия"><Icon name="more" /></button>
            {menuOpen && (
              <>
                <div className="menu-scrim" onClick={() => setMenuOpen(false)} />
                <div className="menu" role="menu" onClick={() => setMenuOpen(false)}>
                  <button><Icon name="copy" size={14} /> Скопировать как markdown</button>
                  <button><Icon name="download" size={14} /> Экспорт в PDF…</button>
                  <button><Icon name="share" size={14} /> Поделиться</button>
                  <div className="menu-sep" />
                  <button><Icon name="refresh" size={14} /> Перетранскрибировать</button>
                  <button onClick={onRegenerate}><Icon name="sparkle" size={14} /> Перегенерировать протокол</button>
                  <div className="menu-sep" />
                  <button className="is-danger"><Icon name="trash" size={14} /> Удалить аудио</button>
                  <button className="is-danger"><Icon name="trash" size={14} /> Удалить встречу</button>
                </div>
              </>
            )}
          </div>
        </div>
      </div>

      <div className="content-body">
        {!hasProto && <NoProtocol meeting={meeting} onGenerate={onRegenerate} />}
        {hasProto && render === 'editorial' && <ProtocolEditorial meeting={meeting} />}
        {hasProto && render === 'cards' && <ProtocolCards meeting={meeting} />}
        {hasProto && render === 'split' && <ProtocolSplit meeting={meeting} />}
        {hasProto && render === 'edit' && <ProtocolEditable meeting={meeting} />}
      </div>
    </div>
  );
}

function NoProtocol({ meeting, onGenerate }) {
  return (
    <div className="empty">
      <div className="mark">
        <Icon name="doc" size={36} />
      </div>
      <h2>Протокол ещё не сгенерирован</h2>
      <p>У встречи есть аудиозапись. Запустите транскрипцию и генерацию протокола — это займёт пару минут.</p>
      <button className="btn btn-accent btn-lg" onClick={onGenerate}>
        <Icon name="sparkle" size={14} /> Сгенерировать протокол
      </button>
    </div>
  );
}

// ────────────────────────────────────────────────────────────────────────────
// 1. Editorial — long-form serif, calm hairlines. The default.
function ProtocolEditorial({ meeting }) {
  const p = meeting.protocol;
  return (
    <article className="protocol">
      <h1>{p.title}</h1>
      <div className="protocol-meta">
        <span>{new Date(meeting.date).toLocaleDateString('ru-RU', { day: 'numeric', month: 'long', year: 'numeric' })}</span>
        <span>·</span>
        <span>{formatLongDuration(meeting.duration)}</span>
        {p.participants && <><span>·</span><span>{p.participants.join(', ')}</span></>}
      </div>

      <h2>Краткое резюме</h2>
      <p>{p.summary}</p>

      {p.topics && p.topics.length > 0 && (
        <>
          <h2>Обсуждённые темы</h2>
          {p.topics.map((t, i) => (
            <React.Fragment key={i}>
              <h3>{t.title}</h3>
              <p>{t.body}</p>
            </React.Fragment>
          ))}
        </>
      )}

      {p.decisions && p.decisions.length > 0 && (
        <>
          <h2>Ключевые решения</h2>
          <ul>{p.decisions.map((d, i) => <li key={i}>{d}</li>)}</ul>
        </>
      )}

      {p.risks && p.risks.length > 0 && (
        <>
          <h2>Риски и проблемы</h2>
          <ul>{p.risks.map((r, i) => <li key={i}>{r}</li>)}</ul>
        </>
      )}

      {p.actions && p.actions.length > 0 && (
        <>
          <h2>Action items</h2>
          <table>
            <thead><tr><th>Задача</th><th>Ответственный</th><th>Срок</th></tr></thead>
            <tbody>
              {p.actions.map((a, i) => (
                <tr key={i}><td>{a.task}</td><td>{a.who}</td><td>{a.when || '—'}</td></tr>
              ))}
            </tbody>
          </table>
        </>
      )}

      {p.open_questions && p.open_questions.length > 0 && (
        <>
          <h2>Открытые вопросы</h2>
          <ul>{p.open_questions.map((q, i) => <li key={i}>{q}</li>)}</ul>
        </>
      )}
    </article>
  );
}

// ────────────────────────────────────────────────────────────────────────────
// 2. Cards — structured sections.
function ProtocolCards({ meeting }) {
  const p = meeting.protocol;
  return (
    <div className="proto-cards">
      <div className="protocol-meta">
        <span>{new Date(meeting.date).toLocaleDateString('ru-RU', { day: 'numeric', month: 'long', year: 'numeric' })}</span>
        <span>·</span>
        <span>{formatLongDuration(meeting.duration)}</span>
        <span>·</span>
        <span>{meeting.template}</span>
      </div>
      <h1 className="protocol-title">{p.title}</h1>

      <div className="card" style={{ background: 'var(--accent-tint)', borderColor: 'transparent' }}>
        <div className="card-label"><span className="swatch" /> Резюме</div>
        <p style={{ fontFamily: 'var(--font-serif)', fontSize: 17, lineHeight: 1.55, margin: 0 }}>{p.summary}</p>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 14 }}>
        {p.decisions && p.decisions.length > 0 && (
          <div className="card">
            <div className="card-label"><span className="swatch" style={{ background: 'var(--ok)' }} /> Решения · {p.decisions.length}</div>
            <ul>{p.decisions.map((d, i) => <li key={i}>{d}</li>)}</ul>
          </div>
        )}
        {p.risks && p.risks.length > 0 && (
          <div className="card">
            <div className="card-label"><span className="swatch" style={{ background: 'var(--warn)' }} /> Риски · {p.risks.length}</div>
            <ul>{p.risks.map((r, i) => <li key={i}>{r}</li>)}</ul>
          </div>
        )}
      </div>

      {p.topics && p.topics.length > 0 && (
        <div className="card">
          <div className="card-label"><span className="swatch" /> Обсуждённые темы · {p.topics.length}</div>
          {p.topics.map((t, i) => (
            <div key={i} style={{ paddingTop: i > 0 ? 14 : 6, borderTop: i > 0 ? '1px solid var(--rule)' : 0, marginTop: i > 0 ? 14 : 0 }}>
              <h3>{t.title}</h3>
              <p style={{ margin: '4px 0 0', color: 'var(--ink-2)' }}>{t.body}</p>
            </div>
          ))}
        </div>
      )}

      {p.actions && p.actions.length > 0 && (
        <div className="card">
          <div className="card-label"><span className="swatch" style={{ background: 'var(--accent)' }} /> Action items · {p.actions.length}</div>
          <div style={{ marginTop: 8 }}>
            {p.actions.map((a, i) => (
              <div key={i} className="action-row">
                <div>
                  <input type="checkbox" style={{ marginRight: 8, accentColor: 'var(--accent)' }} />
                  {a.task}
                </div>
                <div className="who">{a.who}</div>
                <div className="when">{a.when || '—'}</div>
              </div>
            ))}
          </div>
        </div>
      )}

      {p.open_questions && p.open_questions.length > 0 && (
        <div className="card">
          <div className="card-label"><span className="swatch" style={{ background: 'var(--ink-3)' }} /> Открытые вопросы · {p.open_questions.length}</div>
          <ul>{p.open_questions.map((q, i) => <li key={i}>{q}</li>)}</ul>
        </div>
      )}
    </div>
  );
}

// ────────────────────────────────────────────────────────────────────────────
// 3. Split — protocol + transcript side-by-side.
function ProtocolSplit({ meeting }) {
  return (
    <div className="proto-split">
      <article className="protocol">
        <h1>{meeting.protocol.title}</h1>
        <div className="protocol-meta">
          <span>{new Date(meeting.date).toLocaleDateString('ru-RU', { day: 'numeric', month: 'long' })}</span>
          <span>·</span>
          <span>{formatLongDuration(meeting.duration)}</span>
        </div>
        <h2>Резюме</h2>
        <p>{meeting.protocol.summary}</p>
        {meeting.protocol.decisions && (
          <>
            <h2>Решения</h2>
            <ul>{meeting.protocol.decisions.map((d, i) => <li key={i}>{d}</li>)}</ul>
          </>
        )}
        {meeting.protocol.actions && (
          <>
            <h2>Action items</h2>
            <table>
              <thead><tr><th>Задача</th><th>Ответственный</th><th>Срок</th></tr></thead>
              <tbody>
                {meeting.protocol.actions.map((a, i) => (
                  <tr key={i}><td>{a.task}</td><td>{a.who}</td><td>{a.when || '—'}</td></tr>
                ))}
              </tbody>
            </table>
          </>
        )}
      </article>
      <div className="transcript-pane">
        <h4>Транскрипция</h4>
        {meeting.transcript && meeting.transcript.map((u, i) => (
          <div key={i} className="utterance">
            <div className="spk">
              <span style={{ width: 8, height: 8, borderRadius: 999, background: speakerColor(u.speaker) }} />
              {u.speaker}
              <span className="ts">{u.t}</span>
            </div>
            <div>{u.text}</div>
          </div>
        ))}
      </div>
    </div>
  );
}

function speakerColor(name) {
  const colors = ['var(--accent)', 'oklch(55% 0.14 220)', 'oklch(55% 0.14 145)', 'oklch(55% 0.14 290)'];
  let h = 0;
  for (let i = 0; i < name.length; i++) h = (h * 31 + name.charCodeAt(i)) >>> 0;
  return colors[h % colors.length];
}

// ────────────────────────────────────────────────────────────────────────────
// 4. Inline editable.
function ProtocolEditable({ meeting }) {
  const p = meeting.protocol;
  return (
    <article className="protocol proto-edit">
      <h1 contentEditable suppressContentEditableWarning className="editable">{p.title}</h1>
      <div className="protocol-meta">
        <span>{new Date(meeting.date).toLocaleDateString('ru-RU', { day: 'numeric', month: 'long', year: 'numeric' })}</span>
        <span>·</span>
        <span>{formatLongDuration(meeting.duration)}</span>
        <span style={{ marginLeft: 'auto', display: 'flex', gap: 8, alignItems: 'center', color: 'var(--accent-2)' }}>
          <Icon name="edit" size={12} /> Режим редактирования
        </span>
      </div>
      <h2>Краткое резюме</h2>
      <p contentEditable suppressContentEditableWarning className="editable">{p.summary}</p>
      {p.decisions && (
        <>
          <h2>Решения</h2>
          <ul>{p.decisions.map((d, i) => <li key={i} contentEditable suppressContentEditableWarning className="editable">{d}</li>)}</ul>
        </>
      )}
      {p.actions && (
        <>
          <h2>Action items</h2>
          <table>
            <thead><tr><th>Задача</th><th>Ответственный</th><th>Срок</th></tr></thead>
            <tbody>
              {p.actions.map((a, i) => (
                <tr key={i}>
                  <td contentEditable suppressContentEditableWarning className="editable">{a.task}</td>
                  <td contentEditable suppressContentEditableWarning className="editable">{a.who}</td>
                  <td contentEditable suppressContentEditableWarning className="editable">{a.when || '—'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </article>
  );
}

function formatLongDuration(sec) {
  if (!sec) return '';
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  if (h > 0) return `${h} ч ${m} мин`;
  return `${m} мин`;
}

Object.assign(window, { MeetingDetail });
