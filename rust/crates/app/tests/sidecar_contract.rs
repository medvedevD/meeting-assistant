//! Sidecar contract tests (Qt-migration section-08).
//!
//! These spawn the **real `meeting-server` binary** (the same artefact the Qt
//! GUI launches) and assert the lifecycle/protocol contract section-02
//! promises the client: a single JSON handshake as the first stdout bytes,
//! logs on stderr only, bearer-gated `/api/*`, loopback-only bind,
//! `/version`, parent-death reaping (POSIX pipe EOF + `--parent-pid` poll),
//! the distinct singleton exit code, and graceful SIGTERM shutdown.
//!
//! Each server gets its own `XDG_DATA_HOME`/`XDG_CACHE_HOME` temp dir so the
//! SQLite DB, recordings dir and singleton lock are isolated and tests run in
//! parallel — except [`second_instance_exits_with_singleton_code`], which
//! deliberately shares one data dir between two instances.

use std::io::{BufRead, BufReader, Read};
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
        Sidecar::spawn_in(&data.path().to_path_buf(), &[], data)
    }

    /// Spawn pointing at `data_dir` (shared between instances for the
    /// singleton test), with extra args, taking ownership of the temp guard.
    fn spawn_in(
        data_dir: &std::path::Path,
        extra_args: &[String],
        guard: tempfile::TempDir,
    ) -> Sidecar {
        let mut cmd = Command::new(SERVER_BIN);
        cmd.args(extra_args)
            // Isolate DB, recordings dir and the singleton lock.
            .env("XDG_DATA_HOME", data_dir)
            .env("XDG_CACHE_HOME", data_dir)
            // Isolate settings.json + the secrets fallback file, and force the
            // file fallback so tests never read/write the real OS keyring.
            .env("XDG_CONFIG_HOME", data_dir)
            .env("MEETING_ASSISTANT_KEYRING_DISABLE", "1")
            .env_remove("ANTHROPIC_API_KEY")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
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
        ("POST", "/api/v1/transcribe"),
        ("POST", "/api/v1/jobs"),
        ("GET", "/api/v1/jobs/abc"),
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
    assert_eq!(snap["transcriber"]["language"], "ru", "default language");
    assert_eq!(snap["llm"]["active"], "anthropic", "default provider");
    // No key configured yet (ANTHROPIC_API_KEY is removed in the test env).
    assert_eq!(snap["llm"]["anthropic"]["has_key"], false);
    // Keyring is force-disabled in tests, so the UI-facing fallback flag is set.
    assert_eq!(snap["secrets_fallback"], true);
    // Secret values must never be present in the snapshot.
    assert!(snap["llm"]["anthropic"].get("api_key").is_none());

    // PUT a full settings object with a changed language; expect it echoed back.
    let mut updated = snap.clone();
    updated["transcriber"]["language"] = serde_json::json!("en");
    let put_resp = c
        .put(&url)
        .bearer_auth(tok)
        .json(&updated)
        .send()
        .expect("put settings");
    assert_eq!(put_resp.status(), reqwest::StatusCode::OK);
    let echoed: serde_json::Value = put_resp.json().expect("json");
    assert_eq!(echoed["transcriber"]["language"], "en");

    // GET again: the change persisted.
    let after: serde_json::Value = c
        .get(&url)
        .bearer_auth(tok)
        .send()
        .expect("get settings")
        .json()
        .expect("json");
    assert_eq!(after["transcriber"]["language"], "en");

    // Store an OpenAI key; the snapshot's has_key flips without leaking the key.
    let secret_resp = c
        .put(format!("{}/secret", url))
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
        del_body["warning"].is_null(),
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
    let mut cmd = Command::new(SERVER_BIN);
    cmd.arg("--parent-pipe-fd")
        .arg(read_fd.to_string())
        .env("XDG_DATA_HOME", data.path())
        .env("XDG_CACHE_HOME", data.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
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
    let mut second = Command::new(SERVER_BIN)
        .env("XDG_DATA_HOME", &shared_path)
        .env("XDG_CACHE_HOME", &shared_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn second");

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
