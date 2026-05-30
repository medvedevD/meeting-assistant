# Section 03 — Qt/QML skeleton: Fusion, HTTP client, sidecar process mgmt

## Background
The GUI shell and the client half of the loopback-HTTP boundary. UI = Qt Quick
/ QML (owner decision), Fusion style baseline (Basic/Material/Universal
forbidden), plain C++ Qt + a **separate** Rust binary — **no cxx-qt /
qt-build-utils** (there is no FFI; the boundary is HTTP). Target **Qt 6.7+**
(`QRestAccessManager` available).

## Requirements
A launchable Qt app that: enforces Fusion; spawns and supervises the
`meeting-server` sidecar (handshake parse, health gate, clean kill, parent-death
linkage); enforces the protocol-version gate; provides an authenticated HTTP
client + job poller exposed to QML.

## Dependencies
- Requires: section-02 (handshake/contract).
- Blocks: section-04 (screens need the shell + ApiClient), section-07 (packaging
  needs the GUI binary).

## Implementation details
- **Project:** new top-level `qt-app/` (sibling of `ui-compose/`).
  `CMakeLists.txt`: `find_package(Qt6 6.7 REQUIRED COMPONENTS Quick Network)`,
  `qt_standard_project_setup`, `qt_add_executable`, `qt_add_qml_module`.
- **Fusion enforcement:** in `main.cpp`, after `QGuiApplication`, before loading
  the engine: `QQuickStyle::setStyle("Fusion")`. Also ship a compiled-in
  `qtquickcontrols2.conf` (`[Controls] Style=Fusion`). Do not link/ship the
  Material or Universal style plugins. Verify via `palette` inspection.
- **SidecarManager (C++ QObject):** locate the sibling `meeting-server` via
  `QCoreApplication::applicationDirPath()` (per-OS layout from section-07).
  `QProcess::start(path, {...})`, passing the inherited pipe / `--parent-pid`
  and (Windows) assigning a Job Object with kill-on-close. Parse the first
  stdout line → `{port,token,protocol,min_protocol,build}`. Poll `GET /health`
  (bounded: ~15 × 200 ms) before declaring ready. On quit:
  `terminate()`→wait→`kill()`→`waitForFinished()`. On unexpected child exit
  while running: emit a "core restarting" state, respawn (bounded restart
  budget), reset to ready.
- **Version gate (Q9):** compare the GUI's compiled `kClientProtocol` against
  `[min_protocol, protocol]`. Out of range → blocking dialog ("Components are
  incompatible — please update"), do not proceed. In range → proceed even if
  `build` differs. `kClientProtocol` is **generated from the Rust
  `PROTOCOL_VERSION` at build time** (small codegen writing a header) — never
  hand-maintained in two places.
- **ApiClient (C++ → QML):** wraps `QNetworkAccessManager`/`QRestAccessManager`;
  injects `Authorization: Bearer <token>`; base `http://127.0.0.1:<port>`; JSON
  via `QJsonDocument`; async methods + `requestSucceeded`/`requestFailed`
  signals to QML. `JobPoller` owns a `QTimer`, polls `GET /api/v1/jobs/:id`,
  emits `statusChanged`. Register via `qmlRegisterType`/context properties.
- **Dev entrypoint:** add a top-level `run-qt.sh` (successor to
  `run-compose.sh`): `cargo build` of `meeting-server` + CMake build of
  `qt-app/` + copy the sidecar next to the GUI for local dev. Document it as the
  canonical dev workflow.

## Acceptance criteria
- [ ] Launching `qt-app` spawns `meeting-server`, completes handshake + health
      gate, renders the QML root in Fusion (verified).
- [ ] Killing `meeting-server` externally → GUI shows "core restarting",
      respawns, recovers.
- [ ] Quitting the GUI leaves no orphan `meeting-server` (verified on macOS,
      Linux, Windows — Windows via Job Object).
- [ ] Simulated protocol-range mismatch → blocking dialog; build-only diff →
      proceeds.
- [ ] `kClientProtocol` is generated from Rust `PROTOCOL_VERSION` (not
      hand-written).
- [ ] `run-qt.sh` builds + runs the full local stack.

## Files to create/modify
- Create `qt-app/`: `CMakeLists.txt`, `src/main.cpp`,
  `src/SidecarManager.{h,cpp}`, `src/ApiClient.{h,cpp}`,
  `src/JobPoller.{h,cpp}`, `qml/Main.qml`, `resources.qrc`,
  `qtquickcontrols2.conf`, protocol-version codegen step.
- Create top-level `run-qt.sh`.
