//! TLS-backend regression guard.
//!
//! The sidecar must speak TLS through **rustls**, never through `native-tls`.
//! On Linux `native-tls` binds to the *system* OpenSSL, and the AppImage is
//! built in an `ubuntu:20.04` container (the glibc floor), so linuxdeploy
//! bundled that container's OpenSSL 1.1.1 beside the sidecar with
//! `RUNPATH=$ORIGIN/../lib`.
//!
//! Observed failure (Astra Linux, release AppImage): the host
//! `/etc/ssl/openssl.cnf` registers the GOST engine at
//! `dynamic_path = /usr/lib/x86_64-linux-gnu/engines-3/gost.so`, built against
//! OpenSSL **3**. The bundled OpenSSL **1.1.1** dlopen'd it, putting
//! `libcrypto.so.1.1` and `libcrypto.so.3` in one address space. Building the
//! first `reqwest::Client` loads the CA store, and freeing a GOST certificate's
//! key crossed the mismatched ABI:
//!
//! ```text
//! X509_STORE_load_locations → X509_load_cert_crl_file
//!   → X509_INFO_free → EVP_PKEY_free → ENGINE_finish → SIGSEGV
//! ```
//!
//! The sidecar died before emitting its handshake or a single log line, so the
//! GUI could only report `state=Core failed to start (core crashed)`.
//!
//! There is no way to reproduce that segfault in CI — it needs a GOST-configured
//! host — so the guard sits at the level that actually decides the outcome: the
//! dependency graph. If `reqwest`'s default features (or an explicit
//! `native-tls`) ever come back, OpenSSL re-enters the lockfile and this fails.

/// The resolved workspace dependency graph, embedded at compile time so a
/// lockfile change forces this test to rebuild.
const LOCKFILE: &str = include_str!("../../../Cargo.lock");

/// Exact `[[package]] name = ...` values that must never be resolved.
///
/// `openssl-probe` is deliberately absent: it only *locates* the system CA
/// bundle for `rustls-native-certs` and links no OpenSSL code.
const FORBIDDEN: &[&str] = &[
    "native-tls",
    "openssl",
    "openssl-sys",
    "hyper-tls",
    "tokio-native-tls",
];

/// Crates that prove the rustls path is the one actually wired up.
const REQUIRED: &[&str] = &["rustls", "rustls-native-certs"];

/// Collect every `name = "..."` value from the lockfile's `[[package]]` tables.
fn locked_packages() -> Vec<&'static str> {
    LOCKFILE
        .lines()
        .filter_map(|line| line.trim().strip_prefix("name = "))
        .map(|name| name.trim().trim_matches('"'))
        .collect()
}

#[test]
fn no_openssl_in_dependency_graph() {
    let packages = locked_packages();
    assert!(
        !packages.is_empty(),
        "parsed no packages from Cargo.lock — the lockfile format changed"
    );

    let found: Vec<&str> = FORBIDDEN
        .iter()
        .copied()
        .filter(|banned| packages.contains(banned))
        .collect();

    assert!(
        found.is_empty(),
        "OpenSSL/native-tls re-entered the dependency graph: {found:?}.\n\
         The sidecar must use rustls — linking the host OpenSSL crashes the \n\
         AppImage on hosts that register an OpenSSL 3 engine (see this file's \n\
         module docs). Check the `reqwest` features in rust/Cargo.toml: it \n\
         needs `default-features = false` plus `rustls-tls-native-roots`."
    );
}

#[test]
fn rustls_is_the_resolved_tls_backend() {
    let packages = locked_packages();

    let missing: Vec<&str> = REQUIRED
        .iter()
        .copied()
        .filter(|required| !packages.contains(required))
        .collect();

    assert!(
        missing.is_empty(),
        "expected rustls to provide TLS, but these are absent from Cargo.lock: \
         {missing:?}. Without them `reqwest` has no TLS backend at all."
    );
}
