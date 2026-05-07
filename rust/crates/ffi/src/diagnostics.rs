use std::sync::Arc;
use crate::app_core::AppCore;
use crate::types::{
    AppError, DeviceInfoDto, DiagnosticsDto, DiagnosticsPathsDto, FfiResult, path_info,
};

#[uniffi::export(async_runtime = "tokio")]
impl AppCore {
    pub async fn diagnostics(self: Arc<Self>) -> FfiResult<DiagnosticsDto> {
        struct AudioInfo {
            cpal_host: String,
            input_devices: Vec<DeviceInfoDto>,
            output_devices: Vec<DeviceInfoDto>,
        }

        // CPAL device enumeration may block on PipeWire — run off the async executor.
        let audio = tokio::task::spawn_blocking(|| {
            use cpal::traits::{DeviceTrait, HostTrait};
            let host = cpal::default_host();
            let cpal_host = host.id().name().to_string();
            let default_in  = host.default_input_device().and_then(|d| d.name().ok());
            let default_out = host.default_output_device().and_then(|d| d.name().ok());
            let inputs = host.input_devices()
                .map(|it| it.filter_map(|d| d.name().ok())
                    .map(|name| DeviceInfoDto { is_default: default_in.as_deref() == Some(&name), name })
                    .collect())
                .unwrap_or_default();
            let outputs = host.output_devices()
                .map(|it| it.filter_map(|d| d.name().ok())
                    .map(|name| DeviceInfoDto { is_default: default_out.as_deref() == Some(&name), name })
                    .collect())
                .unwrap_or_default();
            AudioInfo { cpal_host, input_devices: inputs, output_devices: outputs }
        })
        .await
        .map_err(AppError::general)?;

        let ffmpeg_ok = tokio::process::Command::new("ffmpeg")
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false);

        let logs = {
            let lock = self.log_buffer.lock();
            lock.iter().cloned().collect()
        };

        Ok(DiagnosticsDto {
            os:           std::env::consts::OS.to_string(),
            arch:         std::env::consts::ARCH.to_string(),
            app_version:  env!("CARGO_PKG_VERSION").to_string(),
            cpal_host:    audio.cpal_host,
            input_devices:  audio.input_devices,
            output_devices: audio.output_devices,
            paths: DiagnosticsPathsDto {
                model:      path_info(&self.model_path),
                db:         path_info(&self.db_path),
                recordings: path_info(&self.recordings_dir),
                prompts:    path_info(&self.prompts_dir),
            },
            has_anthropic_key: std::env::var("ANTHROPIC_API_KEY").is_ok(),
            ffmpeg_ok,
            logs,
        })
    }

    pub async fn diagnostics_logs(self: Arc<Self>) -> FfiResult<Vec<String>> {
        Ok(self.log_buffer.lock().iter().cloned().collect())
    }

    pub async fn open_path(self: Arc<Self>, path: String) -> FfiResult<()> {
        let cmd = if cfg!(target_os = "linux")  { "xdg-open" }
                  else if cfg!(target_os = "macos") { "open" }
                  else { "explorer" };
        tokio::process::Command::new(cmd)
            .arg(&path)
            .spawn()
            .map_err(AppError::general)?;
        Ok(())
    }
}
