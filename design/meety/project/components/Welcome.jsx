// Welcome — first-run AND empty state. The hero shows an actual mockup of a
// rendered protocol so users see what they're going to get.

function Welcome({ onStart, onImport, compact = false }) {
  if (compact) {
    return (
      <div className="empty">
        <Wordmark size={56} />
        <div style={{ height: 4 }} />
        <h2>Выберите встречу</h2>
        <p>Или начните новую запись — meety запишет, расшифрует и подготовит протокол.</p>
        <div style={{ display: 'flex', gap: 8, marginTop: 6, flexWrap: 'wrap', justifyContent: 'center' }}>
          <button className="btn btn-accent" onClick={onStart}><Icon name="mic" size={14} /> Новая запись</button>
          <button className="btn" onClick={onImport}><Icon name="doc" size={14} /> Импорт</button>
        </div>
      </div>
    );
  }

  return (
    <div className="welcome">
      <div className="welcome-hero">
        <div style={{ position: 'relative', zIndex: 1, width: '100%' }}>
          <ProtocolMockup />
        </div>
      </div>
      <div className="welcome-form">
        <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 4 }}>
          <Wordmark size={36} />
          <span className="brand" style={{ fontSize: 26 }}>meety</span>
        </div>
        <h1>Тихий ассистент<br />для шумных встреч.</h1>
        <div className="sub">Запиши, расшифруй, оформи в&nbsp;протокол. Локально. По‑русски.</div>

        <div className="bullet">
          <div className="ord">1</div>
          <div className="desc"><b>Разрешите «Запись экрана»</b> — для захвата системного звука. Никаких снимков экрана meety не делает.</div>
        </div>
        <div className="bullet">
          <div className="ord">2</div>
          <div className="desc"><b>Скачайте модель Whisper</b> · ≈3 ГБ · large-v3. Транскрипция идёт локально, без интернета.</div>
        </div>
        <div className="bullet">
          <div className="ord">3</div>
          <div className="desc"><b>Добавьте API-ключ Claude</b> — для генерации протоколов. Ключ хранится в системном Keychain.</div>
        </div>

        <div className="spacer-sm" />

        <button className="btn btn-accent btn-lg" onClick={onStart} style={{ alignSelf: 'flex-start' }}>
          <Icon name="mic" size={14} /> Начать первую запись
        </button>
        <button className="btn btn-ghost" onClick={onImport} style={{ alignSelf: 'flex-start' }}>
          У меня уже есть запись — импортировать
        </button>
      </div>
    </div>
  );
}

function ProtocolMockup() {
  return (
    <div className="welcome-mockup">
      <div className="mockup-page">
        <div className="mock-meta">21 мая · Командная встреча</div>
        <h3>Платёжный флоу — план релиза</h3>
        <div className="mock-sub">47 мин · Аня, Кирилл, Маша</div>

        <div className="mock-section">Резюме</div>
        <p>Команда обсудила <span className="mock-highlight">интеграцию с Тинькофф</span> и план выхода в&nbsp;прод следующей средой.</p>

        <div className="mock-section">Action items · 3</div>
        <div className="mock-row"><span>Поднять доступ к staging</span><span className="who">Маша · сегодня</span></div>
        <div className="mock-row"><span>Закончить ретрай</span><span className="who">Кирилл · до пятницы</span></div>
        <div className="mock-row"><span>Согласовать релиз со SRE</span><span className="who">Аня · 26 мая</span></div>
      </div>
    </div>
  );
}

Object.assign(window, { Welcome });
