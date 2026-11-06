//! Sidecar contract tests (Qt-migration section-08).
//!
//! These spawn the **real `meeting-server` binary** (the same artefact the Qt
//! GUI launches) and assert the lifecycle/protocol contract section-02
//! promises the client: a single JSON handshake as the first stdout bytes,
//! logs on stderr only, bearer-gated `/api/*`, loopback-only bind,
//! `/version`, parent-death reaping (POSIX pipe EOF + `--parent-pid` poll),
//! the distinct singleton exit code, and graceful SIGTERM shutdown.
//!
//! Each server gets its own temp dir (see [`isolated_server`]) so the SQLite
//! DB, recordings dir, settings and singleton lock are isolated and tests run
//! in parallel — except [`second_instance_exits_with_singleton_code`], which
//! deliberately shares one data dir between two instances.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SERVER_BIN: &str = env!("CARGO_BIN_EXE_meeting-server");

/// Generous upper bound for "parent gone → process exit": ~1 s PID-poll
/// period + up to 1.2 s worker drain + scheduler slack, padded for slow CI.
const REAP_BUDGET: Duration = Duration::from_secs(8);

/// The distinct exit code a second instance returns on lock contention
/// (`meeting-server.rs` `EXIT_SINGLETON_BUSY`).
const EXIT_SINGLETON_BUSY: i32 = 3;

/// The directory [`isolated_server`] configures as the recordings dir.
fn recordings_dir(dir: &Path) -> PathBuf {
    dir.join("recordings")
}

/// A `meeting-server` command that reads and writes only under `dir`.
///
/// `XDG_*` covers the DB, the singleton lock, `settings.json` and the secrets
/// file, but not the recordings dir, which defaults to the OS Documents folder:
/// that follows `HOME` on Linux and macOS and no env var at all on Windows. The
/// `paths.meetings_dir` setting is what moves it on every OS. Without this the
/// startup recovery pass would rewrite damaged WAVs in the developer's real
/// recordings, including one the running app is still writing.
fn isolated_server(dir: &Path) -> Command {
    let recordings = recordings_dir(dir);
    std::fs::create_dir_all(&recordings).expect("create recordings dir");

    // Two instances can share `dir`. Naming the language keeps this equal to a
    // new install: a settings file without one means the legacy "ru".
    let settings = serde_json::json!({
        "paths": { "meetings_dir": recordings },
        "recording": { "source": "mic", "echo_cancel": false },
        "transcriber": { "language": "auto" },
    });
    let settings_path = dir.join("meeting-assistant/settings.json");
    std::fs::create_dir_all(settings_path.parent().unwrap()).expect("create config dir");
    std::fs::write(&settings_path, settings.to_string()).expect("write settings.json");

    let mut cmd = Command::new(SERVER_BIN);
    cmd.env("XDG_DATA_HOME", dir)
        .env("XDG_CACHE_HOME", dir)
        .env("XDG_CONFIG_HOME", dir)
        .env("HOME", dir.join("home"))
        // Tests never read or write the real OS keyring.
        .env("MEETING_ASSISTANT_KEYRING_DISABLE", "1")
        .env_remove("ANTHROPIC_API_KEY")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

/// The one machine-readable line `meeting-server` prints to stdout before it
/// serves anything. Mirrors the section-02 wire shape.
#[derive(Debug, serde::Deserialize)]
struct Handshake {
    ready: bool,
    port: u16,
    token: String,
    protocol: u32,
    min_protocol: u32,
    build: String,
}

/// A spawned `meeting-server` plus its captured streams. `stderr` is drained
/// on a background thread into a shared buffer so (a) the child never blocks
/// on a full pipe and (b) tests can assert logs landed on stderr.
struct Sidecar {
    child: Child,
    handshake: Handshake,
    /// The exact raw bytes read from stdout up to and including the handshake
    /// newline — used to prove the handshake is the *first* bytes on stdout.
    stdout_prefix: Vec<u8>,
    stderr: Arc<Mutex<Vec<u8>>>,
    _data_dir: tempfile::TempDir,
}

impl Sidecar {
    /// Spawn with a fresh isolated data dir.
    fn spawn() -> Sidecar {
        let data = tempfile::tempdir().expect("tempdir");
        let data_path = data.path().to_path_buf();
        Sidecar::spawn_in(&data_path, &[], data)
    }

    /// Spawn pointing at `data_dir` (shared between instances for the
    /// singleton test), with extra args, taking ownership of the temp guard.
    fn spawn_in(data_dir: &Path, extra_args: &[String], guard: tempfile::TempDir) -> Sidecar {
        let mut cmd = isolated_server(data_dir);
        cmd.args(extra_args).env("RUST_LOG", "info");
        let mut child = cmd.spawn().expect("spawn meeting-server");

        // Drain stderr off-thread: never block the child, capture for asserts.
        let stderr_buf = Arc::new(Mutex::new(Vec::new()));
        {
            let mut err = child.stderr.take().expect("stderr pipe");
            let sink = Arc::clone(&stderr_buf);
            std::thread::spawn(move || {
                let mut chunk = [0u8; 4096];
                while let Ok(n) = err.read(&mut chunk) {
                    if n == 0 {
                        break;
                    }
                    sink.lock().unwrap().extend_from_slice(&chunk[..n]);
                }
            });
        }

        // Read exactly the first stdout line (the handshake), with a timeout,
        // capturing the raw prefix so we can assert it is the first bytes.
        let stdout = child.stdout.take().expect("stdout pipe");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = Vec::new();
            let res = reader.read_until(b'\n', &mut line);
            let _ = tx.send(res.map(|_| line));
        });
        let stdout_prefix = match rx.recv_timeout(Duration::from_secs(20)) {
            Ok(Ok(line)) => line,
            other => {
                let _ = child.kill();
                panic!(
                    "no handshake line within 20s: {other:?}\nstderr:\n{}",
                    String::from_utf8_lossy(&stderr_buf.lock().unwrap())
                );
            }
        };

        let handshake: Handshake = serde_json::from_slice(&stdout_prefix).unwrap_or_else(|e| {
            let _ = child.kill();
            panic!(
                "handshake is not valid JSON ({e}): {:?}",
                String::from_utf8_lossy(&stdout_prefix)
            )
        });

        Sidecar {
            child,
            handshake,
            stdout_prefix,
            stderr: stderr_buf,
            _data_dir: guard,
        }
    }

    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.handshake.port)
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    fn stderr_string(&self) -> String {
        String::from_utf8_lossy(&self.stderr.lock().unwrap()).into_owned()
    }

    /// The log of a server that has exited, up to its last line. The thread that
    /// drains stderr can still be catching up when the process is gone.
    #[cfg(unix)]
    fn final_log(&self) -> String {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let log = self.stderr_string();
            if log.contains("graceful shutdown complete") || Instant::now() >= deadline {
                return log;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Wait up to `budget` for the process to exit; return its status.
    fn wait_for_exit(&mut self, budget: Duration) -> Option<std::process::ExitStatus> {
        let deadline = Instant::now() + budget;
        loop {
            match self.child.try_wait().expect("try_wait") {
                Some(status) => return Some(status),
                None if Instant::now() >= deadline => return None,
                None => std::thread::sleep(Duration::from_millis(50)),
            }
        }
    }
}

impl Drop for Sidecar {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("reqwest client")
}

// ── Recordings dir ───────────────────────────────────────────────────────────

/// A recording whose writer died: samples on disk, both RIFF lengths still the
/// placeholder 0, and a torn last frame. The recovery pass rewrites this file
/// whenever it reaches it.
fn crashed_recording() -> Vec<u8> {
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&0u32.to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&48_000u32.to_le_bytes());
    wav.extend_from_slice(&192_000u32.to_le_bytes()); // byte rate
    wav.extend_from_slice(&4u16.to_le_bytes()); // block align
    wav.extend_from_slice(&32u16.to_le_bytes()); // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&0u32.to_le_bytes());
    wav.extend_from_slice(&[0u8; 403]); // 100 frames + 3 bytes of a torn one
    wav
}

fn plant_crashed_recording(recordings: &Path) -> PathBuf {
    let meeting = recordings.join("2026-05-10_14-30_a1b2c3d4");
    std::fs::create_dir_all(&meeting).expect("create meeting dir");
    let wav = meeting.join("recording.wav");
    std::fs::write(&wav, crashed_recording()).expect("write recording");
    wav
}

#[test]
fn recovery_uses_the_configured_recordings_dir_and_leaves_the_default_alone() {
    let data = tempfile::tempdir().unwrap();
    let data_path = data.path().to_path_buf();

    let configured = plant_crashed_recording(&recordings_dir(&data_path));
    // Where the default resolves under the isolated HOME on Linux and macOS. On
    // Windows nothing can reach this copy, so `configured` carries the test.
    let at_default = plant_crashed_recording(&data_path.join("home/Documents/meeting-assistant"));

    // The handshake is printed after the recovery pass, so that has finished.
    let s = Sidecar::spawn_in(&data_path, &[], data);

    let meetings: serde_json::Value = client()
        .get(format!("{}/api/v1/meetings", s.base_url()))
        .bearer_auth(&s.handshake.token)
        .send()
        .expect("list meetings")
        .json()
        .expect("json");
    let listed: Vec<&str> = meetings
        .as_array()
        .expect("meetings array")
        .iter()
        .map(|m| m["audio_path"].as_str().expect("audio_path"))
        .collect();
    assert_eq!(listed, [configured.to_str().unwrap()]);

    assert_eq!(
        std::fs::read(&configured).unwrap().len(),
        444,
        "recovery must finalize the recording in the configured dir"
    );
    assert_eq!(
        std::fs::read(&at_default).unwrap(),
        crashed_recording(),
        "recovery must not touch the default location when a dir is configured"
    );
}

// ── Handshake + stdout/stderr discipline ─────────────────────────────────────

#[test]
fn handshake_is_valid_json_and_the_first_stdout_bytes() {
    let s = Sidecar::spawn();

    // The captured prefix IS the whole handshake line and nothing precedes it:
    // it parsed as JSON above and ends in exactly one newline.
    assert!(
        s.stdout_prefix.ends_with(b"\n"),
        "handshake line must be newline-terminated"
    );
    assert_eq!(
        s.stdout_prefix.iter().filter(|&&b| b == b'\n').count(),
        1,
        "exactly one stdout line up to the handshake"
    );

    assert!(s.handshake.ready, "ready must be true");
    assert!(s.handshake.port > 0, "port must be assigned");
    assert_eq!(
        s.handshake.token.len(),
        64,
        "token is a 256-bit value as 64 hex chars"
    );
    assert!(
        s.handshake.token.bytes().all(|b| b.is_ascii_hexdigit()),
        "token must be hex"
    );
    assert!(s.handshake.protocol >= 1);
    assert!(s.handshake.min_protocol >= 1);
    assert!(s.handshake.min_protocol <= s.handshake.protocol);
    assert!(!s.handshake.build.is_empty());
}

#[test]
fn logs_go_to_stderr_only_never_stdout() {
    let s = Sidecar::spawn();

    // Give the "sidecar listening" info log time to land on stderr.
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && !s.stderr_string().contains("sidecar listening") {
        std::thread::sleep(Duration::from_millis(50));
    }
    let err = s.stderr_string();
    assert!(
        err.contains("sidecar listening"),
        "tracing logs must reach stderr; got:\n{err}"
    );
    // The token must never appear in logs (it travels only via the handshake).
    assert!(
        !err.contains(&s.handshake.token),
        "the bearer token must never be logged"
    );
}

// ── Auth: every /api/* route is bearer-gated ─────────────────────────────────

#[test]
fn api_routes_401_without_token_and_200_with_it() {
    let s = Sidecar::spawn();
    let c = client();
    let url = format!("{}/api/v1/meetings", s.base_url());

    let no_token = c.get(&url).send().expect("request");
    assert_eq!(
        no_token.status(),
        reqwest::StatusCode::UNAUTHORIZED,
        "/api/v1/meetings must be 401 without a bearer token"
    );

    let wrong = c
        .get(&url)
        .bearer_auth("not-the-real-token")
        .send()
        .expect("request");
    assert_eq!(wrong.status(), reqwest::StatusCode::UNAUTHORIZED);

    let ok = c
        .get(&url)
        .bearer_auth(&s.handshake.token)
        .send()
        .expect("request");
    assert_eq!(
        ok.status(),
        reqwest::StatusCode::OK,
        "valid bearer token must reach the handler (200)"
    );
}

/// Every `/api/v1/*` route the Phases 1–5 work added must sit behind the same
/// bearer gate as the original ones. This pins the *real binary's* router (not
/// just the in-crate unit list) so a route registered without the auth layer —
/// or merged onto a sub-router that skips it — fails here.
#[test]
fn every_api_route_is_bearer_gated_401_without_token() {
    let s = Sidecar::spawn();
    let c = client();

    // (method, path) for each auth-gated route, including the Phase 1–5 additions:
    // settings*, templates*, meetings/{import,scan,:id/reprocess,:id}.
    let routes: &[(&str, &str)] = &[
        ("POST", "/api/v1/jobs"),
        ("GET", "/api/v1/active-jobs"),
        ("GET", "/api/v1/jobs/abc"),
        ("GET", "/api/v1/jobs/abc/events"),
        ("POST", "/api/v1/protocols"),
        ("POST", "/api/v1/recordings"),
        ("POST", "/api/v1/recordings/abc/stop"),
        ("GET", "/api/v1/meetings"),
        ("POST", "/api/v1/meetings/import"),
        ("GET", "/api/v1/meetings/scan"),
        ("POST", "/api/v1/meetings/abc/reprocess"),
        ("DELETE", "/api/v1/meetings/abc"),
        ("GET", "/api/v1/settings"),
        ("PUT", "/api/v1/settings"),
        ("PUT", "/api/v1/settings/secret"),
        ("POST", "/api/v1/settings/test"),
        ("GET", "/api/v1/templates"),
        ("GET", "/api/v1/templates/foo"),
        ("PUT", "/api/v1/templates/foo"),
        ("DELETE", "/api/v1/templates/foo"),
        ("POST", "/api/v1/templates/foo/rename"),
        ("GET", "/api/v1/transcription-models"),
        ("POST", "/api/v1/transcription-models/base/install"),
        ("GET", "/api/v1/transcription-models/installations/job-1"),
        ("DELETE", "/api/v1/transcription-models/base"),
    ];

    for (method, path) in routes {
        let url = format!("{}{}", s.base_url(), path);
        let m = reqwest::Method::from_bytes(method.as_bytes()).unwrap();
        let resp = c.request(m, &url).send().expect("request");
        assert_eq!(
            resp.status(),
            reqwest::StatusCode::UNAUTHORIZED,
            "{method} {path} must be 401 without a bearer token"
        );
    }
}

// ── Transcription model management ─────────────────────────────────────────

#[test]
fn transcription_models_catalog_shape_is_stable() {
    let s = Sidecar::spawn();
    let c = client();
    let url = format!("{}/api/v1/transcription-models", s.base_url());

    let no_token = c.get(&url).send().expect("request");
    assert_eq!(no_token.status(), reqwest::StatusCode::UNAUTHORIZED);

    let body: serde_json::Value = c
        .get(&url)
        .bearer_auth(&s.handshake.token)
        .send()
        .expect("request")
        .json()
        .expect("json");

    assert_eq!(body["active_source"], "managed");
    assert!(body["models_dir"].as_str().unwrap().ends_with("models"));
    let models = body["models"].as_array().expect("models array");
    let ids: Vec<_> = models
        .iter()
        .map(|model| model["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        vec![
            "tiny",
            "base",
            "small",
            "medium",
            "large-v3",
            "large-v3-turbo-q5_0",
            "large-v3-turbo-q8_0"
        ]
    );
    let base = models
        .iter()
        .find(|model| model["id"] == "base")
        .expect("base model");
    assert_eq!(base["filename"], "ggml-base.bin");
    assert!(base["download_url"]
        .as_str()
        .unwrap()
        .contains("huggingface.co/ggerganov/whisper.cpp"));
    assert_eq!(base["checksum"].as_str().unwrap().len(), 40);
    assert_eq!(base["installed"], false);
    assert_eq!(base["active"], false);
}

#[test]
fn transcription_model_install_job_exposes_progress_and_terminal_error() {
    let s = Sidecar::spawn();
    let c = client();
    let base = format!("{}/api/v1/transcription-models", s.base_url());
    let settings_url = format!("{}/api/v1/settings", s.base_url());

    // Point models_dir at an existing regular file so the background job fails
    // before any network download. The contract still pins the in-memory job
    // lifecycle shape without pulling a multi-hundred-MB model in tests.
    let models_dir_file = tempfile::NamedTempFile::new().expect("models dir file");
    let mut settings: serde_json::Value = c
        .get(&settings_url)
        .bearer_auth(&s.handshake.token)
        .send()
        .expect("get settings")
        .json()
        .expect("json");
    settings["paths"]["models_dir"] = serde_json::json!(models_dir_file.path().to_str().unwrap());
    let put_settings = c
        .put(&settings_url)
        .bearer_auth(&s.handshake.token)
        .json(&settings)
        .send()
        .expect("put settings");
    assert_eq!(put_settings.status(), reqwest::StatusCode::OK);

    let start: serde_json::Value = c
        .post(format!("{base}/base/install"))
        .bearer_auth(&s.handshake.token)
        .send()
        .expect("start install")
        .json()
        .expect("json");
    let job_id = start["job_id"].as_str().expect("job id");
    assert!(!job_id.is_empty());

    let mut last: Option<serde_json::Value> = None;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let status: serde_json::Value = c
            .get(format!("{base}/installations/{job_id}"))
            .bearer_auth(&s.handshake.token)
            .send()
            .expect("poll install")
            .json()
            .expect("json");
        if status["status"] == "failed" || status["status"] == "done" {
            last = Some(status);
            break;
        }
        last = Some(status);
        std::thread::sleep(Duration::from_millis(100));
    }

    let status = last.expect("at least one install status");
    assert_eq!(status["job_id"], job_id);
    assert_eq!(status["model_id"], "base");
    assert!(matches!(
        status["status"].as_str().unwrap(),
        "queued" | "downloading" | "failed" | "done"
    ));
    assert!(status.get("bytes_downloaded").is_some());
    assert!(
        status["status"] == "failed" || status["status"] == "done",
        "install job should reach a terminal state in the isolated test env: {status}"
    );
}

// ── Settings: GET/PUT round-trip + secret has_key ────────────────────────────

#[test]
fn settings_get_put_roundtrip_and_secret_flag() {
    let s = Sidecar::spawn();
    let c = client();
    let url = format!("{}/api/v1/settings", s.base_url());
    let tok = &s.handshake.token;

    // GET: sanitized snapshot with the default shape.
    let snap: serde_json::Value = c
        .get(&url)
        .bearer_auth(tok)
        .send()
        .expect("get settings")
        .json()
        .expect("json");
    assert_eq!(snap["transcriber"]["language"], "auto", "default language");
    assert_eq!(snap["transcriber"]["model_source"], "managed");
    assert_eq!(snap["transcriber"]["model_id"], serde_json::Value::Null);
    assert_eq!(
        snap["transcriber"]["custom_model_path"],
        serde_json::Value::Null
    );
    assert_eq!(
        snap["transcriber"]["custom_models"]
            .as_array()
            .expect("custom models array")
            .len(),
        0
    );
    assert!(snap["paths"].get("models_dir").is_some());
    assert_eq!(snap["llm"]["active"], "anthropic", "default provider");
    // No key configured yet (ANTHROPIC_API_KEY is removed in the test env).
    assert_eq!(snap["llm"]["anthropic"]["has_key"], false);
    // Keyring is force-disabled in tests, so secrets sit in the plaintext
    // fallback and the snapshot discloses the file backend + its path.
    assert_eq!(snap["secret_storage"]["kind"], "plaintext");
    assert!(snap["secret_storage"]["path"].is_string());
    // Secret values must never be present in the snapshot.
    assert!(snap["llm"]["anthropic"].get("api_key").is_none());

    // PUT a full settings object with a changed language; expect it echoed back.
    let mut updated = snap.clone();
    updated["transcriber"]["language"] = serde_json::json!("en");
    updated["paths"]["models_dir"] = serde_json::json!("/tmp/ma-models");
    updated["transcriber"]["model_source"] = serde_json::json!("custom_path");
    updated["transcriber"]["custom_model_path"] = serde_json::json!("/tmp/ggml-custom.bin");
    updated["transcriber"]["custom_models"] = serde_json::json!([
        {
            "id": "custom-test",
            "name": "Custom Test",
            "path": "/tmp/ggml-custom.bin",
            "description": "External test model"
        }
    ]);
    let put_resp = c
        .put(&url)
        .bearer_auth(tok)
        .json(&updated)
        .send()
        .expect("put settings");
    assert_eq!(put_resp.status(), reqwest::StatusCode::OK);
    let echoed: serde_json::Value = put_resp.json().expect("json");
    assert_eq!(echoed["transcriber"]["language"], "en");
    assert_eq!(echoed["paths"]["models_dir"], "/tmp/ma-models");
    assert_eq!(echoed["transcriber"]["model_source"], "custom_path");
    assert_eq!(
        echoed["transcriber"]["custom_model_path"],
        "/tmp/ggml-custom.bin"
    );
    assert_eq!(
        echoed["transcriber"]["custom_models"][0]["name"],
        "Custom Test"
    );

    // GET again: the change persisted.
    let after: serde_json::Value = c
        .get(&url)
        .bearer_auth(tok)
        .send()
        .expect("get settings")
        .json()
        .expect("json");
    assert_eq!(after["transcriber"]["language"], "en");
    assert_eq!(after["paths"]["models_dir"], "/tmp/ma-models");
    assert_eq!(after["transcriber"]["model_source"], "custom_path");
    assert_eq!(
        after["transcriber"]["custom_models"][0]["id"],
        "custom-test"
    );

    // Store an OpenAI key; the snapshot's has_key flips without leaking the key.
    let secret_resp = c
        .put(format!("{url}/secret"))
        .bearer_auth(tok)
        .json(&serde_json::json!({"provider": "openai", "value": "sk-test"}))
        .send()
        .expect("put secret");
    assert_eq!(secret_resp.status(), reqwest::StatusCode::NO_CONTENT);

    let after_secret: serde_json::Value = c
        .get(&url)
        .bearer_auth(tok)
        .send()
        .expect("get settings")
        .json()
        .expect("json");
    assert_eq!(after_secret["llm"]["openai"]["has_key"], true);
}

// ── Templates: create / read / list / delete lifecycle ──────────────────────

#[test]
fn templates_crud_lifecycle_and_traversal_rejected() {
    let s = Sidecar::spawn();
    let c = client();
    let tok = &s.handshake.token;
    let base = format!("{}/api/v1/templates", s.base_url());

    // Point the prompts dir at an isolated tempdir so the test never writes
    // into the repo's real `/prompts/`. The FileTemplateLoader hot-swaps its
    // dir on a settings PUT, and the template service shares that handle.
    let prompts = tempfile::tempdir().expect("prompts tempdir");
    let settings_url = format!("{}/api/v1/settings", s.base_url());
    let mut snap: serde_json::Value = c
        .get(&settings_url)
        .bearer_auth(tok)
        .send()
        .expect("get settings")
        .json()
        .expect("json");
    snap["paths"]["prompts"] = serde_json::json!(prompts.path().to_str().unwrap());
    let put_settings = c
        .put(&settings_url)
        .bearer_auth(tok)
        .json(&snap)
        .send()
        .expect("put settings");
    assert_eq!(put_settings.status(), reqwest::StatusCode::OK);

    // Create a template with a Cyrillic name (must pass the validator).
    let name = "Тест-шаблон";
    let put = c
        .put(format!("{base}/{name}"))
        .bearer_auth(tok)
        .json(&serde_json::json!({ "body": "Сделай протокол.\n{transcript}" }))
        .send()
        .expect("put template");
    assert_eq!(put.status(), reqwest::StatusCode::NO_CONTENT);

    // Read it back.
    let got: serde_json::Value = c
        .get(format!("{base}/{name}"))
        .bearer_auth(tok)
        .send()
        .expect("get template")
        .json()
        .expect("json");
    assert_eq!(got["name"], name);
    assert_eq!(got["body"], "Сделай протокол.\n{transcript}");

    // It shows up in the list with its body.
    let list: serde_json::Value = c
        .get(&base)
        .bearer_auth(tok)
        .send()
        .expect("list templates")
        .json()
        .expect("json");
    let found = list["templates"]
        .as_array()
        .expect("templates array")
        .iter()
        .any(|t| t["name"] == name && t["body"] == "Сделай протокол.\n{transcript}");
    assert!(found, "created template must appear in the list: {list}");

    // Path traversal is rejected before touching disk (400).
    let evil = c
        .put(format!("{base}/{}", "..%2f..%2fevil"))
        .bearer_auth(tok)
        .json(&serde_json::json!({ "body": "x" }))
        .send()
        .expect("put evil");
    assert_eq!(
        evil.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "a traversal name must be rejected with 400"
    );

    // Delete it; no warning since it is not the default template.
    let del = c
        .delete(format!("{base}/{name}"))
        .bearer_auth(tok)
        .send()
        .expect("delete template");
    assert_eq!(del.status(), reqwest::StatusCode::OK);
    let del_body: serde_json::Value = del.json().expect("json");
    assert_eq!(del_body["deleted"], name);
    assert!(
        del_body["warning"].is_null() && del_body["warning_code"].is_null(),
        "no warning expected: {del_body}"
    );

    // Reading it now is a 404.
    let after = c
        .get(format!("{base}/{name}"))
        .bearer_auth(tok)
        .send()
        .expect("get deleted");
    assert_eq!(after.status(), reqwest::StatusCode::NOT_FOUND);
}

/// Names of the templates a sidecar spawned with `args` lists, sorted.
fn seeded_template_names(args: &[&str]) -> Vec<String> {
    let data = tempfile::tempdir().expect("tempdir");
    let data_path = data.path().to_path_buf();
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let s = Sidecar::spawn_in(&data_path, &args, data);
    let list: serde_json::Value = client()
        .get(format!("{}/api/v1/templates", s.base_url()))
        .bearer_auth(&s.handshake.token)
        .send()
        .expect("list templates")
        .json()
        .expect("json");
    let mut names: Vec<String> = list["templates"]
        .as_array()
        .expect("templates array")
        .iter()
        .map(|t| t["name"].as_str().expect("name").to_string())
        .collect();
    names.sort_unstable();
    names
}

#[test]
fn english_launch_seeds_only_english_templates() {
    assert_eq!(
        seeded_template_names(&["--language", "en"]),
        ["1-on-1", "Daily stand-up", "Simple minutes", "Team meeting"]
    );
}

#[test]
fn launch_without_a_language_keeps_seeding_russian_templates() {
    let russian = ["1-на-1", "Дейлик", "Командная встреча", "Простой протокол"];
    assert_eq!(seeded_template_names(&[]), russian);
    // An unknown code falls back the same way instead of stopping the sidecar.
    assert_eq!(seeded_template_names(&["--language", "de"]), russian);
}

#[test]
fn deleting_the_default_template_returns_a_localizable_notice_code() {
    let s = Sidecar::spawn();
    let c = client();
    let tok = &s.handshake.token;
    let base = format!("{}/api/v1/templates", s.base_url());
    let settings_url = format!("{}/api/v1/settings", s.base_url());

    let mut snap: serde_json::Value = c
        .get(&settings_url)
        .bearer_auth(tok)
        .send()
        .expect("get settings")
        .json()
        .expect("json");
    snap["default_template"] = serde_json::json!("Дейлик");
    let put = c
        .put(&settings_url)
        .bearer_auth(tok)
        .json(&snap)
        .send()
        .expect("put settings");
    assert_eq!(put.status(), reqwest::StatusCode::OK);

    let body: serde_json::Value = c
        .delete(format!("{base}/Дейлик"))
        .bearer_auth(tok)
        .send()
        .expect("delete template")
        .json()
        .expect("json");

    assert_eq!(body["deleted"], "Дейлик");
    assert_eq!(body["warning_code"], "default_template_reset");
    assert!(body["warning_detail"].is_null(), "{body}");
    // `warning` stays readable for clients that predate the code, which is why
    // the code did not need a protocol bump.
    assert!(
        body["warning"].as_str().is_some_and(|w| w.is_ascii()),
        "{body}"
    );
}

// ── Loopback-only bind ───────────────────────────────────────────────────────

#[test]
fn binds_loopback_only_and_health_needs_no_auth() {
    let s = Sidecar::spawn();
    let c = client();

    // Reachable on 127.0.0.1 (the server hard-asserts a loopback bind before
    // it ever prints the handshake — a non-loopback bind aborts startup, so
    // reaching it here at all proves the bind is loopback).
    let health = c
        .get(format!("{}/health", s.base_url()))
        .send()
        .expect("request");
    assert_eq!(health.status(), reqwest::StatusCode::OK);
    let body: serde_json::Value = health.json().expect("json");
    assert_eq!(body["status"], "ok");
}

// ── /version + the protocol-range gate predicate ─────────────────────────────

#[test]
fn version_endpoint_carries_build_and_protocol_range() {
    let s = Sidecar::spawn();
    let c = client();

    let resp = c
        .get(format!("{}/version", s.base_url()))
        .send()
        .expect("request");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let v: serde_json::Value = resp.json().expect("json");
    assert_eq!(v["build"], s.handshake.build);
    assert_eq!(v["protocol"], s.handshake.protocol);
    assert_eq!(v["min_protocol"], s.handshake.min_protocol);
}

/// The compatibility decision the Qt client makes against the handshake range
/// (`SidecarManager::versionGate`): proceed iff `min <= client <= max`. Pinned
/// here so a wire-semantics change is caught on the Rust side too.
fn protocol_in_range(client: u32, min: u32, max: u32) -> bool {
    client >= min && client <= max
}

#[test]
fn protocol_range_predicate_handles_in_range_below_min_and_above_max() {
    // in range (inclusive endpoints)
    assert!(protocol_in_range(2, 1, 3));
    assert!(protocol_in_range(1, 1, 3), "min is inclusive");
    assert!(protocol_in_range(3, 1, 3), "max is inclusive");
    // below min → incompatible
    assert!(!protocol_in_range(1, 2, 3));
    // above max → incompatible
    assert!(!protocol_in_range(4, 1, 3));
}

// ── Parent-death reaping ─────────────────────────────────────────────────────

#[test]
fn parent_death_via_parent_pid_poll_exits_within_budget() {
    // Spawn a sacrificial process; hand its PID to the sidecar as the parent
    // to poll (~1 Hz). Killing it must drive the sidecar to exit. Portable —
    // exercises the cross-platform `--parent-pid` fallback (the only
    // parent-death mechanism on Windows besides the GUI-side Job Object).
    let mut dummy = spawn_sleeper();
    let dummy_pid = dummy.id();

    let data = tempfile::tempdir().unwrap();
    let data_path = data.path().to_path_buf();
    let mut s = Sidecar::spawn_in(
        &data_path,
        &["--parent-pid".into(), dummy_pid.to_string()],
        data,
    );

    dummy.kill().expect("kill dummy parent");
    dummy.wait().ok();

    let status = s.wait_for_exit(REAP_BUDGET).unwrap_or_else(|| {
        panic!("sidecar did not exit within {REAP_BUDGET:?} after parent death")
    });
    // Parent-death is a clean shutdown path (exit 0).
    assert_eq!(status.code(), Some(0), "parent-death exit must be graceful");
}

#[cfg(unix)]
#[test]
fn parent_death_via_pipe_eof_exits_within_budget() {
    use std::os::unix::io::RawFd;
    use std::os::unix::process::CommandExt;

    // The preferred POSIX mechanism: a pipe whose write end the "parent"
    // holds. When that end closes, the child's read hits EOF (race-free).
    let mut fds: [libc::c_int; 2] = [0; 2];
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0, "pipe()");
    let (read_fd, write_fd): (RawFd, RawFd) = (fds[0], fds[1]);

    // Only the "parent" (this test) may hold the write end — mirror
    // SidecarManager: CLOEXEC it so the forked child does NOT inherit a copy
    // (otherwise its own write end keeps the pipe open and EOF never fires).
    unsafe { libc::fcntl(write_fd, libc::F_SETFD, libc::FD_CLOEXEC) };

    let data = tempfile::tempdir().unwrap();
    let mut cmd = isolated_server(data.path());
    cmd.arg("--parent-pipe-fd").arg(read_fd.to_string());
    unsafe {
        // Pre-exec, in the forked child: clear CLOEXEC so the read end
        // survives exec into the server. Async-signal-safe (fcntl only).
        cmd.pre_exec(move || {
            libc::fcntl(read_fd, libc::F_SETFD, 0);
            Ok(())
        });
    }
    let mut child = cmd.spawn().expect("spawn");

    // The test (the "parent") keeps only the write end; the child holds read.
    unsafe { libc::close(read_fd) };

    // Drain stderr off-thread for diagnostics.
    let err_buf = Arc::new(Mutex::new(Vec::new()));
    {
        let mut err = child.stderr.take().unwrap();
        let sink = Arc::clone(&err_buf);
        std::thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            while let Ok(n) = err.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                sink.lock().unwrap().extend_from_slice(&chunk[..n]);
            }
        });
    }

    // Wait for the handshake so we know it's fully up and watching the pipe.
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    out.read_line(&mut line).expect("handshake");
    assert!(line.contains("\"ready\":true"), "handshake: {line}");

    // "Parent dies": close the write end → child reads EOF → graceful exit.
    unsafe { libc::close(write_fd) };

    let deadline = Instant::now() + REAP_BUDGET;
    let status = loop {
        match child.try_wait().unwrap() {
            Some(st) => break st,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                panic!(
                    "sidecar did not exit within {REAP_BUDGET:?} after pipe EOF\nstderr:\n{}",
                    String::from_utf8_lossy(&err_buf.lock().unwrap())
                );
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    assert_eq!(status.code(), Some(0), "pipe-EOF exit must be graceful");
}

// ── Single-instance ──────────────────────────────────────────────────────────

#[test]
fn second_instance_exits_with_distinct_singleton_code() {
    // Both instances must share ONE data dir so they contend for the same
    // flock. `shared` owns it for the whole test; the Sidecar gets a separate
    // throwaway guard so dropping it cannot delete the shared dir early.
    let shared = tempfile::tempdir().unwrap();
    let shared_path = shared.path().to_path_buf();

    // First instance: acquires the lock, prints its handshake, keeps running.
    let mut first = Sidecar::spawn_in(&shared_path, &[], tempfile::tempdir().unwrap());

    // Second instance against the SAME data dir: must lose the lock and exit
    // with the distinct code before it ever serves.
    let mut second = isolated_server(&shared_path).spawn().expect("spawn second");

    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        match second.try_wait().unwrap() {
            Some(st) => break st,
            None if Instant::now() >= deadline => {
                let _ = second.kill();
                panic!("second instance never exited (singleton not enforced)");
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    assert_eq!(
        status.code(),
        Some(EXIT_SINGLETON_BUSY),
        "second instance must exit with the distinct singleton-busy code"
    );

    // The first instance must be unaffected — still serving.
    assert!(
        first.child.try_wait().unwrap().is_none(),
        "the original instance must survive a contended second launch"
    );
    drop(shared);
}

// ── Graceful SIGTERM ─────────────────────────────────────────────────────────

#[cfg(unix)]
#[test]
fn sigterm_triggers_graceful_shutdown_exit_zero() {
    let mut s = Sidecar::spawn();
    unsafe { libc::kill(s.pid() as libc::pid_t, libc::SIGTERM) };
    let status = s
        .wait_for_exit(REAP_BUDGET)
        .expect("sidecar must exit after SIGTERM");
    assert_eq!(
        status.code(),
        Some(0),
        "SIGTERM must drive a graceful exit(0)"
    );

    // Idle workers have nothing to drain. They used to sit out their poll
    // interval, which is longer than the drain budget, and get reported as stuck.
    let log = s.final_log();
    assert!(
        !log.contains("workers did not stop"),
        "idle workers must stop at once:\n{log}"
    );
}

// While a Whisper context is alive its Metal buffers stay registered with ggml's
// device, whose destructor aborts if any are left (macOS 15+ on Apple Silicon).
// That destructor used to run at exit, so a normal quit could die with SIGABRT
// and leave a crash report behind.
#[cfg(unix)]
#[test]
fn sigterm_right_after_a_transcription_exits_zero() {
    let files = tempfile::tempdir().unwrap();
    let mut s = Sidecar::spawn();
    let audio = files.path().join("silence.wav");
    std::fs::write(&audio, one_second_wav()).expect("write audio");
    let job = start_transcription(&s, files.path(), &audio);
    wait_for_job(&s, &job, |state| state["status"] == "done");

    unsafe { libc::kill(s.pid() as libc::pid_t, libc::SIGTERM) };
    let status = s
        .wait_for_exit(REAP_BUDGET)
        .expect("sidecar must exit after SIGTERM");
    assert_eq!(
        status.code(),
        Some(0),
        "a quit with a loaded Whisper context must exit(0):\n{}",
        s.final_log()
    );
}

// A running transcription can't be interrupted, so its thread still holds the
// Whisper context when the drain budget runs out and nothing can free it first.
// Audio from a FIFO with no writer keeps the job in that state for the whole test.
#[cfg(unix)]
#[test]
fn sigterm_during_a_transcription_exits_zero() {
    let files = tempfile::tempdir().unwrap();
    let mut s = Sidecar::spawn();
    let audio = files.path().join("never-ends.wav");
    let mkfifo = Command::new("mkfifo")
        .arg(&audio)
        .status()
        .expect("run mkfifo");
    assert!(mkfifo.success(), "mkfifo: {mkfifo}");
    let job = start_transcription(&s, files.path(), &audio);
    wait_for_job(&s, &job, |state| {
        state["progress"]["stage"] == "decoding_audio"
    });

    unsafe { libc::kill(s.pid() as libc::pid_t, libc::SIGTERM) };
    let status = s
        .wait_for_exit(REAP_BUDGET)
        .expect("sidecar must exit after SIGTERM");
    let log = s.final_log();
    assert!(
        log.contains("workers did not stop"),
        "the job must still be running at exit:\n{log}"
    );
    assert_eq!(
        status.code(),
        Some(0),
        "a quit during a transcription must exit(0):\n{log}"
    );
}

/// Selects a model file that needs no download and submits a transcription of
/// `audio`. Returns the URL of the job.
#[cfg(unix)]
fn start_transcription(s: &Sidecar, files: &Path, audio: &Path) -> String {
    let model = files.join("ggml-empty.bin");
    std::fs::write(&model, empty_whisper_model()).expect("write model");

    let c = client();
    let tok = &s.handshake.token;
    let settings_url = format!("{}/api/v1/settings", s.base_url());
    let mut settings: serde_json::Value = c
        .get(&settings_url)
        .bearer_auth(tok)
        .send()
        .expect("get settings")
        .json()
        .expect("settings json");
    settings["transcriber"]["model_source"] = serde_json::json!("custom_path");
    settings["transcriber"]["custom_model_path"] = serde_json::json!(model);
    settings["transcriber"]["custom_models"] =
        serde_json::json!([{ "id": "empty", "name": "Empty", "path": model }]);
    let put = c
        .put(&settings_url)
        .bearer_auth(tok)
        .json(&settings)
        .send()
        .expect("put settings");
    assert!(put.status().is_success(), "put settings: {}", put.status());

    let job: serde_json::Value = c
        .post(format!("{}/api/v1/jobs", s.base_url()))
        .bearer_auth(tok)
        .json(&serde_json::json!({ "audio_path": audio, "name": "synthetic" }))
        .send()
        .expect("submit job")
        .json()
        .expect("job json");
    format!(
        "{}/api/v1/jobs/{}",
        s.base_url(),
        job["id"].as_str().expect("job id")
    )
}

/// Polls the job until `reached` accepts its state.
#[cfg(unix)]
fn wait_for_job(s: &Sidecar, job_url: &str, reached: impl Fn(&serde_json::Value) -> bool) {
    let c = client();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let state: serde_json::Value = c
            .get(job_url)
            .bearer_auth(&s.handshake.token)
            .send()
            .expect("get job")
            .json()
            .expect("job json");
        if reached(&state) {
            return;
        }
        assert!(
            state["status"] != "failed" && Instant::now() < deadline,
            "job did not get there: {state}\nstderr:\n{}",
            s.stderr_string()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A valid Whisper model file with no tensors in it. whisper.cpp loads this as an
/// "empty model for testing": it allocates the buffers on the real backend (Metal
/// on macOS) and ends decoding at once, so a job runs in moments and no model has
/// to be downloaded. The sizes are the smallest that load.
#[cfg(unix)]
fn empty_whisper_model() -> Vec<u8> {
    const N_MELS: i32 = 80;
    const N_FFT: i32 = 201;

    let mut model = Vec::new();
    model.extend_from_slice(&0x6767_6d6c_u32.to_le_bytes()); // magic: "ggml"
    let hparams: [i32; 11] = [
        51_864, // n_vocab: English-only, so there is no language to detect
        1_500,  // n_audio_ctx
        8,      // n_audio_state
        1,      // n_audio_head
        1,      // n_audio_layer
        448,    // n_text_ctx
        8,      // n_text_state
        1,      // n_text_head
        1,      // n_text_layer
        N_MELS, // n_mels
        0,      // ftype: all f32
    ];
    for value in hparams {
        model.extend_from_slice(&value.to_le_bytes());
    }
    model.extend_from_slice(&N_MELS.to_le_bytes());
    model.extend_from_slice(&N_FFT.to_le_bytes());
    // Mel filters, all zero.
    model.resize(model.len() + (N_MELS * N_FFT) as usize * 4, 0);
    // whisper.cpp fills in the vocab but looks up the blank token by its text.
    model.extend_from_slice(&1i32.to_le_bytes());
    model.extend_from_slice(&1u32.to_le_bytes());
    model.push(b' ');
    model
}

/// One second of silence as a finalized 16 kHz mono 16-bit WAV.
#[cfg(unix)]
fn one_second_wav() -> Vec<u8> {
    const DATA_LEN: u32 = 32_000;

    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + DATA_LEN).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&16_000u32.to_le_bytes());
    wav.extend_from_slice(&32_000u32.to_le_bytes()); // byte rate
    wav.extend_from_slice(&2u16.to_le_bytes()); // block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&DATA_LEN.to_le_bytes());
    wav.resize(wav.len() + DATA_LEN as usize, 0);
    wav
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// A portable, long-lived "parent" process to sacrifice in the parent-death
/// test. Sleeps far longer than the test; we kill it explicitly.
fn spawn_sleeper() -> Child {
    #[cfg(windows)]
    {
        Command::new("powershell")
            .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 120"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleeper")
    }
    #[cfg(not(windows))]
    {
        Command::new("sleep")
            .arg("120")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleeper")
    }
}
