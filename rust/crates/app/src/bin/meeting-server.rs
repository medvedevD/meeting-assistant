//! `meeting-server` — the Rust core run as a loopback-HTTP sidecar for the Qt
//! GUI (section-02 of the Qt migration).
//!
//! Lifecycle / protocol contract:
//! - binds `127.0.0.1:0` (OS-chosen ephemeral port); hard-refuses any
//!   non-loopback bind;
//! - emits exactly **one** machine-readable JSON handshake line on **stdout**
//!   (port + bearer token + protocol range + build), then nothing else ever
//!   touches stdout — all logs go to **stderr**;
//! - every `/api/*` route requires `Authorization: Bearer <token>`;
//!   `/health` and `/version` are unauthenticated;
//! - exits cleanly when its parent GUI dies (POSIX: inherited-pipe EOF —
//!   race-free — or `--parent-pid` `kill(pid,0)` poll; Windows: `--parent-pid`
//!   `OpenProcess` poll, with the GUI-side Job Object as the preferred
//!   kernel-enforced mechanism — see section-03);
//! - enforces single-instance via the same flock the FFI app uses, exiting
//!   with a distinct code on contention;
//! - shuts down gracefully on SIGTERM/SIGINT.
//!
//! The legacy `meeting-assistant serve` subcommand is left untouched.

// `new_desktop` (used only by the legacy CLI bin) is dead code in this bin.
#[allow(dead_code)]
#[path = "../container.rs"]
mod container;

#[path = "../settings_service.rs"]
mod settings_service;

use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::sync::Notify;
use tracing_subscriber::EnvFilter;

use container::{
    default_db_path, default_model_path, default_prompts_dir, default_recordings_dir, Container,
};

/// Clean shutdown (signal, parent death, or graceful stop).
const EXIT_OK: i32 = 0;
/// Another live instance already holds the singleton lock. Distinct so the GUI
/// can surface "already running" rather than a generic failure.
const EXIT_SINGLETON_BUSY: i32 = 3;

#[derive(Default)]
struct Args {
    /// Parent (GUI) PID to poll; exit when it disappears.
    parent_pid: Option<i32>,
    /// Inherited pipe read-end fd (POSIX). EOF on it == parent died (race-free).
    parent_pipe_fd: Option<i32>,
}

fn parse_args() -> Args {
    let mut args = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--parent-pid" => {
                args.parent_pid = it.next().and_then(|v| v.parse().ok());
            }
            "--parent-pipe-fd" => {
                args.parent_pipe_fd = it.next().and_then(|v| v.parse().ok());
            }
            other => {
                tracing::warn!("ignoring unrecognised argument: {other}");
            }
        }
    }
    args
}

fn main() -> Result<()> {
    // stdout discipline: the logger goes to STDERR ONLY, configured before any
    // other code can run. stdout is reserved for the single handshake line.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = parse_args();

    // Single-instance: same flock + path the FFI app uses, so a stray FFI app
    // and the sidecar can never both hold the core.
    if !try_acquire_singleton()? {
        tracing::error!("another instance is already running — exiting");
        std::process::exit(EXIT_SINGLETON_BUSY);
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(run(args))
}

async fn run(args: Args) -> Result<()> {
    // ── Adapter graph + worker ────────────────────────────────────────────────
    let model = default_model_path();
    let db = default_db_path();
    let prompts = default_prompts_dir();
    let recordings = default_recordings_dir();
    std::fs::create_dir_all(&recordings)
        .with_context(|| format!("cannot create recordings dir {recordings:?}"))?;

    let mut container = Container::new_sidecar(&model, &db, &prompts, recordings)
        .context("failed to wire sidecar adapter graph")?;

    // Take the runtime handles out before the adapter Arcs are moved into
    // AppState; build the settings service that powers the /settings routes.
    let handles = container
        .settings_handles
        .take()
        .expect("sidecar container must carry settings handles");
    let settings_service: std::sync::Arc<dyn meeting_api::SettingsService> =
        std::sync::Arc::new(settings_service::AppSettingsService::new(handles));

    // ── Crash-safe recording recovery (section-06) ────────────────────────────
    // A previous core may have been killed mid-recording, leaving an
    // unfinalized WAV (unplayable) and possibly no `meetings` row. Finalize
    // those WAVs and reconcile their rows before we serve, so recovered
    // recordings are listable/transcribable. Best-effort + idempotent: it
    // never aborts startup.
    meeting_adapters::run_recovery(&container.recordings_dir, container.meeting_repo.as_ref())
        .await;

    let (worker_join, worker_shutdown) = container.spawn_worker();
    let worker_abort = worker_join.abort_handle();

    // ── Loopback bind (strict) ────────────────────────────────────────────────
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context("failed to bind 127.0.0.1:0")?;
    let local = listener.local_addr()?;
    if local.ip() != IpAddr::V4(Ipv4Addr::LOCALHOST) {
        anyhow::bail!("refusing to serve on non-loopback address {local}");
    }
    let port = local.port();

    // ── Bearer token (256-bit, hex) ───────────────────────────────────────────
    let token = random_token_hex();

    let router = meeting_api::create_server_router(
        meeting_api::AppState {
            transcriber: container.transcriber,
            meeting_repo: container.meeting_repo,
            job_repo: container.job_repo,
            llm: container.llm,
            templates: container.templates,
            audio_capture: container.audio_capture,
            file_store: container.file_store,
            recordings_dir: container.recordings_dir,
        },
        settings_service,
        token.clone(),
        env!("CARGO_PKG_VERSION"),
    );

    // ── Install OS termination handlers BEFORE the handshake ────────────────
    // Once the handshake is on stdout the parent treats us as live and may
    // SIGTERM us at any instant. tokio installs the kernel handler when
    // `signal()` is called (not on first poll), so creating these *now*
    // replaces SIGTERM's default disposition (instant kill) before we ever
    // announce readiness. Polling the handler only inside the axum
    // graceful-shutdown future left a startup window where SIGTERM killed the
    // sidecar with a signal exit instead of our graceful exit(0) —
    // section-08's SIGTERM contract test caught exactly this race on Linux CI.
    #[cfg(unix)]
    let (mut sig_term, mut sig_int) = {
        use tokio::signal::unix::{signal, SignalKind};
        (
            signal(SignalKind::terminate()).context("install SIGTERM handler")?,
            signal(SignalKind::interrupt()).context("install SIGINT handler")?,
        )
    };

    // ── Handshake — MUST be the first bytes on stdout ─────────────────────────
    let handshake = serde_json::json!({
        "ready": true,
        "port": port,
        "token": token,
        "protocol": meeting_api::PROTOCOL_VERSION,
        "min_protocol": meeting_api::MIN_PROTOCOL_VERSION,
        "build": env!("CARGO_PKG_VERSION"),
    });
    {
        let mut out = std::io::stdout().lock();
        writeln!(out, "{}", serde_json::to_string(&handshake)?)?;
        out.flush()?;
    }
    tracing::info!(port, "sidecar listening on 127.0.0.1 (handshake sent)");

    // ── Shutdown triggers: OS signals + parent death ──────────────────────────
    let parent_gone = std::sync::Arc::new(Notify::new());
    spawn_parent_watchdog(&args, std::sync::Arc::clone(&parent_gone));

    let shutdown = async move {
        #[cfg(unix)]
        tokio::select! {
            _ = sig_term.recv() => tracing::info!("received SIGTERM"),
            _ = sig_int.recv()  => tracing::info!("received SIGINT"),
            _ = parent_gone.notified() => tracing::info!("parent process gone"),
        }
        #[cfg(not(unix))]
        tokio::select! {
            _ = tokio::signal::ctrl_c() => tracing::info!("received Ctrl-C"),
            _ = parent_gone.notified() => tracing::info!("parent process gone"),
        }
    };

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown)
        .await
        .context("http server error")?;

    // ── Drain the worker, then exit ───────────────────────────────────────────
    let _ = worker_shutdown.send(());
    if tokio::time::timeout(Duration::from_millis(1200), worker_join)
        .await
        .is_err()
    {
        tracing::warn!("worker did not stop within 1.2s — aborting");
        worker_abort.abort();
    }
    tracing::info!("graceful shutdown complete");
    std::process::exit(EXIT_OK);
}

/// Spawns the parent-death watchdog(s). On parent death any watchdog fires
/// `parent_gone`, which triggers the same graceful path as a signal.
///
/// Watchdogs use `Notify::notify_one`, **not** `notify_waiters`: the pipe-EOF
/// path can fire before `axum::serve` has polled the shutdown future and
/// registered as a waiter. `notify_waiters` stores no permit, so that wakeup
/// would be lost and the sidecar would never exit (it was — section-08's
/// pipe-EOF contract test caught exactly this race). `notify_one` stores a
/// permit when no waiter is registered yet, so the next `notified()` poll
/// completes immediately. There is exactly one consumer (the shutdown
/// `select!`), so a single permit is sufficient.
fn spawn_parent_watchdog(args: &Args, parent_gone: std::sync::Arc<Notify>) {
    // Preferred POSIX mechanism: inherited pipe. The GUI holds the write end and
    // never writes; when it dies the write end closes and our read hits EOF.
    // Race-free (no PID-reuse window).
    #[cfg(unix)]
    if let Some(fd) = args.parent_pipe_fd {
        let notify = std::sync::Arc::clone(&parent_gone);
        std::thread::spawn(move || {
            use std::io::Read;
            use std::os::unix::io::FromRawFd;
            // SAFETY: the GUI passes this fd as the read end of a dedicated pipe
            // it owns for this child; we take sole ownership of it here.
            let mut f = unsafe { std::fs::File::from_raw_fd(fd) };
            let mut buf = [0u8; 64];
            loop {
                match f.read(&mut buf) {
                    Ok(0) | Err(_) => break, // EOF (parent died) or error
                    Ok(_) => {}              // unexpected bytes — ignore, keep waiting
                }
            }
            notify.notify_one();
        });
    }

    // Fallback / non-pipe path: poll the parent PID ~1 Hz.
    if let Some(pid) = args.parent_pid {
        let notify = std::sync::Arc::clone(&parent_gone);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(1));
            loop {
                tick.tick().await;
                if !parent_alive(pid) {
                    notify.notify_one();
                    break;
                }
            }
        });
    }
}

#[cfg(unix)]
fn parent_alive(pid: i32) -> bool {
    // kill(pid, 0): 0 => alive; ESRCH => gone; EPERM => alive (exists, not ours).
    // Also treat reparenting to init (ppid == 1) as "parent gone".
    if unsafe { libc::getppid() } == 1 {
        return false;
    }
    let rc = unsafe { libc::kill(pid, 0) };
    if rc == 0 {
        return true;
    }
    !matches!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(e) if e == libc::ESRCH
    )
}

#[cfg(windows)]
fn parent_alive(pid: i32) -> bool {
    // Companion to the GUI-side Job Object (section-03, the preferred
    // kernel-enforced mechanism); this poll is the documented fallback.
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
    };
    unsafe {
        let h = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid as u32);
        // windows-sys' `HANDLE` is `isize` in some resolutions and
        // `*mut c_void` in others — `.is_null()` only compiles on the latter.
        // Cast through `isize` so both shapes work; a NULL handle is 0 either
        // way, which is what `OpenProcess` returns on failure.
        if (h as isize) == 0 {
            return false; // process no longer exists
        }
        let signaled = WaitForSingleObject(h, 0) == WAIT_OBJECT_0;
        CloseHandle(h);
        !signaled
    }
}

#[cfg(not(any(unix, windows)))]
fn parent_alive(_pid: i32) -> bool {
    true
}

/// 256-bit random token, lowercase hex. Sourced from the OS CSPRNG.
fn random_token_hex() -> String {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).expect("OS CSPRNG unavailable");
    let mut s = String::with_capacity(64);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Single-instance lock at `$XDG_DATA_HOME/meeting-assistant/meeting-assistant.lock`.
/// Returns `Ok(false)` if another live instance holds it. The OS releases the
/// flock on process exit, even on SIGKILL, so stale lock files are never a problem.
fn try_acquire_singleton() -> Result<bool> {
    use fs2::FileExt;
    use std::sync::OnceLock;

    static LOCK: OnceLock<std::fs::File> = OnceLock::new();
    if LOCK.get().is_some() {
        return Ok(true);
    }

    let lock_path = xdg_data_dir().join("meeting-assistant/meeting-assistant.lock");
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| format!("cannot open lock file {lock_path:?}"))?;

    match file.try_lock_exclusive() {
        Ok(()) => {
            let _ = LOCK.set(file);
            Ok(true)
        }
        Err(_) => Ok(false),
    }
}

fn xdg_data_dir() -> std::path::PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").expect("$HOME is not set");
            std::path::PathBuf::from(home).join(".local/share")
        })
}
