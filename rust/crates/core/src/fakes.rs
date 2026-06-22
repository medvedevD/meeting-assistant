use crate::{
    entities::{Job, JobKind, JobStatus, Meeting, Segment, Transcript},
    ports::{
        AudioCapture, AudioDevice, AudioDeviceEnumerator, AudioDeviceList, AudioLevel,
        AudioLevelMonitor, CaptureSource, CaptureSpec, JobRepo, LlmProvider, MeetingFileStore,
        MeetingRepo, ResolvedDevices, TemplateBundle, TemplateLoader, Transcriber,
    },
    CoreError,
};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

// ── FakeTranscriber ──────────────────────────────────────────────────────────

pub struct FakeTranscriber {
    text: String,
}

impl FakeTranscriber {
    pub fn new(text: impl Into<String>) -> Arc<Self> {
        Arc::new(Self { text: text.into() })
    }
}

#[async_trait]
impl Transcriber for FakeTranscriber {
    async fn transcribe(&self, _audio_path: &Path) -> Result<Transcript, CoreError> {
        Ok(Transcript {
            text: self.text.clone(),
            segments: vec![Segment {
                start_ms: 0,
                end_ms: 1000,
                text: self.text.clone(),
            }],
            language: "ru".to_string(),
        })
    }
}

// ── FakeMeetingRepo ──────────────────────────────────────────────────────────

#[derive(Default)]
pub struct FakeMeetingRepo {
    store: Mutex<Vec<Meeting>>,
    fail_transcript_saves: AtomicBool,
    fail_protocol_saves: AtomicBool,
}

impl FakeMeetingRepo {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn set_fail_transcript_saves(&self, fail: bool) {
        self.fail_transcript_saves.store(fail, Ordering::SeqCst);
    }

    pub fn set_fail_protocol_saves(&self, fail: bool) {
        self.fail_protocol_saves.store(fail, Ordering::SeqCst);
    }
}

#[async_trait]
impl MeetingRepo for FakeMeetingRepo {
    async fn save(&self, meeting: &Meeting) -> Result<(), CoreError> {
        self.store.lock().unwrap().push(meeting.clone());
        Ok(())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<Meeting>, CoreError> {
        Ok(self
            .store
            .lock()
            .unwrap()
            .iter()
            .find(|m| m.id == id)
            .cloned())
    }

    async fn find_by_audio_path(&self, path: &Path) -> Result<Option<Meeting>, CoreError> {
        Ok(self
            .store
            .lock()
            .unwrap()
            .iter()
            .rfind(|m| m.audio_path == *path)
            .cloned())
    }

    async fn save_transcript(&self, id: &str, text: &str) -> Result<(), CoreError> {
        if self.fail_transcript_saves.load(Ordering::SeqCst) {
            return Err(CoreError::Storage(
                "injected transcript persistence failure".into(),
            ));
        }
        let mut store = self.store.lock().unwrap();
        if let Some(m) = store.iter_mut().find(|m| m.id == id) {
            m.transcript_text = Some(text.to_string());
            Ok(())
        } else {
            Err(CoreError::NotFound(id.to_string()))
        }
    }

    async fn save_protocol(&self, id: &str, text: &str) -> Result<(), CoreError> {
        if self.fail_protocol_saves.load(Ordering::SeqCst) {
            return Err(CoreError::Storage(
                "injected protocol persistence failure".into(),
            ));
        }
        let mut store = self.store.lock().unwrap();
        if let Some(m) = store.iter_mut().find(|m| m.id == id) {
            m.protocol_text = Some(text.to_string());
            Ok(())
        } else {
            Err(CoreError::NotFound(id.to_string()))
        }
    }

    async fn save_transcript_file(
        &self,
        id: &str,
        text: &str,
        path: &Path,
    ) -> Result<(), CoreError> {
        if self.fail_transcript_saves.load(Ordering::SeqCst) {
            return Err(CoreError::Storage(
                "injected transcript persistence failure".into(),
            ));
        }
        let mut store = self.store.lock().unwrap();
        if let Some(m) = store.iter_mut().find(|m| m.id == id) {
            m.transcript_text = Some(text.to_string());
            m.transcript_path = Some(path.to_path_buf());
            Ok(())
        } else {
            Err(CoreError::NotFound(id.to_string()))
        }
    }

    async fn save_protocol_file(&self, id: &str, text: &str, path: &Path) -> Result<(), CoreError> {
        if self.fail_protocol_saves.load(Ordering::SeqCst) {
            return Err(CoreError::Storage(
                "injected protocol persistence failure".into(),
            ));
        }
        let mut store = self.store.lock().unwrap();
        if let Some(m) = store.iter_mut().find(|m| m.id == id) {
            m.protocol_text = Some(text.to_string());
            m.protocol_path = Some(path.to_path_buf());
            Ok(())
        } else {
            Err(CoreError::NotFound(id.to_string()))
        }
    }

    async fn update_name(&self, id: &str, name: &str) -> Result<(), CoreError> {
        let mut store = self.store.lock().unwrap();
        if let Some(m) = store.iter_mut().find(|m| m.id == id) {
            m.name = name.to_string();
            Ok(())
        } else {
            Err(CoreError::NotFound(id.to_string()))
        }
    }

    async fn list_all(&self) -> Result<Vec<Meeting>, CoreError> {
        Ok(self.store.lock().unwrap().clone())
    }

    async fn delete_audio_only(&self, id: &str) -> Result<(), CoreError> {
        let mut store = self.store.lock().unwrap();
        if let Some(m) = store.iter_mut().find(|m| m.id == id) {
            m.audio_path = PathBuf::new();
            Ok(())
        } else {
            Err(CoreError::NotFound(id.to_string()))
        }
    }

    async fn clear_transcript(&self, id: &str) -> Result<(), CoreError> {
        let mut store = self.store.lock().unwrap();
        if let Some(m) = store.iter_mut().find(|m| m.id == id) {
            m.transcript_text = None;
            m.transcript_path = None;
            Ok(())
        } else {
            Err(CoreError::NotFound(id.to_string()))
        }
    }

    async fn clear_protocol(&self, id: &str) -> Result<(), CoreError> {
        let mut store = self.store.lock().unwrap();
        if let Some(m) = store.iter_mut().find(|m| m.id == id) {
            m.protocol_text = None;
            m.protocol_path = None;
            Ok(())
        } else {
            Err(CoreError::NotFound(id.to_string()))
        }
    }

    async fn delete(&self, id: &str) -> Result<(), CoreError> {
        self.store.lock().unwrap().retain(|m| m.id != id);
        Ok(())
    }
}

// ── FakeMeetingFileStore ─────────────────────────────────────────────────────

#[derive(Default)]
pub struct FakeMeetingFileStore {
    pub written: Mutex<Vec<(PathBuf, String)>>,
    /// `(dest_dir, source)` for each `import_audio` call.
    pub imported: Mutex<Vec<(PathBuf, PathBuf)>>,
    pub removed_files: Mutex<Vec<PathBuf>>,
    pub removed_dirs: Mutex<Vec<PathBuf>>,
    /// Files returned by `list_audio_files`, regardless of the queried dir.
    audio_files: Mutex<Vec<PathBuf>>,
    fail_result_writes: AtomicBool,
}

impl FakeMeetingFileStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn with_audio_files(files: impl IntoIterator<Item = PathBuf>) -> Arc<Self> {
        Arc::new(Self {
            audio_files: Mutex::new(files.into_iter().collect()),
            ..Self::default()
        })
    }

    pub fn set_fail_result_writes(&self, fail: bool) {
        self.fail_result_writes.store(fail, Ordering::SeqCst);
    }
}

#[async_trait]
impl MeetingFileStore for FakeMeetingFileStore {
    async fn write_transcript(&self, dir: &Path, text: &str) -> Result<PathBuf, CoreError> {
        if self.fail_result_writes.load(Ordering::SeqCst) {
            return Err(CoreError::Storage(
                "injected transcript file failure".into(),
            ));
        }
        let path = dir.join("transcript.md");
        self.written
            .lock()
            .unwrap()
            .push((path.clone(), text.to_string()));
        Ok(path)
    }

    async fn write_protocol(&self, dir: &Path, text: &str) -> Result<PathBuf, CoreError> {
        if self.fail_result_writes.load(Ordering::SeqCst) {
            return Err(CoreError::Storage("injected protocol file failure".into()));
        }
        let path = dir.join("protocol.md");
        self.written
            .lock()
            .unwrap()
            .push((path.clone(), text.to_string()));
        Ok(path)
    }

    async fn import_audio(&self, dir: &Path, source: &Path) -> Result<PathBuf, CoreError> {
        self.imported
            .lock()
            .unwrap()
            .push((dir.to_path_buf(), source.to_path_buf()));
        let name = source.file_name().unwrap_or_default();
        Ok(dir.join(name))
    }

    async fn list_audio_files(
        &self,
        _dir: &Path,
        _max_depth: usize,
    ) -> Result<Vec<PathBuf>, CoreError> {
        Ok(self.audio_files.lock().unwrap().clone())
    }

    async fn remove_file(&self, path: &Path) -> Result<(), CoreError> {
        self.removed_files.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }

    async fn remove_dir_all(&self, dir: &Path) -> Result<(), CoreError> {
        self.removed_dirs.lock().unwrap().push(dir.to_path_buf());
        Ok(())
    }
}

// ── FakeJobRepo ──────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct FakeJobRepo {
    store: Mutex<Vec<Job>>,
    fail_protocol_enqueue: AtomicBool,
    fail_mark_done: AtomicBool,
}

impl FakeJobRepo {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn set_fail_protocol_enqueue(&self, fail: bool) {
        self.fail_protocol_enqueue.store(fail, Ordering::SeqCst);
    }

    pub fn set_fail_mark_done(&self, fail: bool) {
        self.fail_mark_done.store(fail, Ordering::SeqCst);
    }
}

#[async_trait]
impl JobRepo for FakeJobRepo {
    async fn enqueue(&self, job: &Job) -> Result<(), CoreError> {
        if job.kind == JobKind::RegenerateProtocol
            && self.fail_protocol_enqueue.load(Ordering::SeqCst)
        {
            return Err(CoreError::Storage(
                "injected chained protocol enqueue failure".into(),
            ));
        }
        self.store.lock().unwrap().push(job.clone());
        Ok(())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<Job>, CoreError> {
        Ok(self
            .store
            .lock()
            .unwrap()
            .iter()
            .find(|j| j.id == id)
            .cloned())
    }

    async fn list_active(&self) -> Result<Vec<Job>, CoreError> {
        let mut jobs: Vec<Job> = self
            .store
            .lock()
            .unwrap()
            .iter()
            .filter(|j| matches!(j.status, JobStatus::Pending | JobStatus::Running))
            .cloned()
            .collect();
        jobs.sort_by_key(|j| j.created_at);
        Ok(jobs)
    }

    async fn claim_pending_kind(
        &self,
        kinds: &[JobKind],
        now_ts: i64,
    ) -> Result<Option<Job>, CoreError> {
        let mut store = self.store.lock().unwrap();
        let idx = store.iter().position(|j| {
            j.status == JobStatus::Pending
                && j.retry_after <= now_ts
                && kinds.iter().any(|k| k == &j.kind)
        });
        if let Some(i) = idx {
            store[i].status = JobStatus::Running;
            store[i].updated_at = now_ts;
            Ok(Some(store[i].clone()))
        } else {
            Ok(None)
        }
    }

    async fn mark_done(&self, id: &str, now_ts: i64) -> Result<(), CoreError> {
        if self.fail_mark_done.load(Ordering::SeqCst) {
            return Err(CoreError::Storage("injected mark_done failure".into()));
        }
        let mut store = self.store.lock().unwrap();
        if let Some(j) = store.iter_mut().find(|j| j.id == id) {
            j.status = JobStatus::Done;
            j.updated_at = now_ts;
        }
        Ok(())
    }

    async fn reset_for_retry(
        &self,
        id: &str,
        error: &str,
        attempts: u32,
        retry_after: i64,
        now_ts: i64,
    ) -> Result<(), CoreError> {
        let mut store = self.store.lock().unwrap();
        if let Some(j) = store.iter_mut().find(|j| j.id == id) {
            j.status = JobStatus::Pending;
            j.attempts = attempts;
            j.last_error = Some(error.to_string());
            j.retry_after = retry_after;
            j.updated_at = now_ts;
        }
        Ok(())
    }

    async fn mark_permanently_failed(
        &self,
        id: &str,
        error: &str,
        error_class: Option<&str>,
        attempts: u32,
        now_ts: i64,
    ) -> Result<(), CoreError> {
        let mut store = self.store.lock().unwrap();
        if let Some(j) = store.iter_mut().find(|j| j.id == id) {
            j.status = JobStatus::Failed;
            j.attempts = attempts;
            j.last_error = Some(error.to_string());
            j.error_class = error_class.and_then(crate::entities::ErrorClass::parse);
            j.updated_at = now_ts;
        }
        Ok(())
    }

    async fn recover_running_jobs(&self, now_ts: i64) -> Result<u64, CoreError> {
        let mut store = self.store.lock().unwrap();
        let mut count = 0u64;
        for j in store.iter_mut().filter(|j| j.status == JobStatus::Running) {
            j.status = JobStatus::Pending;
            j.attempts += 1;
            j.last_error = Some("interrupted by process restart".into());
            j.updated_at = now_ts;
            count += 1;
        }
        Ok(count)
    }

    async fn cancel_pending(&self, id: &str, now_ts: i64) -> Result<u64, CoreError> {
        let mut store = self.store.lock().unwrap();
        if let Some(j) = store.iter_mut().find(|j| j.id == id) {
            if j.status == JobStatus::Pending {
                j.status = JobStatus::Failed;
                j.error_class = Some(crate::entities::ErrorClass::Cancelled);
                j.last_error = Some("cancelled by user".into());
                j.updated_at = now_ts;
                return Ok(1);
            }
        }
        Ok(0)
    }

    async fn mark_cancelled(&self, id: &str, now_ts: i64) -> Result<u64, CoreError> {
        let mut store = self.store.lock().unwrap();
        if let Some(j) = store.iter_mut().find(|j| j.id == id) {
            if matches!(j.status, JobStatus::Pending | JobStatus::Running) {
                j.status = JobStatus::Failed;
                j.error_class = Some(crate::entities::ErrorClass::Cancelled);
                j.last_error = Some("cancelled by user".into());
                j.updated_at = now_ts;
                return Ok(1);
            }
        }
        Ok(0)
    }
}

// ── FakeLlmProvider ──────────────────────────────────────────────────────────

pub struct FakeLlmProvider {
    response: String,
}

impl FakeLlmProvider {
    pub fn new(response: impl Into<String>) -> Arc<Self> {
        Arc::new(Self {
            response: response.into(),
        })
    }
}

#[async_trait]
impl LlmProvider for FakeLlmProvider {
    async fn generate(
        &self,
        _transcript: &str,
        _instructions: Option<&str>,
    ) -> Result<String, CoreError> {
        Ok(self.response.clone())
    }
}

// ── FakeTemplateLoader ───────────────────────────────────────────────────────

pub struct FakeTemplateLoader {
    templates: Mutex<HashMap<String, String>>,
}

impl FakeTemplateLoader {
    pub fn new(
        templates: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Arc<Self> {
        Arc::new(Self {
            templates: Mutex::new(
                templates
                    .into_iter()
                    .map(|(k, v)| (k.into(), v.into()))
                    .collect(),
            ),
        })
    }

    pub fn empty() -> Arc<Self> {
        Arc::new(Self {
            templates: Mutex::new(HashMap::new()),
        })
    }
}

#[async_trait]
impl TemplateLoader for FakeTemplateLoader {
    async fn load(&self, name: &str) -> Result<Option<String>, CoreError> {
        Ok(self.templates.lock().unwrap().get(name).cloned())
    }

    async fn list_names(&self) -> Result<Vec<String>, CoreError> {
        Ok(self.templates.lock().unwrap().keys().cloned().collect())
    }

    async fn save(&self, name: &str, body: &str) -> Result<(), CoreError> {
        self.templates
            .lock()
            .unwrap()
            .insert(name.to_string(), body.to_string());
        Ok(())
    }

    async fn delete(&self, name: &str) -> Result<(), CoreError> {
        self.templates
            .lock()
            .unwrap()
            .remove(name)
            .map(|_| ())
            .ok_or_else(|| CoreError::NotFound(format!("template '{name}'")))
    }

    async fn rename(&self, old: &str, new: &str) -> Result<(), CoreError> {
        let mut t = self.templates.lock().unwrap();
        let body = t
            .remove(old)
            .ok_or_else(|| CoreError::NotFound(format!("template '{old}'")))?;
        t.insert(new.to_string(), body);
        Ok(())
    }
}

// ── FakeAudioCapture ─────────────────────────────────────────────────────────

#[derive(Default)]
pub struct FakeAudioCapture {
    active: Mutex<HashSet<String>>,
    /// Each entry is `(session_id, spec)` in call order.
    pub started: Mutex<Vec<(String, CaptureSpec)>>,
    pub stopped: Mutex<Vec<String>>,
}

impl FakeAudioCapture {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Returns the full `CaptureSpec` for the most-recently started session.
    pub fn last_spec(&self) -> Option<CaptureSpec> {
        self.started.lock().unwrap().last().map(|(_, s)| s.clone())
    }

    /// Returns the `CaptureSource` for the most-recently started session, if any.
    pub fn last_source(&self) -> Option<CaptureSource> {
        self.started.lock().unwrap().last().map(|(_, s)| s.source)
    }

    /// Returns the `echo_cancel` flag for the most-recently started session, if any.
    pub fn last_echo_cancel(&self) -> Option<bool> {
        self.started
            .lock()
            .unwrap()
            .last()
            .map(|(_, s)| s.echo_cancel)
    }
}

#[async_trait]
impl AudioCapture for FakeAudioCapture {
    async fn start_session(
        &self,
        session_id: &str,
        _output_path: &Path,
        spec: CaptureSpec,
    ) -> Result<ResolvedDevices, CoreError> {
        self.active.lock().unwrap().insert(session_id.to_string());
        // Echo the request back as "resolved": the fake has no real devices, so
        // a pinned name resolves to itself and `None` stays the default.
        let resolved = ResolvedDevices {
            mic: match spec.source {
                CaptureSource::Mic | CaptureSource::Mixed => {
                    Some(spec.mic_device.clone().unwrap_or_else(|| "default".into()))
                }
                CaptureSource::System => None,
            },
            system: match spec.source {
                CaptureSource::System | CaptureSource::Mixed => Some(
                    spec.system_device
                        .clone()
                        .unwrap_or_else(|| "default".into()),
                ),
                CaptureSource::Mic => None,
            },
        };
        self.started
            .lock()
            .unwrap()
            .push((session_id.to_string(), spec));
        Ok(resolved)
    }

    async fn stop_session(&self, session_id: &str) -> Result<(), CoreError> {
        let removed = self.active.lock().unwrap().remove(session_id);
        if !removed {
            return Err(CoreError::Recording(format!(
                "session not found: {session_id}"
            )));
        }
        self.stopped.lock().unwrap().push(session_id.to_string());
        Ok(())
    }

    fn is_active(&self, session_id: &str) -> bool {
        self.active.lock().unwrap().contains(session_id)
    }
}

// ── FakeAudioDeviceEnumerator ────────────────────────────────────────────────

/// Returns a fixed device list. Defaults to one mic + one system source, both
/// marked default, with `system_selectable = true`.
pub struct FakeAudioDeviceEnumerator {
    list: AudioDeviceList,
}

impl Default for FakeAudioDeviceEnumerator {
    fn default() -> Self {
        Self {
            list: AudioDeviceList {
                input: vec![AudioDevice {
                    id: "fake-mic".into(),
                    label: "Fake Microphone".into(),
                    is_default: true,
                }],
                output: vec![AudioDevice {
                    id: "fake-monitor".into(),
                    label: "Fake System Output".into(),
                    is_default: true,
                }],
                system_selectable: true,
            },
        }
    }
}

impl FakeAudioDeviceEnumerator {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn with_list(list: AudioDeviceList) -> Arc<Self> {
        Arc::new(Self { list })
    }
}

#[async_trait]
impl AudioDeviceEnumerator for FakeAudioDeviceEnumerator {
    async fn list_devices(&self) -> Result<AudioDeviceList, CoreError> {
        Ok(self.list.clone())
    }
}

// ── FakeAudioLevelMonitor ─────────────────────────────────────────────────────

/// Records start/stop calls and returns a fixed level for any active session.
#[derive(Default)]
pub struct FakeAudioLevelMonitor {
    active: Mutex<HashSet<String>>,
    pub started: Mutex<Vec<(String, CaptureSpec)>>,
}

impl FakeAudioLevelMonitor {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

#[async_trait]
impl AudioLevelMonitor for FakeAudioLevelMonitor {
    async fn start(&self, id: &str, spec: CaptureSpec) -> Result<ResolvedDevices, CoreError> {
        self.active.lock().unwrap().insert(id.to_string());
        let resolved = ResolvedDevices {
            mic: match spec.source {
                CaptureSource::Mic => {
                    Some(spec.mic_device.clone().unwrap_or_else(|| "default".into()))
                }
                _ => None,
            },
            system: match spec.source {
                CaptureSource::System => Some(
                    spec.system_device
                        .clone()
                        .unwrap_or_else(|| "default".into()),
                ),
                _ => None,
            },
        };
        self.started.lock().unwrap().push((id.to_string(), spec));
        Ok(resolved)
    }

    fn level(&self, id: &str) -> Option<AudioLevel> {
        if self.active.lock().unwrap().contains(id) {
            Some(AudioLevel {
                level: 0.5,
                peak_db: -6.0,
            })
        } else {
            None
        }
    }

    async fn stop(&self, id: &str) -> Result<(), CoreError> {
        if self.active.lock().unwrap().remove(id) {
            Ok(())
        } else {
            Err(CoreError::Recording(format!("monitor not found: {id}")))
        }
    }
}

// ── FakeTemplateBundle ───────────────────────────────────────────────────────

pub struct FakeTemplateBundle {
    entries: Vec<(String, String)>,
}

impl FakeTemplateBundle {
    pub fn new(entries: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>) -> Self {
        Self {
            entries: entries
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }
}

impl TemplateBundle for FakeTemplateBundle {
    fn entries(&self) -> Vec<(String, String)> {
        self.entries.clone()
    }
}

// ── Port ⇄ fake compile-time contract ────────────────────────────────────────
//
// Each binding coerces a fake into its port's trait object. The module compiles
// only while every fake still satisfies its port trait — a signature change to a
// port (new method, changed argument) that leaves a fake stale becomes a build
// error here, at `cargo test -p meeting-core`, instead of surfacing later in
// whichever downstream test happens to exercise the changed method.
//
// Zero runtime cost: the bindings live entirely in the test build and never run.
// Add one line per new port/fake pair so the contract stays exhaustive.
#[cfg(test)]
mod port_fake_contract {
    use super::*;

    #[test]
    fn every_fake_satisfies_its_port() {
        let _: Arc<dyn Transcriber> = FakeTranscriber::new("");
        let _: Arc<dyn MeetingRepo> = FakeMeetingRepo::new();
        let _: Arc<dyn MeetingFileStore> = FakeMeetingFileStore::new();
        let _: Arc<dyn JobRepo> = FakeJobRepo::new();
        let _: Arc<dyn LlmProvider> = FakeLlmProvider::new("");
        let _: Arc<dyn TemplateLoader> = FakeTemplateLoader::empty();
        let _: Arc<dyn TemplateBundle> = Arc::new(FakeTemplateBundle::new([("a", "b")]));
        let _: Arc<dyn AudioCapture> = FakeAudioCapture::new();
    }
}
