#include "Logging.h"

#include <QByteArray>
#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QMutex>
#include <QStandardPaths>
#include <QtGlobal>

namespace {

QFile g_logFile;
QMutex g_logMutex;
QtMessageHandler g_prevHandler = nullptr;

const char *levelTag(QtMsgType type) {
    switch (type) {
    case QtDebugMsg:    return "DEBUG";
    case QtInfoMsg:     return "INFO";
    case QtWarningMsg:  return "WARN";
    case QtCriticalMsg: return "ERROR";
    case QtFatalMsg:    return "FATAL";
    }
    return "INFO";
}

void fileMessageHandler(QtMsgType type, const QMessageLogContext &ctx,
                        const QString &msg) {
    {
        QMutexLocker lock(&g_logMutex);
        if (g_logFile.isOpen()) {
            QByteArray line;
            line += QDateTime::currentDateTimeUtc()
                        .toString(Qt::ISODateWithMs)
                        .toUtf8();
            line += ' ';
            line += levelTag(type);
            line += ' ';
            line += msg.toUtf8();
            line += '\n';
            g_logFile.write(line);
            // Flush every line: a hard crash mid-run must still leave the last
            // message on disk — that line is usually the one that matters.
            g_logFile.flush();
        }
    }
    // Chain *after* writing so a QtFatalMsg is on disk before the default
    // handler aborts the process.
    if (g_prevHandler)
        g_prevHandler(type, ctx, msg);
}

}  // namespace

namespace logging {

QString installFileLogger() {
    const QString base =
        QStandardPaths::writableLocation(QStandardPaths::GenericDataLocation);
    if (base.isEmpty())
        return {};
    const QString dir = QDir(base).filePath(QStringLiteral("meeting-assistant/logs"));
    if (!QDir().mkpath(dir))
        return {};
    const QString path = QDir(dir).filePath(QStringLiteral("meeting-assistant.log"));

    // One-step rotation: keep the previous session as <name>.log.prev.
    if (QFile::exists(path)) {
        const QString prev = path + QStringLiteral(".prev");
        QFile::remove(prev);
        QFile::rename(path, prev);
    }

    g_logFile.setFileName(path);
    if (!g_logFile.open(QIODevice::WriteOnly | QIODevice::Truncate |
                        QIODevice::Text)) {
        qWarning("Logging: cannot open log file %s", qPrintable(path));
        return {};
    }

    g_prevHandler = qInstallMessageHandler(fileMessageHandler);
    qInfo("Logging to %s", qPrintable(path));
    return path;
}

}  // namespace logging
