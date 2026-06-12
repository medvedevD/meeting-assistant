#pragma once

#include <QObject>
#include <QString>
#include <QUrl>

// Tiny QML helper for turning an absolute local filesystem path (as stored in a
// meeting's `audio_path`) into a `file://` URL the QML MediaPlayer can load.
//
// Done in C++ rather than string-concatenated in QML because audio files carry
// spaces and non-ASCII (Cyrillic) names, and only QUrl::fromLocalFile applies
// the correct percent-encoding for those. Exposed as the `pathUtils` context
// property from main.cpp.
class PathUtils : public QObject {
    Q_OBJECT

public:
    explicit PathUtils(QObject *parent = nullptr) : QObject(parent) {}

    // Empty path → empty URL (the player binds to nothing and stays hidden).
    Q_INVOKABLE QUrl fileUrl(const QString &path) const {
        return path.isEmpty() ? QUrl() : QUrl::fromLocalFile(path);
    }
};
