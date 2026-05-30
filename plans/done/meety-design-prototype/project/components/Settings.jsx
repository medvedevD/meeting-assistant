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

const BUILTIN_MODELS = [
  { id: 'tiny',                name: 'Tiny',               size: '75 MiB',  bytes: 75,    desc: 'Самая быстрая · подойдёт для черновиков и коротких заметок.',     installed: false },
  { id: 'base',                name: 'Base',               size: '142 MiB', bytes: 142,   desc: 'Лёгкая · точнее, чем Tiny, по-прежнему очень быстрая.',            installed: true  },
  { id: 'small',               name: 'Small',              size: '466 MiB', bytes: 466,   desc: 'Сбалансированная · ежедневные встречи без излишеств.',             installed: false },
  { id: 'medium',              name: 'Medium',             size: '1.5 GiB', bytes: 1500,  desc: 'Лучше справляется с шумным аудио и длинными встречами.',           installed: true  },
  { id: 'large-v3',            name: 'Large v3',           size: '2.9 GiB', bytes: 2900,  desc: 'Максимальное качество · медленная и требует много памяти.',         installed: false },
  { id: 'large-v3-turbo-q5',   name: 'Large v3 Turbo Q5',  size: '548 MiB', bytes: 548,   desc: 'Сжатая Turbo · быстрее Large при умеренном размере.',              installed: true  },
  { id: 'large-v3-turbo-q8',   name: 'Large v3 Turbo Q8',  size: '834 MiB', bytes: 834,   desc: 'Turbo с большей точностью · крупнее, чем Q5, но качественнее.',     installed: true  },
];

const MODELS_FOLDER = '/home/dmitry/.local/share/meeting-assistant/models';

function TranscriptionPane() {
  const [active, setActive] = React.useState('large-v3-turbo-q5');
  const [installed, setInstalled] = React.useState(
    () => Object.fromEntries(BUILTIN_MODELS.map(m => [m.id, m.installed]))
  );
  const [downloading, setDownloading] = React.useState({}); // id -> progress 0..1
  const [customs, setCustoms] = React.useState([
    // example seed so the section isn't empty in the mock
    // { id: 'custom-1', name: 'Whisper FT — Tech RU', path: '/Users/dmitry/Models/ggml-large-v3-ru-tech.bin', size: '2.1 GiB' },
  ]);
  const [addOpen, setAddOpen] = React.useState(false);

  // resolve currently-active model from the union of built-ins + customs
  const activeModel =
    BUILTIN_MODELS.find(m => m.id === active) ||
    customs.find(c => c.id === active) ||
    BUILTIN_MODELS[0];

  const activeIsCustom = !!customs.find(c => c.id === active);
  const activePath = activeIsCustom
    ? activeModel.path
    : `${MODELS_FOLDER}/ggml-${activeModel.id}.bin`;

  // simulate a download
  function startDownload(id) {
    setDownloading(d => ({ ...d, [id]: 0 }));
    const tick = setInterval(() => {
      setDownloading(d => {
        const p = (d[id] ?? 0) + 0.04 + Math.random() * 0.06;
        if (p >= 1) {
          clearInterval(tick);
          setInstalled(inst => ({ ...inst, [id]: true }));
          const { [id]: _, ...rest } = d;
          return rest;
        }
        return { ...d, [id]: p };
      });
    }, 250);
  }

  function addCustom(custom) {
    const id = `custom-${Date.now()}`;
    setCustoms(cs => [...cs, { ...custom, id }]);
    setAddOpen(false);
  }

  function removeCustom(id) {
    setCustoms(cs => cs.filter(c => c.id !== id));
    if (active === id) setActive('large-v3-turbo-q5');
  }

  return (
    <>
      <h2>Транскрипция</h2>
      <p className="pane-sub">Whisper работает локально на вашем устройстве. Аудио никогда не покидает компьютер.</p>

      {/* ─── Active model hero ──────────────────────────────────────────────── */}
      <div className="model-hero">
        <div className="model-hero-icn"><Icon name="waveform" size={20} /></div>
        <div className="model-hero-body">
          <div className="model-hero-label">Активная модель</div>
          <div className="model-hero-name">
            {activeModel.name}
            <span className="model-size-chip">{activeModel.size || '—'}</span>
          </div>
          <div className="model-hero-desc">
            {activeIsCustom
              ? 'Внешний ggml-совместимый файл. Будет использоваться для всех новых транскрипций.'
              : activeModel.desc}
          </div>
          <div className="model-hero-path" title={activePath}>
            <Icon name="folder" size={11} />
            <span className="path">{activePath}</span>
          </div>
        </div>
        <div className="model-hero-actions">
          <button className="btn btn-ghost btn-sm" title="Показать в Finder">
            <Icon name="folder" size={13} /> Показать
          </button>
        </div>
      </div>

      {/* ─── Catalog ────────────────────────────────────────────────────────── */}
      <div className="catalog-head">
        <div>
          <div className="catalog-title">Каталог моделей</div>
          <div className="help">Установленные модели берутся из этой папки. Добавляйте свои или скачивайте недостающие.</div>
        </div>
        <div className="catalog-toolbar">
          <button className="folder-chip" title={MODELS_FOLDER}>
            <Icon name="folder" size={12} />
            <span className="folder-chip-path">…/meeting-assistant/models</span>
            <Icon name="edit" size={11} />
          </button>
          <button className="btn btn-ghost btn-sm"><Icon name="refresh" size={12} /> Обновить</button>
        </div>
      </div>

      {/* Built-in models — Whisper.cpp ggml catalog, split by state */}
      {(() => {
        const installedList = BUILTIN_MODELS
          .filter(m => installed[m.id])
          .sort((a, b) => (b.id === active ? 1 : 0) - (a.id === active ? 1 : 0));
        const availableList = BUILTIN_MODELS.filter(m => !installed[m.id]);
        const renderRow = (m) => (
          <ModelRow
            key={m.id}
            model={m}
            isActive={m.id === active}
            isInstalled={installed[m.id]}
            downloading={downloading[m.id]}
            onSelect={() => installed[m.id] && setActive(m.id)}
            onDownload={() => startDownload(m.id)}
            onDelete={() => {
              setInstalled(inst => ({ ...inst, [m.id]: false }));
              if (active === m.id) setActive('large-v3-turbo-q5');
            }}
          />
        );
        return (
          <div className="model-list">
            {installedList.length > 0 && (
              <>
                <div className="model-sublabel">
                  <span>На устройстве</span>
                  <span className="model-sublabel-count">{installedList.length}</span>
                </div>
                {installedList.map(renderRow)}
              </>
            )}
            {availableList.length > 0 && (
              <>
                <div className={`model-sublabel ${installedList.length > 0 ? 'has-divider' : ''}`}>
                  <span>Доступно для загрузки</span>
                  <span className="model-sublabel-count">{availableList.length}</span>
                </div>
                {availableList.map(renderRow)}
              </>
            )}
          </div>
        );
      })()}

      {/* Custom / out-of-catalog models */}
      <div className="model-group-label" style={{ marginTop: 22 }}>
        <span>Модели вне каталога</span>
        <span className="model-group-meta">Внешний ggml-*.bin файл с диска</span>
      </div>
      <div className="model-list">
        {customs.length > 0 && customs.map(c => (
          <CustomModelRow
            key={c.id}
            model={c}
            isActive={c.id === active}
            onSelect={() => setActive(c.id)}
            onRemove={() => removeCustom(c.id)}
          />
        ))}

        {addOpen ? (
          <AddCustomForm onCancel={() => setAddOpen(false)} onAdd={addCustom} />
        ) : (
          <button className="model-add-row" onClick={() => setAddOpen(true)}>
            <Icon name="plus" size={14} />
            <span><b>Добавить файл модели</b> — ggml-совместимый .bin с диска</span>
          </button>
        )}
      </div>

      <div className="divider" style={{ marginTop: 28 }} />

      {/* Language */}
      <div className="row" style={{ borderBottom: 0 }}>
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

function ModelRow({ model, isActive, isInstalled, downloading, onSelect, onDownload, onDelete }) {
  const isDownloading = downloading !== undefined;
  return (
    <div
      className={`model-row ${isActive ? 'is-active' : ''} ${isInstalled ? 'is-installed' : 'is-uninstalled'}`}
      onClick={() => isInstalled && !isActive && onSelect()}
      role={isInstalled ? 'button' : undefined}
      tabIndex={isInstalled ? 0 : -1}
    >
      <div className="model-row-radio">
        {isActive
          ? <span className="model-radio-dot" />
          : isInstalled
            ? <span className="model-radio-ring" />
            : null
        }
      </div>
      <div className="model-row-body">
        <div className="model-row-name">
          {model.name}
          <span className="model-size-chip">{model.size}</span>
          {isActive && <span className="model-active-tag">Активна</span>}
        </div>
        <div className="model-row-desc">{model.desc}</div>
        {isDownloading && (
          <div className="model-row-progress">
            <div className="model-row-progress-bar"><div style={{ width: `${Math.round(downloading * 100)}%` }} /></div>
            <span className="model-row-progress-text">{Math.round(downloading * 100)}%</span>
          </div>
        )}
      </div>
      <div className="model-row-actions" onClick={e => e.stopPropagation()}>
        {isDownloading ? (
          <button className="btn btn-sm">Отмена</button>
        ) : isActive ? (
          null
        ) : isInstalled ? (
          <button className="btn btn-ghost btn-icon-sm" title="Удалить с диска" onClick={onDelete}>
            <Icon name="trash" size={13} />
          </button>
        ) : (
          <button className="btn btn-sm" onClick={onDownload}>
            <Icon name="download" size={12} /> Скачать
          </button>
        )}
      </div>
    </div>
  );
}

function CustomModelRow({ model, isActive, onSelect, onRemove }) {
  return (
    <div
      className={`model-row is-custom is-installed ${isActive ? 'is-active' : ''}`}
      onClick={() => !isActive && onSelect()}
      role="button"
      tabIndex={0}
    >
      <div className="model-row-radio">
        {isActive ? <span className="model-radio-dot" /> : <span className="model-radio-ring" />}
      </div>
      <div className="model-row-body">
        <div className="model-row-name">
          {model.name}
          {model.size && <span className="model-size-chip">{model.size}</span>}
          {isActive && <span className="model-active-tag">Активна</span>}
        </div>
        {model.desc && (
          <div className="model-row-desc">{model.desc}</div>
        )}
        <div className="model-row-path-mono" title={model.path}>
          {model.path}
        </div>
      </div>
      <div className="model-row-actions" onClick={e => e.stopPropagation()}>
        {isActive ? null : (
          <button className="btn btn-ghost btn-icon-sm" title="Убрать из списка" onClick={onRemove}>
            <Icon name="trash" size={13} />
          </button>
        )}
      </div>
    </div>
  );
}

function AddCustomForm({ onCancel, onAdd }) {
  const [name, setName] = React.useState('');
  const [path, setPath] = React.useState('');
  const [desc, setDesc] = React.useState('');
  const filename = path.split('/').pop() || '';
  const ok = path.endsWith('.bin');
  return (
    <div className="model-add-form">
      <div className="model-add-form-title">Добавить свой файл модели</div>
      <div className="model-add-form-desc">
        Whisper.cpp ggml-*.bin или совместимый. Файл остаётся на месте — meety только запоминает путь.
      </div>

      <div className="model-add-form-row">
        <div className="label">Файл</div>
        <div className="field">
          <input
            value={path}
            placeholder="/path/to/ggml-large-v3.bin"
            onChange={e => setPath(e.target.value)}
            style={{ fontFamily: 'var(--font-mono)', fontSize: 12.5 }}
          />
          <button className="btn btn-ghost btn-sm" style={{ padding: '2px 8px' }}>
            <Icon name="folder" size={12} /> Обзор
          </button>
        </div>
      </div>

      <div className="model-add-form-row">
        <div className="label">Название <span className="label-opt">(опц.)</span></div>
        <div className="field">
          <input
            value={name}
            placeholder={filename ? `«${filename.replace(/^ggml-/, '').replace(/\.bin$/, '')}»` : 'Whisper FT'}
            onChange={e => setName(e.target.value)}
          />
        </div>
      </div>

      <div className="model-add-form-row">
        <div className="label">Описание <span className="label-opt">(опц.)</span></div>
        <div className="field model-add-form-textarea">
          <textarea
            value={desc}
            placeholder="Например: дообученная под технические встречи на русском."
            onChange={e => setDesc(e.target.value)}
            rows={2}
          />
        </div>
      </div>

      <div className="model-add-form-actions">
        <button className="btn btn-ghost" onClick={onCancel}>Отмена</button>
        <button
          className="btn btn-primary"
          disabled={!ok}
          onClick={() => ok && onAdd({
            name: name.trim() || filename.replace(/^ggml-/, '').replace(/\.bin$/, '') || 'Своя модель',
            path,
            desc: desc.trim(),
            size: '',
          })}
        >
          Добавить
        </button>
      </div>
    </div>
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
