#pragma once

#include <QFile>
#include <QString>
#include <QtTest>

// Each QML test binary bundles its own cut of the MeetingAssistant module, and
// `import MeetingAssistant` goes through two copies of its qmldir: the file the
// target generated under its import dir and, via that file's
// `prefer :/qt/qml/MeetingAssistant/` line, the copy embedded in the binary.
// Qt 6.7 takes the type list from the file on disk; Qt 6.8+ replaces it with
// the embedded copy. A wrong embedded copy therefore stays invisible on 6.7 and
// breaks imports on anything newer ("MeetyMenuItem is not a type").
//
// qt_add_qml_module() names that resource after the URI
// (.qt/rcc/qmake_MeetingAssistant.qrc), so two targets in one CMake directory
// share the file and both embed the last target's qmldir. Reading the embedded
// copy directly keeps this check independent of the Qt version's import logic.
inline void verifyEmbeddedQmldir(const QString &importDir) {
    QFile embedded(QStringLiteral(":/qt/qml/MeetingAssistant/qmldir"));
    QVERIFY2(embedded.open(QIODevice::ReadOnly),
             "the binary embeds no MeetingAssistant qmldir");

    QFile generated(importDir + QStringLiteral("/MeetingAssistant/qmldir"));
    QVERIFY2(generated.open(QIODevice::ReadOnly),
             qPrintable(generated.fileName() + QStringLiteral(": ")
                        + generated.errorString()));

    QVERIFY2(embedded.readAll() == generated.readAll(),
             qPrintable(QStringLiteral(
                            "the embedded qmldir is not the one this target "
                            "generated at %1: is another QML module target "
                            "sharing this CMake directory?")
                            .arg(generated.fileName())));
}
