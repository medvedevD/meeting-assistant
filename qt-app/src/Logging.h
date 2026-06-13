#pragma once

#include <QString>

// Minimal file logger for the packaged GUI. Installs a Qt message handler that
// tees every qDebug/qInfo/qWarning/qCritical line — which already includes the
// sidecar's stderr relayed by SidecarManager as "[meeting-server] …" — into a
// per-user log file, *in addition to* the default stderr stream.
//
// Why: a GUI launched from Finder / Explorer / a .desktop entry has no terminal,
// so stderr is discarded and a failed run is undebuggable. This file is the only
// place to read what happened on a tester/self-test machine. Path (mirrors the
// sidecar's data-dir family):
//   macOS    ~/Library/Application Support/meeting-assistant/logs/meeting-assistant.log
//   Linux    ~/.local/share/meeting-assistant/logs/meeting-assistant.log
//   Windows  %LOCALAPPDATA%\meeting-assistant\logs\meeting-assistant.log
namespace logging {

// Call once, after QGuiApplication's application/organization name are set
// (QStandardPaths reads them). The previous session's log is rotated to
// `<name>.log.prev` so a crash that forces a relaunch doesn't erase the log
// that explains it. Returns the resolved log-file path (empty on failure).
QString installFileLogger();

}  // namespace logging
