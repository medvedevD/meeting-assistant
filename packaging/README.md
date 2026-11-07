# Packaging — Meeting Assistant (section-07)

Ships v1 as a **two-binary bundle** on macOS / Linux / Windows: the Qt Quick
GUI (`meeting-assistant-qt`, `MeetingAssistant` on macOS) plus the Rust
`meeting-server` sidecar, presented as one application.

| OS | Recipe | Artifact | Gate |
|---|---|---|---|
| macOS | [`macos/build-app.sh`](macos/build-app.sh) | `MeetingAssistant.app` + guided `.dmg`, ad-hoc deep-signed | self-hosted Homebrew Cask or `Open Anyway` |
| Linux | [`linux/build-appimage.sh`](linux/build-appimage.sh) | `MeetingAssistant-<arch>.AppImage` | none ($0) |
| Windows | [`windows/build-installer.ps1`](windows/build-installer.ps1) | `MeetingAssistant-Setup-<ver>.exe` (Inno Setup) | unsigned → 1-click SmartScreen |

Build commands (Qt 6.7+ required; auto-discovered, or set `CMAKE_PREFIX_PATH`):

```bash
./packaging/macos/build-app.sh          # → dist/macos/{MeetingAssistant.app,*.dmg}
./packaging/linux/build-appimage.sh     # → dist/linux/MeetingAssistant-<arch>.AppImage
pwsh packaging/windows/build-installer.ps1   # → dist/windows/MeetingAssistant-Setup-*.exe
```

## Qt version per OS

CI and releases do not use one Qt version everywhere. `qt-ci.yml` and `release.yml`
set it per OS and must agree.

| OS | Runner | Qt | What holds it there |
|---|---|---|---|
| macOS | `macos-26` | 6.11.1 | Needs 6.9+: older Qt links AGL, which the Xcode 26 SDK removed. 6.12 needs macOS 14.4, above the app's minimum of 13. Local development uses 6.11.1. |
| Linux | `ubuntu:20.04` container | 6.7.3 | Qt 6.10+ needs glibc 2.34, and its `moc` won't start on this container's 2.31. Raising the floor would drop Astra Linux and Debian 11. |
| Windows | `windows-latest` | 6.7.3 | Nothing yet: it stays where it passes until there is a reason to move it. |

Qt 6.9.3 is the only version all three could share. It would rework the Linux
FFmpeg staging, because its Linux build ships FFmpeg libraries where 6.7.3 ships
none, in exchange for an older Qt than the one in development.

Moving the macOS Qt to 6.12 is a product decision: it drops macOS 13.

## macOS minimum version

The app promises macOS 13 in `Info.plist.in` and in the Homebrew cask, because
ScreenCaptureKit needs it. A compile with no deployment target builds for the OS it
runs on instead, so `build-app.sh` and `qt-ci.yml` set `MACOSX_DEPLOYMENT_TARGET`
from `Info.plist.in`, and [`macos/check-bundle.sh`](macos/check-bundle.sh) fails the
build if any binary needs a newer macOS. This proves what the binaries declare, not
that they run: no hosted runner has macOS 13.

## CPU baseline

whisper.cpp compiles for one CPU level, chosen at build time. ggml's default,
`GGML_NATIVE=ON`, takes that level from the machine doing the build, so a release
follows whichever runner it lands on. v0.1.0-alpha.13 shows it: the Windows build got
AVX-512, which most PCs lack, and the Linux build from the same commit did not. Picking
code at run time (`GGML_CPU_ALL_VARIANTS`) needs shared libraries, and `whisper-rs-sys`
links statically, so it would take a fork of that crate.

The root `.cargo/config.toml` sets `GGML_NATIVE=OFF`, which gives:

| OS | Baseline | Runs on |
|---|---|---|
| Windows, Linux | AVX2, FMA, F16C, BMI2 | Intel Haswell (2013) and later, AMD Excavator (2015) and later |
| macOS | The M1's features, clang's default for arm64 macOS | Every Apple silicon chip |

Below the baseline, the sidecar dies with an illegal instruction when it transcribes.
Lowering the x86 baseline is a product decision: it slows transcription for everyone.
[`docs/INSTALL.md`](../docs/INSTALL.md) and the release notes in `release.yml` state the
requirement.

- **Changing a `GGML_*` value needs a new cache key.** `whisper-rs-sys` does not ask
  cargo to watch these variables, so cargo keeps the old objects. `rust-cache` falls
  back to an older entry when its hash misses, so only its `key` input drops them.
  Change the `cpu-baseline-avx2` part in `qt-ci.yml` and `release.yml`. A local tree
  needs `cargo clean -p whisper-rs-sys`, once after pulling the first pin too.
- **[`check-cpu-baseline.sh`](../.github/scripts/check-cpu-baseline.sh) reads the
  binary in both workflows.** It fails on `zmm` registers on x86-64, and on i8mm, SVE
  or SME in ggml on arm64. This proves what the binary asks for, not that it runs on an
  old CPU: no hosted runner has one. It also can't see a native build on a runner
  without AVX-512, which is why the pin lives in config rather than in this check.
- **A build tuned to your own machine** is still there for local work: run
  `cargo clean -p whisper-rs-sys`, then build with `GGML_NATIVE=ON` set. Don't package
  that build. Numbers from `rust/scripts/bench_transcribe.sh` match what users get only
  without it.

## Cross-cutting invariant: GUI locates its sibling sidecar

On every OS the CMake `install()` rules ([qt-app/CMakeLists.txt](../qt-app/CMakeLists.txt))
place `meeting-server` **in the same directory as the GUI executable**:

- macOS `MeetingAssistant.app/Contents/MacOS/{MeetingAssistant,meeting-server}`
- Linux `usr/bin/{meeting-assistant-qt,meeting-server}` (inside the AppImage)
- Windows `<InstallDir>\{meeting-assistant-qt.exe,meeting-server.exe}`

`SidecarManager::locateSidecar()` resolves `QCoreApplication::applicationDirPath()`,
so this layout is the contract. None of the three Qt deploy tools
(`macdeployqt` / `linuxdeploy-plugin-qt` / `windeployqt`) bundle the helper —
the CMake rule is what does, and the macOS spike validated it.

## Version pinning (both binaries, one revision)

Both binaries are built **from the same checkout in the same recipe run** and
carry the same version (`0.1.0`: `qt-app` `project(... VERSION)` /
`MACOSX_BUNDLE_*`; `meeting-server` `CARGO_PKG_VERSION`). They are never
shipped independently. The **runtime safety net** is the IPC protocol-version
handshake (sections 02/03): the sidecar reports `protocol` /
`min_protocol` / `build`; the GUI's compiled-in `kClientProtocol`
([generated](../qt-app/cmake/GenClientProtocol.cmake) from the Rust single
source of truth) must be in range or `SidecarManager` refuses to proceed
(`Incompatible`). Build skew is *caught at runtime*, not assumed away — which
also makes an auto-updater obligation explicit: it must replace **both**
binaries atomically.

## Qt LGPLv3 compliance (all 3 artifacts)

Qt 6 is used under **LGPLv3**. Compliance, identically in every artifact:

1. **Dynamic linking only.** `qt-app` links Qt shared libs/frameworks (Qt Quick,
   Quick Controls, Network, **Multimedia**); nothing Qt is static. The deploy
   tools ship the Qt runtime as replaceable shared objects (macOS
   `Contents/Frameworks`, AppImage bundled Qt, Windows `Qt6*.dll`), so the user
   can substitute a compatible Qt.
2. **Written offer + license text + version**, bundled inside each artifact
   (CMake install of [`LICENSES/`](LICENSES/)):
   - macOS `MeetingAssistant.app/Contents/Resources/licenses/`
   - Linux `usr/share/doc/meeting-assistant/licenses/`
   - Windows `<InstallDir>\licenses\`
   Contents: `Qt-LGPLv3-WRITTEN-OFFER.txt` (3-yr source offer + relink note),
   `Qt-LICENSE.txt`, `Qt-LICENSE.GPL3-EXCEPT.txt`, `Qt-VERSION.txt`,
   `FFmpeg-LGPLv2.1-NOTICE.txt`.
3. **The exact Qt version.** CMake generates `Qt-VERSION.txt` from
   `LICENSES/Qt-VERSION.txt.in` at configure time, because the version differs per OS.
   `check-bundle.sh` compares it with the bundled QtCore on macOS.

Static linking would force releasing the app's source — we do not static-link.

### Bundled fonts

The fonts are compiled into the GUI binary and are under the **SIL Open Font
License**, which requires the license to travel with them. CMake installs each
`qt-app/fonts/OFL-*.txt` into the same `licenses` folder as above. A new font needs
its license added to `MA_FONT_LICENSES` in `qt-app/CMakeLists.txt`.

### Qt Multimedia / FFmpeg backend

The in-card audio player uses **Qt Multimedia**, whose backend bundles
**FFmpeg** shared libraries (LGPLv2.1+, dynamically linked) plus the
`multimedia` backend plugin. Each recipe now force-bundles and **asserts** the
backend so a missing plugin fails the build instead of shipping a dead player:

- macOS — `macdeployqt` bundles it from the linked `Qt6::Multimedia` + QML
  import; `build-app.sh` asserts `libffmpegmediaplugin.dylib` is present before
  signing. `codesign-deep.sh`'s recursive pass already signs the plugin and the
  ffmpeg `*.dylib`.
- Linux — `EXTRA_QT_PLUGINS=multimedia` forces the plugin into the AppDir (a
  QML-import scan alone misses it); `build-appimage.sh` asserts the plugin and
  `libavcodec` landed.
- Windows — `windeployqt --qmldir` bundles it; `build-installer.ps1` asserts
  `ffmpegmediaplugin.dll` before Inno Setup packs `Stage\*`.

FFmpeg compliance mirrors Qt's (dynamic link + written offer + source URL) in
`FFmpeg-LGPLv2.1-NOTICE.txt`.

## Per-OS gate status & documented exits

- **macOS — Homebrew unsigned-cask sunset (~Sept 2026): TRACKED.** v1 is
  ad-hoc signed + un-notarized; the self-hosted cask removes quarantine in
  `postflight`. Homebrew is sunsetting unsigned casks around **2026-09-01**.
  Full risk, date, fallback (self-hosted tap) and exit (Apple Developer ID +
  notarization) → **[macos/HOMEBREW-SUNSET.md](macos/HOMEBREW-SUNSET.md)**.
  Early-spike result → **[macos/SPIKE-RESULTS.md](macos/SPIKE-RESULTS.md)**.
- **macOS — Screen-Recording TCC** uses ad-hoc identity, so the grant resets
  per update until a stable Developer ID lands (same exit as above; accepted,
  section-05). Required `NSScreenCaptureUsageDescription` is in the signed
  `Info.plist`.
- **Windows — unsigned/SmartScreen: documented.** One-click bypass; exit is an
  OV/EV cert → **[windows/SMARTSCREEN.md](windows/SMARTSCREEN.md)**.
- **Linux — no gate.** AppImage, $0, no signing. System audio via `parec`.

## Layout

```
packaging/
  README.md                     ← this file
  SMOKE-CHECKLIST.md            manual per-OS check before each release
  assets/                       app icons + macOS DMG background
  LICENSES/                     Qt LGPLv3 written offer + license text (bundled)
  macos/   Info.plist.in  entitlements.plist  codesign-deep.sh  build-app.sh
           check-bundle.sh  render-release-cask.sh  HOMEBREW-SUNSET.md  SPIKE-RESULTS.md
  linux/   build-appimage.sh  meeting-assistant.desktop
  windows/ build-installer.ps1  installer.iss  SMARTSCREEN.md
```
