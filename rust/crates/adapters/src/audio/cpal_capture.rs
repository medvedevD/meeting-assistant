use std::collections::HashMap;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use async_trait::async_trait;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use meeting_core::{CoreError, ports::AudioCapture};

struct Session {
    stop_tx: std::sync::mpsc::SyncSender<()>,
    thread: thread::JoinHandle<Result<(), String>>,
}

pub struct CpalAudioCapture {
    sessions: Mutex<HashMap<String, Session>>,
}

impl CpalAudioCapture {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(HashMap::new()) }
    }
}

impl Default for CpalAudioCapture {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AudioCapture for CpalAudioCapture {
    async fn start_session(&self, session_id: &str, output_path: &Path) -> Result<(), CoreError> {
        let path = output_path.to_path_buf();
        let id = session_id.to_string();
        let (stop_tx, stop_rx) = std::sync::mpsc::sync_channel(1);

        let thread = thread::Builder::new()
            .name(format!("cpal-recording-{id}"))
            .spawn(move || record_to_file(path, stop_rx))
            .map_err(|e| CoreError::Recording(e.to_string()))?;

        self.sessions
            .lock()
            .unwrap()
            .insert(session_id.to_string(), Session { stop_tx, thread });

        Ok(())
    }

    async fn stop_session(&self, session_id: &str) -> Result<(), CoreError> {
        let session = self
            .sessions
            .lock()
            .unwrap()
            .remove(session_id)
            .ok_or_else(|| CoreError::Recording(format!("session not found: {session_id}")))?;

        // Signal the recording thread to stop. If it already exited (error), ignore send failure.
        let _ = session.stop_tx.send(());

        session
            .thread
            .join()
            .map_err(|_| CoreError::Recording("recording thread panicked".into()))?
            .map_err(CoreError::Recording)
    }

    fn is_active(&self, session_id: &str) -> bool {
        self.sessions.lock().unwrap().contains_key(session_id)
    }
}

fn record_to_file(
    output_path: PathBuf,
    stop_rx: std::sync::mpsc::Receiver<()>,
) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "no default input device available".to_string())?;

    let config = device
        .default_input_config()
        .map_err(|e| e.to_string())?;

    let spec = hound::WavSpec {
        channels: config.channels(),
        sample_rate: config.sample_rate().0,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let file = std::fs::File::create(&output_path).map_err(|e| e.to_string())?;
    let writer = Arc::new(Mutex::new(
        hound::WavWriter::new(BufWriter::new(file), spec).map_err(|e| e.to_string())?,
    ));

    let writer_cb = Arc::clone(&writer);
    let stream = device
        .build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                let mut w = writer_cb.lock().unwrap();
                for &s in data {
                    let _ = w.write_sample(s);
                }
            },
            |err| tracing::error!("cpal input stream error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())?;

    stream.play().map_err(|e| e.to_string())?;

    // Block until stop signal or channel closed.
    let _ = stop_rx.recv();

    // Stop stream before finalizing the writer.
    drop(stream);

    Arc::try_unwrap(writer)
        .map_err(|_| "writer Arc still has multiple owners".to_string())?
        .into_inner()
        .unwrap()
        .finalize()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests the state machine without touching real audio hardware.
    /// Actual audio I/O is covered by the `#[ignore]` integration test below.

    #[tokio::test]
    async fn is_active_false_before_start() {
        let cap = CpalAudioCapture::new();
        assert!(!cap.is_active("x"));
    }

    #[tokio::test]
    async fn stop_unknown_session_returns_err() {
        let cap = CpalAudioCapture::new();
        let err = cap.stop_session("no-such").await;
        assert!(matches!(err, Err(CoreError::Recording(_))));
    }

    /// Requires a real microphone / audio device.
    #[tokio::test]
    #[ignore = "requires audio hardware"]
    async fn records_non_empty_wav_file() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let path = dir.path().join("test").join("recording.wav");
        let cap = CpalAudioCapture::new();

        cap.start_session("s1", &path).await.unwrap();
        assert!(cap.is_active("s1"));

        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        cap.stop_session("s1").await.unwrap();

        assert!(!cap.is_active("s1"));
        let meta = std::fs::metadata(&path).unwrap();
        assert!(meta.len() > 44, "WAV should be bigger than just the header");
    }
}
