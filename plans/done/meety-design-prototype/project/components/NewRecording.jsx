// New recording + live recording — 3 hero variants for the live state.
// idle  : the entry form (name, source, drop zone, import)
// orb   : warm glass orb + big serif timer  (default — "surprise me")
// wave  : audio waveform + level meter
// trans : streaming live transcript

function NewRecording({ state, onBack, onStart, onStop, onImport, settings }) {
  const [name, setName] = React.useState('');
  const [source, setSource] = React.useState(settings?.source || 'mixed');
  const [echoCancel, setEchoCancel] = React.useState(settings?.echoCancel || false);

  if (state.kind === 'idle') {
    return (
      <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
        <div className="content-bar">
          <button className="btn btn-ghost" onClick={onBack}><Icon name="arrow-left" size={14} /> Назад</button>
          <div className="content-title">Новая запись</div>
        </div>
        <div className="content-body">
          <div className="newrec">
            <h1>Готовы записать?</h1>
            <p className="sub">Дайте встрече название — остальное meety сделает сам.</p>

            <label className="label">Название встречи</label>
            <div className="field">
              <input
                placeholder="Например, «Дейлик с командой Платформы»"
                value={name}
                onChange={(e) => setName(e.target.value)}
                autoFocus
              />
            </div>

            <div className="spacer-sm" />

            <label className="label">Источник звука</label>
            <div className="segmented" style={{ display: 'flex', width: '100%' }}>
              {[
                { v: 'mic', l: 'Микрофон' },
                { v: 'system', l: 'Система' },
                { v: 'mixed', l: 'Оба источника' },
              ].map(o => (
                <button
                  key={o.v}
                  className={source === o.v ? 'is-active' : ''}
                  onClick={() => setSource(o.v)}
                  style={{ flex: 1 }}
                >{o.l}</button>
              ))}
            </div>
            <div className="help">Запись системного звука требует разрешения «Запись экрана» в macOS — meety записывает только звук.</div>

            <div className="spacer-sm" />

            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '14px 0', borderTop: '1px solid var(--rule)', borderBottom: '1px solid var(--rule)' }}>
              <div>
                <div style={{ fontSize: 13.5, fontWeight: 500 }}>Подавление эха</div>
                <div className="help" style={{ marginTop: 2 }}>Рекомендуется при записи через микрофон.</div>
              </div>
              <button
                className="switch"
                data-on={echoCancel}
                onClick={() => setEchoCancel(!echoCancel)}
                aria-label="toggle echo cancel"
              />
            </div>

            <div className="dropzone" onClick={onImport}>
              <strong>Перетащите аудиофайл сюда</strong>
              wav · mp3 · m4a · flac · ogg — meety импортирует и поставит в очередь на транскрипцию
            </div>

            <div className="spacer-sm" />

            <button className="btn btn-accent btn-lg" style={{ width: '100%', justifyContent: 'center' }} onClick={() => onStart(name)}>
              <Icon name="mic" size={14} /> Начать запись
            </button>
            <div className="import-row">
              <button className="btn" onClick={onImport}><Icon name="doc" size={14} /> Импорт файла…</button>
              <button className="btn"><Icon name="folder" size={14} /> Из папки…</button>
            </div>
          </div>
        </div>
      </div>
    );
  }

  // recording state — switch hero by variant
  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      <div className="content-bar">
        <span className="rec-label">Запись</span>
        <div style={{ flex: 1 }} />
        <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--ink-3)' }}>
          {sourceLabel(source)} · {echoCancel ? 'AEC вкл' : 'AEC выкл'}
        </span>
      </div>
      <div className="content-body">
        {state.variant === 'orb' && <RecOrb state={state} onStop={onStop} />}
        {state.variant === 'wave' && <RecWave state={state} onStop={onStop} />}
        {state.variant === 'trans' && <RecTrans state={state} onStop={onStop} />}
      </div>
    </div>
  );
}

function sourceLabel(s) {
  return { mic: 'Микрофон', system: 'Система', mixed: 'Оба источника' }[s] || '';
}

function fmt(sec) {
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
}

// Simulate a believable audio envelope. Returns 0..1.
// Combines a slow breath cycle + speech-like bursts + light noise.
function useAudioEnvelope(elapsed, vt) {
  // vt is a faster tick (~10Hz). Returns smoothed value 0..1.
  const [level, setLevel] = React.useState(0);
  const targetRef = React.useRef(0);
  React.useEffect(() => {
    // a "syllable" rhythm — pulse roughly every 200-400ms
    const t = vt / 10;
    const breath = 0.25 + 0.15 * Math.sin(t * 0.6);
    const burst = Math.max(0, Math.sin(t * 6) * 0.4 + Math.sin(t * 11.3) * 0.25);
    const speech = (Math.sin(t * 2.1) > 0.2 ? burst : 0);
    const noise = (Math.random() - 0.5) * 0.06;
    targetRef.current = Math.max(0.05, Math.min(1, breath + speech + noise));
    // smooth toward target
    setLevel(prev => prev + (targetRef.current - prev) * 0.35);
  }, [vt]);
  return level;
}

// Variant 1 — heroic typographic mark. The brand letterform at huge scale,
// breathing with the audio envelope. Ink saturates with voice; the dot pulses.
function RecOrb({ state, onStop }) {
  const level = useAudioEnvelope(state.elapsed, state.vt || 0);
  // saturation, scale and ink-bleed all driven by envelope
  const opacity = 0.42 + level * 0.55;
  const scale = 0.985 + level * 0.025;
  const bleed = level * 22; // px of blur for the ink-bleed shadow

  return (
    <div className="rec-stage">
      <div className="rec-glyph-wrap">
        <div className="rec-glyph" style={{
          opacity,
          transform: `scale(${scale})`,
          textShadow: `0 0 ${bleed}px oklch(22% 0.012 65 / ${0.15 + level * 0.25})`,
        }}>
          <span className="rec-glyph-letter">m</span>
          <span className="rec-glyph-dot" style={{
            opacity: 0.6 + level * 0.4,
            transform: `scale(${0.9 + level * 0.3})`,
          }} />
        </div>
        <div className="rec-glyph-baseline" />
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 6 }}>
        <div className="rec-timer">{fmt(state.elapsed)}<span className="ms">.{String(Math.floor(((state.vt || 0)) % 10))}</span></div>
        <div className="rec-name">«{state.name}»</div>
      </div>
      <div style={{ display: 'flex', gap: 10, marginTop: 4 }}>
        <button className="btn btn-lg"><Icon name="pause" size={14} /> Пауза</button>
        <button className="btn btn-accent btn-lg" onClick={onStop}><Icon name="stop" size={14} /> Остановить</button>
      </div>
    </div>
  );
}

// Variant 2 — honest mirrored waveform. Computed synchronously so the bars
// always reflect the current envelope; no ring-buffer race.
function RecWave({ state, onStop }) {
  const N = 72;
  const level = useAudioEnvelope(state.elapsed, state.vt || 0);
  // Generate a history-like view using sine families seeded by vt — gives a
  // believable speech waveform that visibly moves left as state.vt increments.
  const t = (state.vt || 0);
  const bars = React.useMemo(() => {
    const arr = [];
    for (let i = 0; i < N; i++) {
      // x is "age" of this bar — i=0 oldest, i=N-1 newest (the "now" edge).
      const x = (N - 1 - i) + t * 0.5;
      const wave =
        Math.abs(Math.sin(x * 0.35)) * 0.55 +
        Math.abs(Math.sin(x * 0.13 + 1.2)) * 0.35 +
        Math.abs(Math.sin(x * 0.71 + 0.4)) * 0.25;
      // bias newest bars by the current level
      const proximity = Math.max(0, 1 - (N - 1 - i) / 12);
      const v = Math.max(0.04, Math.min(1, wave * (0.5 + level * 0.8) * (1 + proximity * 0.3)));
      arr.push(v);
    }
    return arr;
  }, [t, level]);
  const dbLevel = level > 0 ? Math.max(-60, Math.round(20 * Math.log10(level + 0.001))) : -60;

  return (
    <div className="rec-stage">
      <div className="rec-label">Запись · {sourceLabel(state.source)}</div>
      <div style={{ display: 'flex', alignItems: 'baseline', gap: 20 }}>
        <div className="rec-timer">{fmt(state.elapsed)}</div>
        <div style={{ fontFamily: 'var(--font-mono)', fontSize: 13, color: 'var(--ink-3)', minWidth: 60 }}>
          {dbLevel} dB
        </div>
      </div>
      <div className="rec-name">«{state.name}»</div>
      <div className="waveform-honest">
        {bars.map((v, i) => (
          <div key={i} className="bar-pair">
            <div className="bar bar-up" style={{ height: `${5 + v * 95}%` }} />
            <div className="bar bar-down" style={{ height: `${5 + v * 95}%` }} />
          </div>
        ))}
        <div className="wave-now" />
      </div>
      <div style={{ width: '100%', maxWidth: 600, display: 'flex', justifyContent: 'space-between', fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--ink-4)', padding: '0 4px' }}>
        <span>−60</span><span>−40</span><span>−20</span><span>0 dB</span>
      </div>
      <div style={{ display: 'flex', gap: 10, marginTop: 18 }}>
        <button className="btn btn-lg"><Icon name="pause" size={14} /> Пауза</button>
        <button className="btn btn-accent btn-lg" onClick={onStop}><Icon name="stop" size={14} /> Остановить</button>
      </div>
    </div>
  );
}

// Variant 3 — iA Writer-style focus on the latest utterance.
function RecTrans({ state, onStop }) {
  const text = LIVE_LINES.slice(0, Math.min(LIVE_LINES.length, Math.floor(state.elapsed / 2) + 1));
  const ref = React.useRef(null);
  React.useEffect(() => { if (ref.current) ref.current.scrollTop = ref.current.scrollHeight; }, [text.length]);
  const level = useAudioEnvelope(state.elapsed, state.vt || 0);

  return (
    <div className="rec-stage" style={{ gap: 18, padding: '32px 60px' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 24, width: '100%', maxWidth: 720, justifyContent: 'space-between' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 18 }}>
          <span className="rec-label">Запись</span>
          <div className="rec-timer" style={{ fontSize: 36 }}>{fmt(state.elapsed)}</div>
        </div>
        <div style={{ display: 'flex', gap: 2, alignItems: 'center', height: 22 }}>
          {[0,1,2,3,4,5,6,7,8,9].map(i => {
            const phase = (state.vt || 0) * 0.6 + i * 1.2;
            const v = 0.2 + 0.8 * Math.abs(Math.sin(phase)) * level * 1.5;
            return <div key={i} style={{ width: 2.5, height: `${Math.max(10, Math.min(100, v * 100))}%`, background: 'var(--accent)', borderRadius: 999, transition: 'height 0.06s linear' }} />;
          })}
        </div>
      </div>
      <div className="live-focus" ref={ref}>
        {text.length === 0 && (
          <div className="live-line is-current">
            <div className="live-spk">слушаю</div>
            <div className="live-body"><span className="live-cursor" /></div>
          </div>
        )}
        {text.map((u, i) => {
          const isCurrent = i === text.length - 1;
          return (
            <div key={i} className={`live-line ${isCurrent ? 'is-current' : ''}`}>
              <div className="live-spk">{u.spk}</div>
              <div className="live-body">
                {isCurrent ? <FocusedText text={u.text} elapsed={state.elapsed} /> : u.text}
              </div>
            </div>
          );
        })}
      </div>
      <div style={{ display: 'flex', gap: 10 }}>
        <button className="btn btn-lg"><Icon name="pause" size={14} /> Пауза</button>
        <button className="btn btn-accent btn-lg" onClick={onStop}><Icon name="stop" size={14} /> Остановить</button>
      </div>
    </div>
  );
}

// Reveals words progressively + highlights the most-recent one (iA Writer focus).
function FocusedText({ text, elapsed }) {
  const words = text.split(' ');
  // 2 seconds per line, distribute over words
  const wordsRevealed = Math.min(words.length, Math.max(1, Math.floor((elapsed % 2) * words.length / 2) + 1));
  return (
    <>
      {words.slice(0, wordsRevealed - 1).join(' ')}
      {wordsRevealed > 1 && ' '}
      <span className="word-focus">{words[wordsRevealed - 1]}</span>
      <span className="live-cursor" />
    </>
  );
}

const LIVE_LINES = [
  { spk: 'Аня', text: 'Сегодня хотела пробежаться по статусу платёжного флоу.' },
  { spk: 'Кирилл', text: 'Со стороны бэкенда — закончили интеграцию с Тинькофф, остался ретрай.' },
  { spk: 'Аня', text: 'Какие сроки на ретрай?' },
  { spk: 'Кирилл', text: 'Две недели максимум. Заблокированы тем, что нужен access к staging-окружению.' },
  { spk: 'Маша', text: 'Я подниму доступ сегодня. У нас уже есть тикет в IT.' },
  { spk: 'Аня', text: 'Ок. Тогда план такой: ретрай к концу спринта, выкатываем в прод следующей средой.' },
  { spk: 'Кирилл', text: 'Договорились.' },
];

Object.assign(window, { NewRecording });
