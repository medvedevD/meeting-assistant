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

Static linking would force releasing the app's source — we do not static-link.

### Qt Multimedia / FFmpeg backend

The in-card audio player uses **Qt Multimedia**, whose Qt 6.7 backend bundles
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
  assets/                       app icons + macOS DMG background
  LICENSES/                     Qt LGPLv3 written offer + license text (bundled)
  macos/   Info.plist.in  entitlements.plist  codesign-deep.sh  build-app.sh
           render-release-cask.sh  HOMEBREW-SUNSET.md  SPIKE-RESULTS.md
  linux/   build-appimage.sh  meeting-assistant.desktop
  windows/ build-installer.ps1  installer.iss  SMARTSCREEN.md
```
