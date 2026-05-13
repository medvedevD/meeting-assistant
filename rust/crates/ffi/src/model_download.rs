use std::sync::Arc;
use std::time::Duration;

use crate::app_core::AppCore;
use crate::types::AppError;

// UniFFI 0.28 does not support callback_interface in async functions.
// download_model is therefore a blocking function — callers must invoke it
// from a background thread (e.g. Kotlin Dispatchers.IO).
#[uniffi::export(callback_interface)]
pub trait ModelDownloadCallback: Send + Sync {
    fn on_progress(&self, bytes_downloaded: i64, total_bytes: i64);
    fn on_complete(&self);
    fn on_error(&self, message: String);
}

/// Returns true if the default model file exists on disk.
#[uniffi::export]
pub fn model_exists(core: Arc<AppCore>) -> bool {
    core.model_path.exists()
}

/// Download a model from `url` to `dest_path` (or core.model_path if None).
/// Reports progress via `callback` on each chunk.
/// Uses atomic write: downloads to .tmp, renames on success.
/// Blocking — call from a background thread (Kotlin: Dispatchers.IO).
#[uniffi::export]
pub fn download_model(
    core: Arc<AppCore>,
    url: String,
    dest_path: Option<String>,
    callback: Box<dyn ModelDownloadCallback>,
) -> Result<(), AppError> {
    use reqwest::blocking::Client;

    let dest = dest_path
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| core.model_path.clone());

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::General(e.to_string()))?;
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| AppError::General(e.to_string()))?;

    let response = client
        .get(&url)
        .send()
        .map_err(|e| AppError::General(e.to_string()))?;

    if !response.status().is_success() {
        return Err(AppError::General(format!(
            "HTTP {} for {}",
            response.status(),
            url
        )));
    }

    let total = response.content_length().unwrap_or(0) as i64;
    let mut downloaded = 0i64;

    let tmp_path = dest.with_extension("tmp");
    let mut file = std::fs::File::create(&tmp_path)
        .map_err(|e| AppError::General(e.to_string()))?;

    use std::io::{Read, Write};
    let mut reader = response;
    let mut buf = vec![0u8; 65536];
    loop {
        let n = reader.read(&mut buf)
            .map_err(|e| AppError::General(e.to_string()))?;
        if n == 0 { break; }
        file.write_all(&buf[..n])
            .map_err(|e| AppError::General(e.to_string()))?;
        downloaded += n as i64;
        callback.on_progress(downloaded, total);
    }

    drop(file);

    std::fs::rename(&tmp_path, &dest)
        .map_err(|e| AppError::General(e.to_string()))?;

    callback.on_complete();
    Ok(())
}
