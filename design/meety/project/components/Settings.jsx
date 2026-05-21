// Settings — macOS-style category sidebar inside the content pane,
// matching the existing screen's information architecture but quietened.

function Settings({ onBack }) {
  const [tab, setTab] = React.useState('transcription');
  const tabs = [
    { id: 'transcription', label: 'Транскрипция', icon: 'mic' },
    { id: 'llm',           label: 'LLM-провайдер', icon: 'sparkle' },
    { id: 'templates',     label: 'Шаблоны', icon: 'book' },
    { id: 'storage',       label: 'Хранилище', icon: 'storage' },
    { id: 'recording',     label: 'Запись', icon: 'speakers' },
  ];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      <div className="content-bar">
        <button className="btn btn-ghost" onClick={onBack}><Icon name="arrow-left" size={14} /> Назад</button>
        <div className="content-title">Настройки</div>
        <div className="content-actions">
          <button className="btn btn-ghost">Сбросить</button>
          <button className="btn btn-primary">Сохранить</button>
        </div>
      </div>
      <div className="content-body" style={{ overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
        <div className="settings">
          <nav className="settings-nav">
            {tabs.map(t => (
              <button key={t.id} className={tab === t.id ? 'is-active' : ''} onClick={() => setTab(t.id)}>
                <Icon name={t.icon} size={15} /> {t.label}
              </button>
            ))}
          </nav>

          <div className="settings-pane">
            {tab === 'transcription' && <TranscriptionPane />}
            {tab === 'llm'           && <LlmPane />}
            {tab === 'templates'     && <TemplatesPane />}
            {tab === 'storage'       && <StoragePane />}
            {tab === 'recording'     && <RecordingPane />}
          </div>
        </div>
      </div>
    </div>
  );
}

function TranscriptionPane() {
  const [model, setModel] = React.useState('large-v3');
  const [device, setDevice] = React.useState('mps');
  const [diarize, setDiarize] = React.useState(true);
  return (
    <>
      <h2>Транскрипция</h2>
      <p className="pane-sub">Whisper работает локально на вашем устройстве. Аудио никогда не покидает Mac.</p>

      <div className="row">
        <div>
          <div className="label">Модель</div>
          <div className="help">Чем крупнее модель, тем точнее распознавание и медленнее обработка.</div>
        </div>
        <div className="field-wrap">
          <div className="segmented" style={{ display: 'flex' }}>
            {['base', 'small', 'medium', 'large-v3'].map(m => (
              <button key={m} className={model === m ? 'is-active' : ''} onClick={() => setModel(m)} style={{ flex: 1 }}>{m}</button>
            ))}
          </div>
          <div className="help" style={{ marginTop: 8 }}>
            {model === 'large-v3' ? '≈ 0.4× реального времени · ~3 GB RAM' : '~ быстрее, но менее точно'}
          </div>
        </div>
      </div>

      <div className="row">
        <div>
          <div className="label">Устройство</div>
          <div className="help">Apple Silicon — Metal Performance Shaders. CPU — fallback.</div>
        </div>
        <div className="field-wrap">
          <div className="segmented">
            <button className={device === 'mps' ? 'is-active' : ''} onClick={() => setDevice('mps')}>MPS (рекомендуется)</button>
            <button className={device === 'cpu' ? 'is-active' : ''} onClick={() => setDevice('cpu')}>CPU</button>
          </div>
        </div>
      </div>

      <div className="row">
        <div>
          <div className="label">Разделение по говорящим</div>
          <div className="help">Маркирует фрагменты как «Спикер 1», «Спикер 2» и т. д.</div>
        </div>
        <div className="field-wrap" style={{ display: 'flex', justifyContent: 'flex-end' }}>
          <button className="switch" data-on={diarize} onClick={() => setDiarize(!diarize)} />
        </div>
      </div>

      <div className="row">
        <div>
          <div className="label">Язык</div>
          <div className="help">«Автоматически» определит по содержимому — обычно работает хорошо.</div>
        </div>
        <div className="field-wrap">
          <div className="field"><input defaultValue="Автоматически" /></div>
        </div>
      </div>
    </>
  );
}

function LlmPane() {
  return (
    <>
      <h2>LLM-провайдер</h2>
      <p className="pane-sub">meety использует большую модель для превращения транскрипта в структурированный протокол.</p>

      <div className="row">
        <div>
          <div className="label">Провайдер</div>
          <div className="help">Anthropic Claude — рекомендуется для русского языка и техконтекста.</div>
        </div>
        <div className="field-wrap">
          <div className="segmented">
            <button className="is-active">Anthropic</button>
            <button>OpenAI</button>
            <button>Local (Ollama)</button>
          </div>
        </div>
      </div>

      <div className="row">
        <div>
          <div className="label">API-ключ</div>
          <div className="help" style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            <Icon name="key" size={12} /> Хранится в системном Keychain
          </div>
        </div>
        <div className="field-wrap">
          <div className="field">
            <input type="password" defaultValue="sk-ant-api03-•••••••••••••••••••••••••••••••••••••••••" />
            <button className="btn btn-ghost" style={{ padding: '2px 6px' }}><Icon name="eye" size={14} /></button>
          </div>
          <div className="help" style={{ marginTop: 8, color: 'var(--ok)' }}>
            <Icon name="check" size={12} /> Ключ действителен · последняя проверка минуту назад
          </div>
        </div>
      </div>

      <div className="row">
        <div>
          <div className="label">Модель</div>
          <div className="help">Sonnet — баланс качества и стоимости. Opus — для длинных встреч.</div>
        </div>
        <div className="field-wrap">
          <div className="segmented">
            <button>Haiku</button>
            <button className="is-active">Sonnet 4.5</button>
            <button>Opus 4</button>
          </div>
        </div>
      </div>
    </>
  );
}

function TemplatesPane() {
  const [active, setActive] = React.useState('simple');

  const templates = {
    simple: {
      name: 'Простой протокол',
      desc: 'Универсальный — тип, резюме, темы, решения',
      icon: 'doc',
      meetings: 32,
      isDefault: true,
      prompt: `Ты — ассистент по составлению протоколов встреч и разговоров.

На основе транскрипции составь краткий, структурированный протокол на русском языке.

Требования:
- Убирай повторы, шум и несущественные детали
- Не додумывай факты, если они не были явно сказаны
- Пиши кратко и по делу
- Определи тип встречи (рабочая, интервью, обсуждение, обучение)

Структура:
## Тип встречи
## Краткое резюме
## Ключевые темы
## Решения
## Задачи / Дальнейшие действия
## Важные выводы / инсайты
## Открытые вопросы

Транскрипция:
{transcript}`,
    },
    '1on1': {
      name: '1-на-1',
      desc: 'Менеджер и сотрудник: статус, блокеры, обратная связь',
      icon: 'user',
      meetings: 14,
      prompt: `Ты — опытный ассистент руководителя, специализирующийся на протоколах встреч 1-на-1.

Контекст: это встреча между менеджером и сотрудником его команды. Разговор ведётся на русском языке, может содержать технические термины и названия задач/проектов.

На основе транскрипции составь структурированный протокол. Используй только то, что реально прозвучало.

## Краткое резюме
## Статус задач и проектов
## Проблемы и блокеры
## Обратная связь
## Решения и договорённости
## Action items
| Задача | Ответственный | Срок |
## Следующая встреча

Транскрипция встречи "{meeting_name}":
{transcript}`,
    },
    team: {
      name: 'Командная встреча',
      desc: 'Цель, темы с под-разделами, решения, риски',
      icon: 'team',
      meetings: 8,
      prompt: `Ты — опытный ассистент руководителя, специализирующийся на протоколах командных встреч.

## Цель встречи
## Участники
## Краткое резюме
## Обсуждённые темы
### [Название темы]
## Ключевые решения
## Риски и проблемы
## Action items
| Задача | Ответственный | Срок |
## Открытые вопросы

Транскрипция встречи "{meeting_name}":
{transcript}`,
    },
    daily: {
      name: 'Дейлик',
      desc: 'Сделано · план · блокеры по каждому участнику',
      icon: 'cpu',
      meetings: 71,
      prompt: `Ты — ассистент руководителя технической команды, специализирующийся на протоколах ежедневных стендапов.

## Статус по участникам
### [Имя участника]
- **Сделано:** что завершил со вчера
- **В работе / план:** над чем работает сегодня
- **Блокеры:** что мешает

## Общие блокеры и риски
## Решения и договорённости
## Action items

Требования:
- Максимальная краткость
- Сохраняй конкретику: имена, названия задач, сервисов

Транскрипция встречи "{meeting_name}":
{transcript}`,
    },
  };

  const t = templates[active];

  return (
    <>
      <h2>Шаблоны</h2>
      <p className="pane-sub">Каждый шаблон — это инструкция для модели. Можно редактировать prompt и добавлять свои.</p>

      <div style={{ display: 'flex', gap: 10, marginBottom: 16 }}>
        <button className="btn"><Icon name="plus" size={14} /> Новый шаблон</button>
        <button className="btn btn-ghost"><Icon name="folder" size={14} /> Открыть папку</button>
      </div>

      <div className="template-grid">
        {Object.entries(templates).map(([id, tpl]) => (
          <button
            key={id}
            className={`template-card ${active === id ? 'is-active' : ''}`}
            onClick={() => setActive(id)}
          >
            <div className="tpl-icn"><Icon name={tpl.icon} size={16} /></div>
            <div style={{ flex: 1, textAlign: 'left', minWidth: 0 }}>
              <div className="tpl-name">
                {tpl.name}
                {tpl.isDefault && <span className="tag" style={{ marginLeft: 8 }}>По умолчанию</span>}
              </div>
              <div className="tpl-desc">{tpl.desc}</div>
            </div>
            <div className="tpl-count">{tpl.meetings}</div>
          </button>
        ))}
      </div>

      <div className="spacer" />

      <div className="prompt-viewer">
        <div className="prompt-header">
          <div>
            <div className="prompt-title">{t.name}</div>
            <div className="prompt-sub">Этот текст отправляется в Claude вместе с транскрипцией</div>
          </div>
          <div style={{ display: 'flex', gap: 6 }}>
            <button className="btn btn-ghost"><Icon name="copy" size={14} /> Копировать</button>
            <button className="btn btn-ghost"><Icon name="edit" size={14} /> Редактировать</button>
          </div>
        </div>
        <pre className="prompt-code">{t.prompt}</pre>
        <div className="prompt-footer">
          <div className="chip"><Icon name="key" size={11} /> {'{meeting_name}'}</div>
          <div className="chip"><Icon name="key" size={11} /> {'{transcript}'}</div>
          <span style={{ color: 'var(--ink-4)', fontSize: 11, marginLeft: 'auto' }}>
            подставляются автоматически
          </span>
        </div>
      </div>
    </>
  );
}

function StoragePane() {
  return (
    <>
      <h2>Хранилище</h2>
      <p className="pane-sub">Где meety хранит базу, аудио и транскрипты.</p>

      <div className="row">
        <div>
          <div className="label">Папка встреч</div>
          <div className="help">Аудиозаписи и транскрипты.</div>
        </div>
        <div className="field-wrap">
          <div className="field">
            <input defaultValue="~/Documents/meety/recordings" />
            <button className="btn btn-ghost" style={{ padding: '2px 8px' }}><Icon name="folder" size={14} /></button>
          </div>
        </div>
      </div>

      <div className="row">
        <div>
          <div className="label">База данных</div>
          <div className="help">SQLite. Изменение пути требует перезапуска.</div>
        </div>
        <div className="field-wrap">
          <div className="field">
            <input defaultValue="~/Library/Application Support/meety/meetings.db" />
            <button className="btn btn-ghost" style={{ padding: '2px 8px' }}><Icon name="folder" size={14} /></button>
          </div>
        </div>
      </div>

      <div className="row">
        <div>
          <div className="label">Использование диска</div>
          <div className="help">125 встреч · 4.2 ГБ аудио · 18 МБ транскриптов</div>
        </div>
        <div className="field-wrap">
          <div style={{ height: 6, borderRadius: 3, background: 'var(--paper-3)', overflow: 'hidden', marginBottom: 8 }}>
            <div style={{ width: '34%', height: '100%', background: 'var(--accent)' }} />
          </div>
          <div className="help">4.2 ГБ из 12 ГБ, выделенных приложению</div>
          <div className="spacer-sm" />
          <button className="btn"><Icon name="trash" size={14} /> Удалить аудио старше 90 дней</button>
        </div>
      </div>
    </>
  );
}

function RecordingPane() {
  return (
    <>
      <h2>Запись</h2>
      <p className="pane-sub">Настройки по умолчанию для новых записей. Их можно переопределить перед каждой записью.</p>

      <div className="row">
        <div>
          <div className="label">Источник звука</div>
          <div className="help">Что записывать по умолчанию.</div>
        </div>
        <div className="field-wrap">
          <div className="segmented">
            <button>Микрофон</button>
            <button>Система</button>
            <button className="is-active">Оба</button>
          </div>
        </div>
      </div>

      <div className="row">
        <div>
          <div className="label">Микрофон</div>
        </div>
        <div className="field-wrap">
          <div className="field"><input defaultValue="MacBook Pro Microphone" /></div>
        </div>
      </div>

      <div className="row">
        <div>
          <div className="label">Подавление эха</div>
          <div className="help">По умолчанию выключено для системного звука.</div>
        </div>
        <div className="field-wrap" style={{ display: 'flex', justifyContent: 'flex-end' }}>
          <button className="switch" data-on={false} />
        </div>
      </div>
    </>
  );
}

Object.assign(window, { Settings });
