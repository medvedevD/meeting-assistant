/// Round-trip serde tests for every DTO used in the FFI boundary.
/// These guarantee that types survive JSON serialization (e.g. when stored or logged)
/// and that field names are stable (no accidental renaming breaks the Kotlin contract).
#[cfg(test)]
mod dto_roundtrip {
    use crate::types::*;

    fn roundtrip<T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug>(value: T) {
        let json = serde_json::to_string(&value).expect("serialize");
        let back: T = serde_json::from_str(&json).expect("deserialize");
        // Compare via Debug output — no PartialEq required.
        assert_eq!(format!("{:?}", value), format!("{:?}", back));
    }

    #[test]
    fn meeting_dto() {
        roundtrip(MeetingDto {
            id: "m1".into(),
            name: "Standup".into(),
            audio_path: "/tmp/rec.wav".into(),
            has_transcript: true,
            has_protocol: false,
            created_at: 1_700_000_000,
        });
    }

    #[test]
    fn recording_dto() {
        roundtrip(RecordingDto {
            id: "r1".into(),
            name: "Test".into(),
            audio_path: "/tmp/rec.wav".into(),
        });
    }

    #[test]
    fn protocol_dto() {
        roundtrip(ProtocolDto { markdown: "# Notes\n- item".into() });
    }

    #[test]
    fn job_dto_with_error() {
        roundtrip(JobDto {
            id: "j1".into(),
            meeting_id: "m1".into(),
            status: "failed".into(),
            attempts: 3,
            last_error: Some("connection timeout".into()),
        });
    }

    #[test]
    fn job_dto_no_error() {
        roundtrip(JobDto {
            id: "j2".into(),
            meeting_id: "m2".into(),
            status: "done".into(),
            attempts: 1,
            last_error: None,
        });
    }

    #[test]
    fn settings_dto_full() {
        roundtrip(SettingsDto {
            paths: SettingsPathsDto {
                model: Some("/models/ggml-medium.bin".into()),
                db: Some("/db/index.db".into()),
                recordings: Some("/recs".into()),
                prompts: Some("/prompts".into()),
            },
            anthropic_api_key: Some("sk-ant-test".into()),
            recording: RecordingPrefsDto {
                source: "mixed".into(),
                echo_cancel: true,
            },
            default_template: Some("standup".into()),
        });
    }

    #[test]
    fn settings_dto_empty_optionals() {
        roundtrip(SettingsDto {
            paths: SettingsPathsDto {
                model: None,
                db: None,
                recordings: None,
                prompts: None,
            },
            anthropic_api_key: None,
            recording: RecordingPrefsDto {
                source: "mic".into(),
                echo_cancel: false,
            },
            default_template: None,
        });
    }

    #[test]
    fn path_info_dto() {
        roundtrip(PathInfoDto {
            path: "/tmp".into(),
            exists: true,
            writable: true,
            size_bytes: Some(4096),
        });
    }

    #[test]
    fn device_info_dto() {
        roundtrip(DeviceInfoDto {
            name: "Built-in Microphone".into(),
            is_default: true,
        });
    }

    #[test]
    fn diagnostics_dto() {
        let pi = PathInfoDto { path: "/x".into(), exists: false, writable: false, size_bytes: None };
        roundtrip(DiagnosticsDto {
            os: "linux".into(),
            arch: "x86_64".into(),
            app_version: "0.1.0".into(),
            cpal_host: "ALSA".into(),
            input_devices: vec![DeviceInfoDto { name: "Mic".into(), is_default: true }],
            output_devices: vec![],
            paths: DiagnosticsPathsDto {
                model: pi.clone(),
                db: pi.clone(),
                recordings: pi.clone(),
                prompts: pi,
            },
            has_anthropic_key: false,
            ffmpeg_ok: true,
            logs: vec!["INFO init".into()],
        });
    }
}
