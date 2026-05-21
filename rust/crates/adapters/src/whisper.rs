use std::path::{Path, PathBuf};
use std::sync::Arc;
use async_trait::async_trait;
use meeting_core::{
    CoreError,
    entities::{PipelineStage, Segment, Transcript},
    ports::{ProgressSink, Transcriber},
};
use serde::{Deserialize, Serialize};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// A progress sink that discards reports — used by the non-progress path.
fn noop_sink() -> ProgressSink {
    Arc::new(|_stage: PipelineStage, _pct: u8| {})
}

// ── Transcriber preferences ───────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TranscriberPrefs {
    /// BCP-47 language code ("ru", "en") or "auto" for detection. Default: "ru".
    #[serde(default = "default_language")]
    pub language: String,
    /// Beam size for decoding. 1 = Greedy (fastest), 2–5 = BeamSearch (higher quality).
    #[serde(default = "default_beam_size")]
    pub beam_size: u32,
    /// Number of CPU threads for inference. 0 = auto (num_cpus::get_physical()).
    #[serde(default)]
    pub n_threads: u32,
}

fn default_language() -> String { "ru".to_string() }
fn default_beam_size() -> u32 { 1 }

impl TranscriberPrefs {
    pub fn new(language: impl Into<String>, beam_size: u32, n_threads: u32) -> Self {
        Self { language: language.into(), beam_size, n_threads }
    }
}

// ── Non-lazy transcriber (legacy, kept for tests) ─────────────────────────────

pub struct WhisperTranscriber {
    ctx: Arc<WhisperContext>,
}

impl WhisperTranscriber {
    pub fn new(model_path: &Path) -> anyhow::Result<Self> {
        let path = model_path.to_str()
            .ok_or_else(|| anyhow::anyhow!("model path is not valid UTF-8"))?;
        let t = std::time::Instant::now();
        let ctx = WhisperContext::new_with_params(path, WhisperContextParameters::default())
            .map_err(|e| anyhow::anyhow!("failed to load whisper model from {path}: {e}"))?;
        let model_name = model_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path);
        bench_log(&format!("model_loaded  model={model_name} load_ms={}", t.elapsed().as_millis()));
        Ok(Self { ctx: Arc::new(ctx) })
    }
}

#[async_trait]
impl Transcriber for WhisperTranscriber {
    async fn transcribe(&self, audio_path: &Path) -> Result<Transcript, CoreError> {
        let ctx = Arc::clone(&self.ctx);
        let path = audio_path.to_path_buf();
        let prefs = TranscriberPrefs::default();
        let sink = noop_sink();
        tokio::task::spawn_blocking(move || run_whisper(&ctx, &path, &prefs, &sink))
            .await
            .map_err(|e| CoreError::Transcription(e.to_string()))?
    }

    async fn transcribe_with_progress(
        &self,
        audio_path: &Path,
        on_progress: ProgressSink,
    ) -> Result<Transcript, CoreError> {
        let ctx = Arc::clone(&self.ctx);
        let path = audio_path.to_path_buf();
        let prefs = TranscriberPrefs::default();
        tokio::task::spawn_blocking(move || run_whisper(&ctx, &path, &prefs, &on_progress))
            .await
            .map_err(|e| CoreError::Transcription(e.to_string()))?
    }
}

fn run_whisper(
    ctx: &WhisperContext,
    audio_path: &PathBuf,
    prefs: &TranscriberPrefs,
    on_progress: &ProgressSink,
) -> Result<Transcript, CoreError> {
    let t_total = std::time::Instant::now();

    on_progress(PipelineStage::DecodingAudio, 0);
    let t = std::time::Instant::now();
    let samples = load_wav_as_mono_f32(audio_path)
        .map_err(|e| CoreError::Transcription(e.to_string()))?;
    let audio_duration_s = samples.len() as f64 / WHISPER_SAMPLE_RATE as f64;
    bench_log(&format!("audio_loaded  duration_s={audio_duration_s:.1} load_ms={}", t.elapsed().as_millis()));

    let mut state = ctx.create_state()
        .map_err(|e| CoreError::Transcription(e.to_string()))?;

    let strategy = if prefs.beam_size <= 1 {
        SamplingStrategy::Greedy { best_of: 1 }
    } else {
        SamplingStrategy::BeamSearch { beam_size: prefs.beam_size as i32, patience: -1.0 }
    };
    let mut params = FullParams::new(strategy);

    let lang = if prefs.language == "auto" { None } else { Some(prefs.language.as_str()) };
    params.set_language(lang);
    let threads = if prefs.n_threads == 0 {
        num_cpus::get_physical() as i32
    } else {
        prefs.n_threads as i32
    };
    params.set_n_threads(threads);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    // Stream inference progress (0–100) into the live-progress sink, inside the
    // Transcribing stage. The callback runs on the inference thread.
    on_progress(PipelineStage::Transcribing, 0);
    {
        let sink = Arc::clone(on_progress);
        params.set_progress_callback_safe(move |p: i32| {
            sink(PipelineStage::Transcribing, p.clamp(0, 100) as u8);
        });
    }

    let t = std::time::Instant::now();
    state.full(params, &samples)
        .map_err(|e| CoreError::Transcription(e.to_string()))?;
    let infer_ms = t.elapsed().as_millis();
    let rtf = infer_ms as f64 / 1000.0 / audio_duration_s;
    bench_log(&format!("inference_done  infer_ms={infer_ms} rtf={rtf:.2}"));

    let mut segments = Vec::new();
    let mut full_text = String::new();

    for seg in state.as_iter() {
        let text = seg.to_str_lossy()
            .map_err(|e| CoreError::Transcription(e.to_string()))?;
        // whisper timestamps are in centiseconds → convert to ms (clamp negatives to 0)
        let start_ms = seg.start_timestamp().max(0) as u64 * 10;
        let end_ms = seg.end_timestamp().max(0) as u64 * 10;

        full_text.push_str(&text);
        segments.push(Segment { start_ms, end_ms, text: text.trim().to_string() });
    }

    let detected_lang = prefs.language.clone();
    bench_log(&format!("transcription_complete  total_ms={}", t_total.elapsed().as_millis()));

    Ok(Transcript {
        text: full_text.trim().to_string(),
        segments,
        language: detected_lang,
    })
}

const WHISPER_SAMPLE_RATE: u32 = 16000;
const IDLE_UNLOAD_SECS: u64 = 300; // 5 minutes

fn load_wav_as_mono_f32(path: &Path) -> anyhow::Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();

    let samples_f32: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => {
            reader.samples::<f32>().collect::<Result<_, _>>()?
        }
        hound::SampleFormat::Int => {
            let samples_i16: Vec<i16> = reader.samples::<i16>().collect::<Result<_, _>>()?;
            let mut out = vec![0f32; samples_i16.len()];
            whisper_rs::convert_integer_to_float_audio(&samples_i16, &mut out)
                .map_err(|e| anyhow::anyhow!("audio conversion failed: {e}"))?;
            out
        }
    };

    let mono = if spec.channels == 1 {
        samples_f32
    } else {
        // whisper-rs 0.16: writes into a caller-provided buffer (len = input/2)
        let mut out = vec![0f32; samples_f32.len() / 2];
        whisper_rs::convert_stereo_to_mono_audio(&samples_f32, &mut out)
            .map_err(|e| anyhow::anyhow!("stereo→mono failed: {e}"))?;
        out
    };

    if spec.sample_rate == WHISPER_SAMPLE_RATE {
        return Ok(mono);
    }

    tracing::info!(
        from = spec.sample_rate,
        to = WHISPER_SAMPLE_RATE,
        "resampling audio"
    );
    resample(mono, spec.sample_rate, WHISPER_SAMPLE_RATE)
}

fn bench_log(msg: &str) {
    use std::io::Write;
    let base = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let path = std::path::Path::new(&base)
        .join(".local/share/meeting-assistant/transcription.log");
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "[{ts}] {msg}");
    }
}

// ── LazyWhisperTranscriber ───────────────────────────────────────────────────

#[async_trait]
pub(crate) trait WhisperRunner: Send + Sync {
    async fn run(
        &self,
        audio_path: &Path,
        prefs: &TranscriberPrefs,
        on_progress: ProgressSink,
    ) -> Result<Transcript, CoreError>;
}

pub(crate) type RunnerFactory =
    Arc<dyn Fn(&Path) -> Result<Arc<dyn WhisperRunner>, CoreError> + Send + Sync>;

pub(crate) struct LazyState {
    pub(crate) runner: Option<Arc<dyn WhisperRunner>>,
    pub(crate) active_count: usize,
    pub(crate) unload_handle: Option<tokio::task::AbortHandle>,
}

impl Default for LazyState {
    fn default() -> Self {
        Self { runner: None, active_count: 0, unload_handle: None }
    }
}

pub struct LazyWhisperTranscriber {
    model_path: std::sync::RwLock<PathBuf>,
    factory: RunnerFactory,
    pub(crate) state: Arc<tokio::sync::Mutex<LazyState>>,
    prefs: std::sync::RwLock<TranscriberPrefs>,
}

struct RealWhisperRunner {
    ctx: Arc<WhisperContext>,
}

#[async_trait]
impl WhisperRunner for RealWhisperRunner {
    async fn run(
        &self,
        audio_path: &Path,
        prefs: &TranscriberPrefs,
        on_progress: ProgressSink,
    ) -> Result<Transcript, CoreError> {
        let ctx = Arc::clone(&self.ctx);
        let path = audio_path.to_path_buf();
        let prefs = prefs.clone();
        tokio::task::spawn_blocking(move || run_whisper(&ctx, &path, &prefs, &on_progress))
            .await
            .map_err(|e| CoreError::Transcription(e.to_string()))?
    }
}

impl LazyWhisperTranscriber {
    /// Creates a production instance with given transcriber preferences.
    /// Does NOT load the model — loading is deferred to first transcription.
    pub fn new(model_path: PathBuf, prefs: TranscriberPrefs) -> Self {
        let factory: RunnerFactory = Arc::new(|path: &Path| {
            let path_str = path.to_str()
                .ok_or_else(|| CoreError::Transcription("model path is not valid UTF-8".into()))?;
            let t = std::time::Instant::now();
            let ctx = WhisperContext::new_with_params(path_str, WhisperContextParameters::default())
                .map_err(|e| CoreError::Transcription(format!("Модель не найдена или повреждена: {e}")))?;
            let model_name = path.file_name().and_then(|n| n.to_str()).unwrap_or(path_str);
            bench_log(&format!("model_loaded  model={model_name} load_ms={}", t.elapsed().as_millis()));
            Ok(Arc::new(RealWhisperRunner { ctx: Arc::new(ctx) }) as Arc<dyn WhisperRunner>)
        });
        Self::new_with_factory(model_path, factory, prefs)
    }

    pub(crate) fn new_with_factory(model_path: PathBuf, factory: RunnerFactory, prefs: TranscriberPrefs) -> Self {
        Self {
            model_path: std::sync::RwLock::new(model_path),
            factory,
            state: Arc::new(tokio::sync::Mutex::new(LazyState::default())),
            prefs: std::sync::RwLock::new(prefs),
        }
    }

    /// Preloads the model in the background without running a transcription.
    /// Safe to call multiple times — a no-op if the model is already loaded.
    pub async fn ensure_loaded(&self) -> Result<(), CoreError> {
        let model_path = self.model_path.read().unwrap().clone();
        let mut state = self.state.lock().await;
        if state.runner.is_none() {
            tracing::info!("warm preloading whisper model: {:?}", model_path);
            state.runner = Some((self.factory)(&model_path)?);
        }
        Ok(())
    }

    pub fn current_model_path(&self) -> PathBuf {
        self.model_path.read().unwrap().clone()
    }

    /// Update transcriber preferences at runtime (e.g. after user saves settings).
    pub fn set_prefs(&self, prefs: TranscriberPrefs) {
        *self.prefs.write().unwrap() = prefs;
    }

    /// Change the model path at runtime. Immediately unloads the current model from memory;
    /// the new model will be loaded on the next transcription request.
    pub async fn set_model_path(&self, new_path: PathBuf) {
        *self.model_path.write().unwrap() = new_path;
        let mut state = self.state.lock().await;
        state.runner = None;
        if let Some(handle) = state.unload_handle.take() {
            handle.abort();
        }
        tracing::info!("whisper model path updated, old model unloaded");
    }

    #[cfg(test)]
    async fn ctx_is_loaded(&self) -> bool {
        self.state.lock().await.runner.is_some()
    }
}

impl LazyWhisperTranscriber {
    /// Shared acquire → run → release path used by both `transcribe` and
    /// `transcribe_with_progress`. The `on_progress` sink is forwarded to the
    /// runner (a no-op sink for the plain path).
    async fn transcribe_inner(
        &self,
        audio_path: &Path,
        on_progress: ProgressSink,
    ) -> Result<Transcript, CoreError> {
        let prefs = self.prefs.read().unwrap().clone();
        let model_path = self.model_path.read().unwrap().clone();

        // --- 1. ACQUIRE ---
        let runner: Arc<dyn WhisperRunner> = {
            let mut state = self.state.lock().await;
            if state.runner.is_none() {
                tracing::info!("loading whisper model: {:?}", model_path);
                // Early return on error — active_count and unload_handle are untouched
                state.runner = Some((self.factory)(&model_path)?);
            }
            if let Some(handle) = state.unload_handle.take() {
                handle.abort();
            }
            state.active_count += 1;
            Arc::clone(state.runner.as_ref().unwrap())
        }; // ← lock released, runner lives as Arc

        // --- 2. TRANSCRIBE (без lock) ---
        let result = runner.run(audio_path, &prefs, on_progress).await;

        // --- 3. RELEASE ---
        {
            let mut state = self.state.lock().await;
            state.active_count -= 1;
            if state.active_count == 0 {
                let state_clone = Arc::clone(&self.state);
                let task = tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(IDLE_UNLOAD_SECS)).await;
                    state_clone.lock().await.runner = None;
                    tracing::info!("whisper model unloaded after {}s idle", IDLE_UNLOAD_SECS);
                });
                state.unload_handle = Some(task.abort_handle());
            }
        }

        result
    }
}

#[async_trait]
impl Transcriber for LazyWhisperTranscriber {
    async fn transcribe(&self, audio_path: &Path) -> Result<Transcript, CoreError> {
        self.transcribe_inner(audio_path, noop_sink()).await
    }

    async fn transcribe_with_progress(
        &self,
        audio_path: &Path,
        on_progress: ProgressSink,
    ) -> Result<Transcript, CoreError> {
        self.transcribe_inner(audio_path, on_progress).await
    }
}

impl Drop for LazyWhisperTranscriber {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.try_lock() {
            if let Some(handle) = state.unload_handle.take() {
                handle.abort();
            }
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    // --- Fake infrastructure ---

    struct FakeRunner {
        fail_run: bool,
    }

    #[async_trait]
    impl WhisperRunner for FakeRunner {
        async fn run(
            &self,
            _audio_path: &Path,
            _prefs: &TranscriberPrefs,
            _on_progress: ProgressSink,
        ) -> Result<Transcript, CoreError> {
            if self.fail_run {
                return Err(CoreError::Transcription("fake run error".into()));
            }
            Ok(Transcript { text: "ok".into(), segments: vec![], language: "en".into() })
        }
    }

    fn factory_ok(load_count: Arc<AtomicUsize>) -> RunnerFactory {
        Arc::new(move |_path| {
            load_count.fetch_add(1, Ordering::SeqCst);
            Ok(Arc::new(FakeRunner { fail_run: false }) as Arc<dyn WhisperRunner>)
        })
    }

    fn factory_fail_load(load_count: Arc<AtomicUsize>) -> RunnerFactory {
        Arc::new(move |_path| {
            load_count.fetch_add(1, Ordering::SeqCst);
            Err(CoreError::Transcription("model not found".into()))
        })
    }

    fn factory_fail_run(load_count: Arc<AtomicUsize>) -> RunnerFactory {
        Arc::new(move |_path| {
            load_count.fetch_add(1, Ordering::SeqCst);
            Ok(Arc::new(FakeRunner { fail_run: true }) as Arc<dyn WhisperRunner>)
        })
    }

    fn factory_conditional(load_count: Arc<AtomicUsize>, should_fail: Arc<AtomicBool>) -> RunnerFactory {
        Arc::new(move |_path| {
            load_count.fetch_add(1, Ordering::SeqCst);
            if should_fail.load(Ordering::SeqCst) {
                return Err(CoreError::Transcription("model not found".into()));
            }
            Ok(Arc::new(FakeRunner { fail_run: false }) as Arc<dyn WhisperRunner>)
        })
    }

    fn audio() -> &'static Path {
        Path::new("/fake/audio.wav")
    }

    fn default_prefs() -> TranscriberPrefs { TranscriberPrefs::default() }

    // TC-001: первый вызов загружает модель
    #[tokio::test]
    async fn tc001_first_call_loads_model() {
        let load_count = Arc::new(AtomicUsize::new(0));
        let t = LazyWhisperTranscriber::new_with_factory(
            PathBuf::from("/model.bin"),
            factory_ok(load_count.clone()),
            default_prefs(),
        );

        t.transcribe(audio()).await.unwrap();

        assert_eq!(load_count.load(Ordering::SeqCst), 1);
    }

    // TC-002: параллельные вызовы не загружают модель повторно
    #[tokio::test]
    async fn tc002_concurrent_calls_load_once() {
        let load_count = Arc::new(AtomicUsize::new(0));
        let t = Arc::new(LazyWhisperTranscriber::new_with_factory(
            PathBuf::from("/model.bin"),
            factory_ok(load_count.clone()),
            default_prefs(),
        ));

        let t1 = Arc::clone(&t);
        let t2 = Arc::clone(&t);
        let (r1, r2) = tokio::join!(t1.transcribe(audio()), t2.transcribe(audio()));
        r1.unwrap();
        r2.unwrap();

        assert_eq!(load_count.load(Ordering::SeqCst), 1);
    }

    // TC-003: несуществующий путь → Err(CoreError), не паника
    #[tokio::test]
    async fn tc003_failed_load_returns_core_error_not_panic() {
        let t = LazyWhisperTranscriber::new_with_factory(
            PathBuf::from("/nonexistent.bin"),
            factory_fail_load(Arc::new(AtomicUsize::new(0))),
            default_prefs(),
        );

        let result = t.transcribe(audio()).await;

        assert!(matches!(result, Err(CoreError::Transcription(_))));
    }

    // TC-004: после IDLE_UNLOAD_SECS простоя ctx выгружается
    #[tokio::test(start_paused = true)]
    async fn tc004_unloads_after_idle() {
        let t = LazyWhisperTranscriber::new_with_factory(
            PathBuf::from("/model.bin"),
            factory_ok(Arc::new(AtomicUsize::new(0))),
            default_prefs(),
        );

        t.transcribe(audio()).await.unwrap();
        assert!(t.ctx_is_loaded().await);

        // Yield once so the spawned unload task is polled and registers its sleep at T=0.
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(IDLE_UNLOAD_SECS + 1)).await;
        tokio::task::yield_now().await;

        assert!(!t.ctx_is_loaded().await);
    }

    // TC-005: вызов до истечения TTL сбрасывает таймер, ctx остаётся
    #[tokio::test(start_paused = true)]
    async fn tc005_call_before_idle_resets_timer() {
        let t = LazyWhisperTranscriber::new_with_factory(
            PathBuf::from("/model.bin"),
            factory_ok(Arc::new(AtomicUsize::new(0))),
            default_prefs(),
        );

        t.transcribe(audio()).await.unwrap();
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(100)).await;
        tokio::task::yield_now().await;

        // Второй вызов сбрасывает таймер
        t.transcribe(audio()).await.unwrap();
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(100)).await;
        tokio::task::yield_now().await;

        assert!(t.ctx_is_loaded().await, "ctx должен быть загружен через 100с после последнего вызова");

        tokio::time::advance(Duration::from_secs(IDLE_UNLOAD_SECS + 1)).await;
        tokio::task::yield_now().await;

        assert!(!t.ctx_is_loaded().await, "ctx должен выгрузиться после TTL");
    }

    // TC-006: ошибка транскрипции не ломает active_count
    #[tokio::test]
    async fn tc006_transcription_error_releases_active_count() {
        let t = LazyWhisperTranscriber::new_with_factory(
            PathBuf::from("/model.bin"),
            factory_fail_run(Arc::new(AtomicUsize::new(0))),
            default_prefs(),
        );

        let result = t.transcribe(audio()).await;
        assert!(result.is_err());

        let state = t.state.lock().await;
        assert_eq!(state.active_count, 0, "active_count должен быть 0 после ошибки");
        assert!(state.unload_handle.is_some(), "таймер выгрузки должен быть запущен");
    }

    // TC-007: повторный вызов после failed load пробует загрузить снова
    #[tokio::test]
    async fn tc007_retries_load_after_failure() {
        let load_count = Arc::new(AtomicUsize::new(0));
        let should_fail = Arc::new(AtomicBool::new(true));
        let t = LazyWhisperTranscriber::new_with_factory(
            PathBuf::from("/model.bin"),
            factory_conditional(load_count.clone(), should_fail.clone()),
            default_prefs(),
        );

        assert!(t.transcribe(audio()).await.is_err());
        assert_eq!(load_count.load(Ordering::SeqCst), 1);
        assert!(!t.ctx_is_loaded().await, "ctx должен остаться None после ошибки загрузки");

        should_fail.store(false, Ordering::SeqCst);
        assert!(t.transcribe(audio()).await.is_ok());
        assert_eq!(load_count.load(Ordering::SeqCst), 2, "должна быть повторная попытка загрузки");
    }

    // TC-008: N параллельных завершений → один unload_handle
    #[tokio::test]
    async fn tc008_parallel_completions_create_one_timer() {
        let t = Arc::new(LazyWhisperTranscriber::new_with_factory(
            PathBuf::from("/model.bin"),
            factory_ok(Arc::new(AtomicUsize::new(0))),
            default_prefs(),
        ));

        let handles: Vec<_> = (0..3)
            .map(|_| {
                let t = Arc::clone(&t);
                tokio::spawn(async move { t.transcribe(audio()).await.unwrap() })
            })
            .collect();
        for h in handles { h.await.unwrap(); }

        let state = t.state.lock().await;
        assert_eq!(state.active_count, 0);
        assert!(state.unload_handle.is_some(), "должен быть ровно один активный таймер");
    }

    // TC-009: Drop при pending-таймере отменяет таймер
    #[tokio::test(start_paused = true)]
    async fn tc009_drop_aborts_unload_timer() {
        let t = LazyWhisperTranscriber::new_with_factory(
            PathBuf::from("/model.bin"),
            factory_ok(Arc::new(AtomicUsize::new(0))),
            default_prefs(),
        );

        t.transcribe(audio()).await.unwrap();
        tokio::task::yield_now().await;
        drop(t);

        tokio::time::advance(Duration::from_secs(IDLE_UNLOAD_SECS + 1)).await;
        tokio::task::yield_now().await;
    }

    // TC-010: new() не делает IO
    #[test]
    fn tc010_new_does_not_do_io() {
        let _t = LazyWhisperTranscriber::new(
            PathBuf::from("/totally/fake/path/model.bin"),
            TranscriberPrefs::default(),
        );
    }

    // TC-011: ensure_loaded загружает модель без транскрипции
    #[tokio::test]
    async fn tc011_ensure_loaded_preloads_model() {
        let load_count = Arc::new(AtomicUsize::new(0));
        let t = LazyWhisperTranscriber::new_with_factory(
            PathBuf::from("/model.bin"),
            factory_ok(load_count.clone()),
            default_prefs(),
        );

        assert!(!t.ctx_is_loaded().await);
        t.ensure_loaded().await.unwrap();
        assert!(t.ctx_is_loaded().await);
        assert_eq!(load_count.load(Ordering::SeqCst), 1);

        // Повторный вызов — no-op
        t.ensure_loaded().await.unwrap();
        assert_eq!(load_count.load(Ordering::SeqCst), 1);
    }

    // TC-012: set_model_path выгружает runner и обновляет путь
    #[tokio::test]
    async fn tc012_set_model_path_unloads_runner() {
        let load_count = Arc::new(AtomicUsize::new(0));
        let t = LazyWhisperTranscriber::new_with_factory(
            PathBuf::from("/model-a.bin"),
            factory_ok(load_count.clone()),
            default_prefs(),
        );

        // Загрузить модель A
        t.transcribe(audio()).await.unwrap();
        assert!(t.ctx_is_loaded().await);
        assert_eq!(load_count.load(Ordering::SeqCst), 1);

        // Сменить путь → runner должен выгрузиться
        t.set_model_path(PathBuf::from("/model-b.bin")).await;
        assert!(!t.ctx_is_loaded().await, "runner должен быть выгружен после set_model_path");

        // Следующая транскрипция загружает снова (по новому пути)
        t.transcribe(audio()).await.unwrap();
        assert_eq!(load_count.load(Ordering::SeqCst), 2, "должна быть загрузка новой модели");
    }
}

fn resample(samples: Vec<f32>, from_rate: u32, to_rate: u32) -> anyhow::Result<Vec<f32>> {
    use rubato::{FftFixedIn, Resampler};

    const CHUNK: usize = 1024;

    let mut resampler = FftFixedIn::<f32>::new(
        from_rate as usize,
        to_rate as usize,
        CHUNK,
        2,
        1,
    )?;

    let ratio = to_rate as f64 / from_rate as f64;
    let mut output = Vec::with_capacity((samples.len() as f64 * ratio) as usize + CHUNK);
    let mut pos = 0;

    while pos + CHUNK <= samples.len() {
        let waves_in = vec![samples[pos..pos + CHUNK].to_vec()];
        let waves_out = resampler.process(&waves_in, None)?;
        output.extend_from_slice(&waves_out[0]);
        pos += CHUNK;
    }

    if pos < samples.len() {
        let mut tail = samples[pos..].to_vec();
        tail.resize(CHUNK, 0.0);
        let waves_in = vec![tail];
        let waves_out = resampler.process_partial(Some(&waves_in), None)?;
        output.extend_from_slice(&waves_out[0]);
    }

    Ok(output)
}
