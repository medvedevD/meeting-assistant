// Meety — main app. State machine: which screen is showing.

const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "theme": "light",
  "palette": "sienna",
  "density": "regular",
  "sidebar": "full",
  "protocolRender": "editorial",
  "recordingHero": "orb",
  "screen": "detail"
}/*EDITMODE-END*/;

function App() {
  const [t, setTweak] = useTweaks(TWEAK_DEFAULTS);

  // app state
  const [screen, setScreen] = React.useState(t.screen || 'detail');
  // keep in sync if tweak panel changes 'screen'
  React.useEffect(() => { if (t.screen && t.screen !== screen) setScreen(t.screen); }, [t.screen]);

  // expose for debug + screenshots: window.__meety.setScreen(name), setTweak({...})
  React.useEffect(() => {
    window.__meety = {
      setScreen: (s) => { setScreen(s); setTweak('screen', s); },
      setTweak: (k, v) => setTweak(k, v),
      getState: () => ({ screen, ...t }),
    };
  }, [screen, t]);

  const [selectedId, setSelectedId] = React.useState('m1');
  const [query, setQuery] = React.useState('');

  // recording state
  const [recState, setRecState] = React.useState({ kind: 'idle' });
  const [paletteOpen, setPaletteOpen] = React.useState(false);

  // ⌘K / ctrl+K — open palette
  React.useEffect(() => {
    const onKey = (e) => {
      const k = e.key.toLowerCase();
      if ((e.metaKey || e.ctrlKey) && k === 'k') {
        e.preventDefault();
        setPaletteOpen(o => !o);
      } else if ((e.metaKey || e.ctrlKey) && k === 'n') {
        e.preventDefault();
        handleShowNew();
      } else if ((e.metaKey || e.ctrlKey) && k === ',') {
        e.preventDefault();
        handleShowSettings();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const selected = MEETINGS.find(m => m.id === selectedId);

  // simulate recording timer
  React.useEffect(() => {
    if (recState.kind !== 'recording') return;
    const id = setInterval(() => {
      setRecState(s => s.kind === 'recording' ? { ...s, elapsed: s.elapsed + 1 } : s);
    }, 1000);
    return () => clearInterval(id);
  }, [recState.kind]);

  // also tick faster for visual variants (waveform, transcript anim)
  const [vt, setVt] = React.useState(0);
  React.useEffect(() => {
    if (recState.kind !== 'recording') return;
    const id = setInterval(() => setVt(v => v + 1), 100);
    return () => clearInterval(id);
  }, [recState.kind]);

  const handleSelect = (id) => {
    setSelectedId(id);
    setScreen('detail');
    setTweak('screen', 'detail');
  };
  const handleShowNew = () => { setScreen('newrec'); setTweak('screen', 'newrec'); };
  const handleShowSettings = () => { setScreen('settings'); setTweak('screen', 'settings'); };
  const handleShowWelcome = () => { setScreen('welcome'); setTweak('screen', 'welcome'); };
  const handleShowEmpty = () => { setScreen('empty'); setTweak('screen', 'empty'); };
  const handleShowGenerate = () => { setScreen('generate'); setTweak('screen', 'generate'); };
  const handleShowDetail = () => { setScreen('detail'); setTweak('screen', 'detail'); };

  const handleStartRecording = (name) => {
    setRecState({ kind: 'recording', name: name || 'Без названия', source: 'mixed', elapsed: 0, variant: t.recordingHero });
  };
  const handleStopRecording = () => {
    setRecState({ kind: 'idle' });
    setScreen('generate');
    setTweak('screen', 'generate');
  };

  // when in newrec screen and recording started, hero variant follows tweak
  // (vt is passed as a separate prop, so no need to mirror it into recState).

  const meetings = React.useMemo(() => {
    // when recording, prepend a recording row
    if (recState.kind === 'recording') {
      return [
        { id: 'rec', name: recState.name, date: new Date(), recording: true, elapsed: recState.elapsed },
        ...MEETINGS,
      ];
    }
    return MEETINGS;
  }, [recState]);

  // attribute hooks
  React.useEffect(() => {
    document.documentElement.setAttribute('data-theme', t.theme);
    document.documentElement.setAttribute('data-palette', t.palette);
    document.documentElement.setAttribute('data-density', t.density);
  }, [t.theme, t.palette, t.density]);

  // body content per screen
  const titlebarTitle =
    screen === 'detail'  ? (selected?.name || 'meety')
    : screen === 'newrec'  ? 'Новая запись'
    : screen === 'generate' ? 'Генерация протокола'
    : screen === 'settings' ? 'Настройки'
    : 'meety';

  return (
    <div className="viewport">
      <div className="window" data-sidebar={t.sidebar}>
        <div className="titlebar">
          <div className="traffic"><i /><i /><i /></div>
          <div className="titlebar-title">{titlebarTitle}</div>
        </div>

        <Sidebar
          meetings={meetings}
          selectedId={selectedId}
          onSelect={handleSelect}
          onNew={handleShowNew}
          onShowSettings={handleShowSettings}
          onOpenPalette={() => setPaletteOpen(true)}
          sidebarStyle={t.sidebar}
          density={t.density}
          query={query}
          setQuery={setQuery}
        />

        <div className="content">
          {screen === 'welcome' && (
            <Welcome onStart={handleShowNew} onImport={handleShowNew} />
          )}
          {screen === 'empty' && (
            <Welcome compact onStart={handleShowNew} onImport={handleShowNew} />
          )}
          {screen === 'detail' && selected && (
            <MeetingDetail
              meeting={selected}
              render={t.protocolRender}
              onBack={handleShowEmpty}
              onRegenerate={handleShowGenerate}
            />
          )}
          {screen === 'newrec' && (
            <NewRecording
              state={recState.kind === 'recording'
                ? { ...recState, variant: t.recordingHero, vt }
                : { kind: 'idle' }}
              onBack={handleShowEmpty}
              onStart={handleStartRecording}
              onStop={handleStopRecording}
              onImport={() => alert('Открыть файловый диалог…')}
              settings={{ source: 'mixed', echoCancel: false }}
            />
          )}
          {screen === 'generate' && (
            <GenerateProtocol
              meeting={selected || MEETINGS[0]}
              onBack={handleShowDetail}
              onDone={() => { /* host swap handled by clicking Continue */ }}
            />
          )}
          {screen === 'settings' && (
            <Settings onBack={handleShowEmpty} />
          )}
        </div>

        <CommandPalette
          open={paletteOpen}
          meetings={MEETINGS}
          onClose={() => setPaletteOpen(false)}
          onSelect={(id) => { setSelectedId(id); setScreen('detail'); setTweak('screen', 'detail'); }}
          onNavigate={(s) => { setScreen(s); setTweak('screen', s); }}
        />
      </div>

      {/* Tweaks */}
      <TweaksPanel>
        <TweakSection label="Screen" />
        <TweakSelect
          label="Show"
          value={t.screen}
          options={[
            { value: 'welcome',  label: 'Первый запуск' },
            { value: 'empty',    label: 'Выберите встречу' },
            { value: 'detail',   label: 'Детали встречи' },
            { value: 'newrec',   label: 'Новая запись' },
            { value: 'generate', label: 'Генерация протокола' },
            { value: 'settings', label: 'Настройки' },
          ]}
          onChange={(v) => setTweak('screen', v)}
        />

        <TweakSection label="Style" />
        <TweakRadio
          label="Theme"
          value={t.theme}
          options={['light', 'dark']}
          onChange={(v) => setTweak('theme', v)}
        />
        <TweakSelect
          label="Palette"
          value={t.palette}
          options={[
            { value: 'sienna', label: 'Sienna (warm paper)' },
            { value: 'slate',  label: 'Slate (cool blue)' },
            { value: 'moss',   label: 'Moss (deep green)' },
          ]}
          onChange={(v) => setTweak('palette', v)}
        />
        <TweakRadio
          label="Density"
          value={t.density}
          options={['compact', 'regular', 'spacious']}
          onChange={(v) => setTweak('density', v)}
        />
        <TweakRadio
          label="Sidebar"
          value={t.sidebar}
          options={['full', 'compact']}
          onChange={(v) => setTweak('sidebar', v)}
        />

        <TweakSection label="Protocol render" />
        <TweakSelect
          label="Layout"
          value={t.protocolRender}
          options={[
            { value: 'editorial', label: 'Editorial · длинный сериф' },
            { value: 'cards',     label: 'Cards · структурированные блоки' },
            { value: 'split',     label: 'Split · протокол + транскрипт' },
            { value: 'edit',      label: 'Edit · режим редактирования' },
          ]}
          onChange={(v) => setTweak('protocolRender', v)}
        />

        <TweakSection label="Recording hero" />
        <TweakSelect
          label="Variant"
          value={t.recordingHero}
          options={[
            { value: 'orb',   label: 'Orb · стеклянная сфера' },
            { value: 'wave',  label: 'Wave · волновая форма' },
            { value: 'trans', label: 'Live · стрим транскрипта' },
          ]}
          onChange={(v) => setTweak('recordingHero', v)}
        />
        <TweakButton
          label="Запустить запись"
          onClick={() => {
            setScreen('newrec');
            setTweak('screen', 'newrec');
            handleStartRecording('Демо-запись');
          }}
        />
      </TweaksPanel>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App />);
