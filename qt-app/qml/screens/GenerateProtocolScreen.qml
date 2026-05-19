// ProtocolGenerateViewModel reimplemented client-side (audit decision ①):
// the Compose async job-poll path can't feed POST /api/v1/protocols (no
// transcript-readback route), so transcription uses the SYNC
// POST /api/v1/transcribe (returns text + persists), then /protocols.
// Observable behavior preserved: transcribe → generate, progress, errors,
// retry. State machine maps 1:1 to GenerateState.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtCore

Page {
    id: scr
    property var shell
    property var store
    property string meetingId
    property string meetingName
    property string audioPath
    property bool hasTranscript: false
    property bool autoStart: false

    // "idle"|"transcribing"|"transcriptFailed"|"generating"|"done"|"failed"
    property string st: "idle"
    property string errorMsg: ""
    property string transcript: ""

    Settings {
        id: prefs
        category: "protocol"
        property string defaultTemplate: ""
    }

    function generate() {
        if (st !== "idle" && st !== "transcriptFailed" && st !== "failed")
            return
        errorMsg = ""
        // No transcript-readback route → always (re)transcribe to obtain text
        // (documented behavior-parity caveat for hasTranscript meetings).
        st = "transcribing"
        transcribeReq.post("/api/v1/transcribe",
                           { "path": audioPath, "meeting_id": meetingId })
    }

    function generateProtocol() {
        st = "generating"
        var body = { "transcript": transcript, "meeting_name": meetingName }
        if (prefs.defaultTemplate.trim().length > 0)
            body["template_name"] = prefs.defaultTemplate.trim()
        protocolsReq.post("/api/v1/protocols", body)
    }

    Request {
        id: transcribeReq
        onOk: function (j) {
            scr.transcript = j.text
            scr.store.refresh() // has_transcript flips
            scr.generateProtocol()
        }
        onFail: function (s, e) {
            scr.errorMsg = e
            scr.st = "transcriptFailed"
        }
    }

    Request {
        id: protocolsReq
        onOk: function (j) {
            scr.store.cacheProtocol(scr.meetingId, j.markdown)
            scr.st = "done"
            scr.shell.showDetail({
                "id": scr.meetingId, "name": scr.meetingName,
                "audio_path": scr.audioPath, "has_transcript": true,
                "created_at": 0
            })
        }
        onFail: function (s, e) {
            scr.errorMsg = e
            scr.st = "failed"
        }
    }

    Component.onCompleted: if (autoStart && st === "idle") generate()

    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            ToolButton {
                text: qsTr("‹ Назад")
                enabled: scr.st === "idle" || scr.st === "transcriptFailed"
                         || scr.st === "failed"
                onClicked: scr.shell.showList()
            }
            Label {
                text: qsTr("Генерация протокола")
                font.bold: true
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
            }
            Item { Layout.preferredWidth: 64 }
        }
    }

    // ── idle ────────────────────────────────────────────────────────────────
    ColumnLayout {
        visible: scr.st === "idle"
        anchors.fill: parent
        anchors.margins: 32
        spacing: 20
        Label { text: scr.meetingName; font.pixelSize: 18; font.bold: true }
        Label {
            opacity: 0.7
            text: scr.hasTranscript
                  ? qsTr("Транскрипт готов")
                  : qsTr("Транскрипция будет запущена автоматически")
        }
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 6
            Label { text: qsTr("Шаблон протокола (необязательно)"); opacity: 0.7 }
            TextField {
                Layout.fillWidth: true
                text: prefs.defaultTemplate
                placeholderText: qsTr("По умолчанию")
                onEditingFinished: prefs.defaultTemplate = text
            }
        }
        Item { Layout.fillHeight: true }
        Button {
            Layout.fillWidth: true
            text: qsTr("Сгенерировать протокол")
            highlighted: true
            onClicked: scr.generate()
        }
    }

    // ── progress (transcribing / generating / done) ─────────────────────────
    ColumnLayout {
        visible: scr.st === "transcribing" || scr.st === "generating"
                 || scr.st === "done"
        anchors.centerIn: parent
        spacing: 16
        BusyIndicator {
            Layout.alignment: Qt.AlignHCenter
            running: scr.st !== "done"
        }
        Label {
            Layout.alignment: Qt.AlignHCenter
            font.pixelSize: 16
            text: scr.st === "transcribing" ? qsTr("Транскрипция аудио…")
                : scr.st === "generating"  ? qsTr("Генерация протокола…")
                                           : qsTr("Готово!")
        }
        Label {
            Layout.alignment: Qt.AlignHCenter
            opacity: 0.6
            text: scr.st === "transcribing"
                    ? qsTr("это может занять несколько минут")
                : scr.st === "generating"
                    ? qsTr("Claude обрабатывает транскрипт")
                    : qsTr("Открываем протокол…")
        }
    }

    // ── error (transcriptFailed / failed) ───────────────────────────────────
    ColumnLayout {
        visible: scr.st === "transcriptFailed" || scr.st === "failed"
        anchors.centerIn: parent
        width: parent.width - 64
        spacing: 16
        Label {
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            color: scr.palette.toolTipText
            text: qsTr("Ошибка: %1").arg(scr.errorMsg)
        }
        Button {
            Layout.alignment: Qt.AlignHCenter
            text: qsTr("Повторить")
            onClicked: scr.generate()
        }
    }
}
