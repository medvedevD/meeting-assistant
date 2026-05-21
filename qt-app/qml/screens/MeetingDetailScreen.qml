// MeetingDetailScreen behavior port. Protocol view is the app's core output:
// rendered with the built-in Qt MarkdownText path (audit: chosen renderer +
// limits documented). `protocolLoad` has no sidecar route → the in-session
// MeetingStore cache stands in (audit: scoped reimplement).
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import MeetingAssistant

Page {
    id: scr
    property var shell
    property var store
    property string meetingId
    property string meetingName
    property string audioPath
    property bool hasTranscript: false
    property double createdAt: 0

    readonly property string protocol: store ? store.protocolFor(meetingId) : ""
    readonly property bool hasProtocol: protocol.length > 0

    // Active reprocess job (transcribe / protocol). Drives the inline progress.
    property string activeJobId: ""
    property string actionNote: ""

    function reprocess(kind) {
        scr.actionNote = ""
        reprocessReq.post("/api/v1/meetings/" + meetingId + "/reprocess",
                          { "kind": kind })
    }

    Request {
        id: reprocessReq
        onOk: function (j) { scr.activeJobId = j.job_id }
        onFail: function (s, e) { scr.actionNote = qsTr("Ошибка: %1").arg(e) }
    }
    Request {
        id: deleteReq
        onOk: function (j) {
            scr.store.refresh()
            scr.shell.showList()
        }
        onFail: function (s, e) { scr.actionNote = qsTr("Ошибка удаления: %1").arg(e) }
    }
    Request {
        id: deleteAudioReq
        onOk: function (j) {
            scr.actionNote = qsTr("Аудио удалено, транскрипт сохранён.")
            scr.store.refresh()
        }
        onFail: function (s, e) { scr.actionNote = qsTr("Ошибка: %1").arg(e) }
    }

    Menu {
        id: actionMenu
        MenuItem {
            text: qsTr("Перетранскрибировать")
            enabled: scr.audioPath.length > 0 && scr.activeJobId.length === 0
            onTriggered: scr.reprocess("transcribe")
        }
        MenuItem {
            text: qsTr("Перегенерировать протокол")
            enabled: scr.activeJobId.length === 0
            onTriggered: scr.reprocess("protocol")
        }
        MenuSeparator {}
        MenuItem {
            text: qsTr("Удалить аудио (оставить транскрипт)")
            enabled: scr.audioPath.length > 0
            onTriggered: deleteAudioReq.del("/api/v1/meetings/" + scr.meetingId + "?mode=audio")
        }
        MenuItem {
            text: qsTr("Удалить встречу")
            onTriggered: confirmDelete.open()
        }
    }

    Dialog {
        id: confirmDelete
        modal: true
        anchors.centerIn: Overlay.overlay
        title: qsTr("Удалить встречу?")
        standardButtons: Dialog.Yes | Dialog.No
        onAccepted: deleteReq.del("/api/v1/meetings/" + scr.meetingId + "?mode=full")
        Label {
            width: 360
            wrapMode: Text.WordWrap
            text: qsTr("Встреча, её транскрипт, протокол и аудиофайл будут удалены безвозвратно.")
        }
    }

    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            ToolButton {
                text: qsTr("‹ Назад")
                onClicked: scr.shell.showList()
            }
            Label {
                text: scr.meetingName
                font.bold: true
                elide: Text.ElideRight
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
            }
            ToolButton {
                text: qsTr("Генерировать")
                onClicked: scr.shell.showGenerate({
                    "id": scr.meetingId, "name": scr.meetingName,
                    "audio_path": scr.audioPath,
                    "has_transcript": scr.hasTranscript,
                    "created_at": scr.createdAt
                }, false)
            }
            ToolButton {
                text: qsTr("⟳")
                onClicked: scr.store.refresh()
            }
            ToolButton {
                text: qsTr("⋯")
                onClicked: actionMenu.popup()
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // meta header
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 16
            Label {
                Layout.fillWidth: true
                opacity: 0.7
                text: scr.createdAt > 0
                      ? Qt.formatDateTime(new Date(scr.createdAt * 1000),
                                          Qt.locale(), Locale.ShortFormat)
                      : ""
            }
            Label {
                visible: scr.hasTranscript
                text: qsTr("Транскрипт")
                font.pixelSize: 11
                color: scr.palette.highlight
            }
        }
        // action note (delete-audio result / reprocess errors)
        Label {
            visible: scr.actionNote.length > 0
            Layout.fillWidth: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            wrapMode: Text.WordWrap
            opacity: 0.8
            text: scr.actionNote
        }

        // inline reprocess progress (transcribe / protocol regeneration)
        Pane {
            visible: scr.activeJobId.length > 0
            Layout.fillWidth: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            PipelineProgress {
                width: parent.width
                jobId: scr.activeJobId
                onOpenSettings: scr.shell.showSettings()
                onFinished: function (status, job) {
                    scr.activeJobId = ""
                    scr.store.refresh()
                    if (status === "done")
                        scr.actionNote = qsTr("Готово.")
                }
            }
        }

        MenuSeparator { Layout.fillWidth: true }

        // protocol present → markdown render
        ScrollView {
            visible: scr.hasProtocol
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

            TextEdit {
                padding: 24
                readOnly: true
                selectByMouse: true
                wrapMode: TextEdit.Wrap
                textFormat: TextEdit.MarkdownText
                color: scr.palette.text
                text: scr.protocol
            }
        }

        // no protocol → CTA (parity with Compose NoProtocolPane)
        ColumnLayout {
            visible: !scr.hasProtocol
            Layout.fillWidth: true
            Layout.fillHeight: true
            Item { Layout.fillHeight: true }
            Label {
                Layout.alignment: Qt.AlignHCenter
                text: qsTr("Протокол ещё не сгенерирован")
                font.pixelSize: 16
                opacity: 0.7
            }
            Button {
                visible: scr.audioPath.length > 0
                Layout.alignment: Qt.AlignHCenter
                text: qsTr("Сгенерировать протокол")
                highlighted: true
                onClicked: scr.shell.showGenerate({
                    "id": scr.meetingId, "name": scr.meetingName,
                    "audio_path": scr.audioPath,
                    "has_transcript": scr.hasTranscript,
                    "created_at": scr.createdAt
                }, false)
            }
            Label {
                visible: scr.audioPath.length === 0
                Layout.alignment: Qt.AlignHCenter
                opacity: 0.6
                text: qsTr("Запишите встречу, чтобы сгенерировать протокол")
            }
            Item { Layout.fillHeight: true }
        }
    }
}
