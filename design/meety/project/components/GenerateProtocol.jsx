// Generate protocol — progress flow with LIVE content leaking through.
// Step 1: actual transcript lines stream in. Step 2: protocol sections
// materialize one by one. The user can read along as the machine works.

// content shown during transcription (chunks of speech)
const GEN_TRANSCRIPT = [
  { spk: 'Аня (PM)', text: 'Спасибо что собрались. Давайте быстро по статусу платёжного флоу.' },
  { spk: 'Кирилл (Backend)', text: 'Со стороны бэка — закончили интеграцию с Тинькофф, прошли тесты в песочнице.' },
  { spk: 'Кирилл (Backend)', text: 'Остался механизм ретраев — он на этой неделе.' },
  { spk: 'Аня (PM)', text: 'Какие сроки на ретрай?' },
  { spk: 'Кирилл (Backend)', text: 'Две недели. Заблокированы — нужен доступ к staging-окружению.' },
  { spk: 'Маша (DevOps)', text: 'Доступ подниму сегодня, у нас уже есть тикет в IT.' },
  { spk: 'Аня (PM)', text: 'Отлично. План: ретрай к концу спринта, выкатываем в прод следующей средой.' },
];

// content built up during protocol generation, in order of appearance
const GEN_PROTOCOL_STEPS = [
  { type: 'title', text: 'Платёжный флоу — статус и план релиза' },
  { type: 'h2', text: 'Краткое резюме' },
  { type: 'p', text: 'Команда обсудила интеграцию с Тинькофф и план выхода в прод следующей средой.' },
  { type: 'h2', text: 'Ключевые решения' },
  { type: 'bullet', text: 'Выкатка платёжного флоу в прод — следующая среда (28 мая).' },
  { type: 'bullet', text: 'Использовать отдельный кластер под платёжный сервис.' },
  { type: 'h2', text: 'Action items · 3' },
  { type: 'action', task: 'Поднять доступ к staging', who: 'Маша', when: 'Сегодня' },
  { type: 'action', task: 'Закончить механизм ретраев', who: 'Кирилл', when: 'Конец спринта' },
  { type: 'action', task: 'Согласовать план релиза со SRE', who: 'Аня', when: '26 мая' },
];

function GenerateProtocol({ meeting, onBack, onDone }) {
  // 0 = подготовка, 1 = транскрипция, 2 = протокол, 3 = done
  const [step, setStep] = React.useState(0);
  const [percent, setPercent] = React.useState(0);
  const [transcriptIdx, setTranscriptIdx] = React.useState(0);
  const [protoIdx, setProtoIdx] = React.useState(0);

  // step machine
  React.useEffect(() => {
    if (step === 0) {
      const t = setTimeout(() => { setStep(1); setPercent(0); }, 1100);
      return () => clearTimeout(t);
    }
    if (step === 1) {
      // reveal one transcript chunk every ~700ms while ticking percent
      const lineInt = setInterval(() => {
        setTranscriptIdx(i => {
          if (i >= GEN_TRANSCRIPT.length) return i;
          return i + 1;
        });
      }, 700);
      const pctInt = setInterval(() => {
        setPercent(p => {
          if (p >= 100) return 100;
          return Math.min(100, p + 1.4);
        });
      }, 70);
      const finish = setTimeout(() => { setStep(2); setPercent(0); }, 5400);
      return () => { clearInterval(lineInt); clearInterval(pctInt); clearTimeout(finish); };
    }
    if (step === 2) {
      const int = setInterval(() => {
        setProtoIdx(i => {
          if (i >= GEN_PROTOCOL_STEPS.length) return i;
          return i + 1;
        });
      }, 420);
      const finish = setTimeout(() => { setStep(3); onDone && onDone(); }, GEN_PROTOCOL_STEPS.length * 420 + 800);
      return () => { clearInterval(int); clearTimeout(finish); };
    }
  }, [step]);

  const transcriptLines = GEN_TRANSCRIPT.slice(0, transcriptIdx);
  const protoLines = GEN_PROTOCOL_STEPS.slice(0, protoIdx);
  const bodyRef = React.useRef(null);
  const transcriptRef = React.useRef(null);
  const protoRef = React.useRef(null);

  // Keep the latest streamed content in view as it grows.
  React.useEffect(() => {
    const el = bodyRef.current;
    if (el) el.scrollTo({ top: el.scrollHeight, behavior: 'smooth' });
  }, [step]);
  React.useEffect(() => {
    const el = transcriptRef.current;
    if (el) el.scrollTo({ top: el.scrollHeight, behavior: 'smooth' });
  }, [transcriptLines.length]);
  React.useEffect(() => {
    const el = protoRef.current;
    if (el) el.scrollTo({ top: el.scrollHeight, behavior: 'smooth' });
  }, [protoLines.length]);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      <div className="content-bar">
        <button className="btn btn-ghost" onClick={onBack}><Icon name="arrow-left" size={14} /> Назад</button>
        <div className="content-title">Генерация протокола</div>
        <div className="content-actions">
          <span className="gen-status-pill">
            <span className="gen-dot" /> {stepLabel(step)}
          </span>
        </div>
      </div>

      <div className="content-body gen-body" ref={bodyRef}>
        <div className="gen-shell">
          <div className="gen-header">
            <div className="gen-template">{meeting.template || 'Простой протокол'}</div>
            <div className="gen-title">{meeting.name}</div>
            <div className="gen-meta">{formatLongDuration(meeting.duration)} · {sourceLabelShort(meeting.source)}</div>

            <div className="steps">
              <Step idx={0} step={step} label="Подготовка" />
              <Step idx={1} step={step} label="Транскрипция" />
              <Step idx={2} step={step} label="Протокол" />
            </div>

            <div className={`progressbar ${step === 0 || step === 2 ? 'indeterminate' : ''}`}>
              <div style={{ width: step === 1 ? `${percent}%` : (step >= 2 ? '100%' : undefined) }} />
            </div>
            <div className="progress-sub">
              <span>{subLine(step, meeting, percent)}</span>
              <span>{step === 1 ? `${Math.floor(percent)} %` : ''}</span>
            </div>
          </div>

          {/* LEAK: transcription content as it streams */}
          {step >= 1 && (
            <div className="gen-leak">
              <div className="gen-leak-label">
                {step === 1 ? 'Распознаётся в реальном времени' : 'Транскрипция готова'}
              </div>
              <div className={`gen-transcript ${step === 1 ? 'is-streaming' : 'is-frozen'}`} ref={transcriptRef}>
                {transcriptLines.length === 0 && (
                  <div className="gen-listening">слушаю…<span className="live-cursor" /></div>
                )}
                {transcriptLines.map((u, i) => {
                  const isLatest = step === 1 && i === transcriptLines.length - 1;
                  return (
                    <div key={i} className={`gen-utt ${isLatest ? 'is-latest' : ''}`}>
                      <span className="gen-spk">{u.spk}</span>
                      <span className="gen-text">{u.text}{isLatest && <span className="live-cursor" />}</span>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* LEAK: protocol as it materializes */}
          {step >= 2 && (
            <div className="gen-leak">
              <div className="gen-leak-label">
                {step === 2 ? 'Claude сводит в протокол' : 'Готово'}
              </div>
              <div className="gen-proto" ref={protoRef}>
                {protoLines.map((it, i) => {
                  if (it.type === 'title') return <h1 key={i} className="gen-proto-title">{it.text}</h1>;
                  if (it.type === 'h2') return <h2 key={i} className="gen-proto-h2">{it.text}</h2>;
                  if (it.type === 'p') return <p key={i} className="gen-proto-p">{it.text}</p>;
                  if (it.type === 'bullet') return <div key={i} className="gen-proto-bullet"><span className="bull">•</span> {it.text}</div>;
                  if (it.type === 'action') return (
                    <div key={i} className="gen-proto-action">
                      <span className="task">{it.task}</span>
                      <span className="who">{it.who}</span>
                      <span className="when">{it.when}</span>
                    </div>
                  );
                  return null;
                })}
                {step === 2 && protoIdx < GEN_PROTOCOL_STEPS.length && (
                  <span className="live-cursor" style={{ marginLeft: 2 }} />
                )}
              </div>
            </div>
          )}

          <div className="gen-actions">
            {step === 3 ? (
              <button className="btn btn-accent btn-lg" onClick={() => onDone && onDone()}>
                <Icon name="check" size={14} /> Открыть протокол
              </button>
            ) : (
              <button className="btn btn-ghost" onClick={onBack}>Отменить</button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function Step({ idx, step, label }) {
  const cls = idx < step ? 'is-done' : idx === step ? 'is-active' : '';
  return (
    <div className={`step ${cls}`}>
      <div className="marker">
        {idx < step ? <Icon name="check" size={14} /> : (idx + 1)}
      </div>
      <div className="step-label">{label}</div>
    </div>
  );
}

function stepLabel(step) {
  return ['Подготовка', 'Распознавание речи', 'Составление протокола', 'Готово'][step] || '';
}

function subLine(step, meeting, percent) {
  if (step === 0) return 'Загружаю Whisper large-v3…';
  if (step === 1) return `Whisper · ${meeting.duration ? Math.floor(meeting.duration / 60) : 0} мин аудио · большая модель`;
  if (step === 2) return 'Claude · Sonnet 4.5 · читает 8 200 токенов';
  if (step === 3) return 'Готово — протокол сохранён локально';
  return '';
}

function sourceLabelShort(s) {
  return { mic: 'микрофон', system: 'система', mixed: 'оба источника' }[s] || 'аудио';
}

function formatLongDuration(sec) {
  if (!sec) return '';
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  if (h > 0) return `${h} ч ${m} мин`;
  return `${m} мин`;
}

Object.assign(window, { GenerateProtocol });
