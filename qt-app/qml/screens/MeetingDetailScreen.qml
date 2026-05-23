// MeetingDetailScreen — Meety redesign (qt-redesign Phase 3). Content-bar +
// Editorial protocol render. Protocol is Markdown (the core emits markdown), so
// "editorial" = MarkdownText styled with the serif face on warm paper, in a
// centred reading column. Behaviour (reprocess / delete / progress) unchanged.
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

    // Persisted protocol, loaded from the backend (GET /api/v1/meetings/:id).
    property string protocol: ""
    readonly property bool hasProtocol: protocol.length > 0

    // Active reprocess job (transcribe / protocol). Drives the inline progress.
    property string activeJobId: ""
    property string actionNote: ""

    background: Rectangle { color: Theme.paper }

    Component.onCompleted: loadProtocol()

    function loadProtocol() {
        protocolReq.get("/api/v1/meetings/" + meetingId)
    }

    function reprocess(kind) {
        scr.actionNote = ""
        reprocessReq.post("/api/v1/meetings/" + meetingId + "/reprocess",
                          { "kind": kind })
    }

    Request {
        id: protocolReq
        onOk: function (j) {
            scr.protocol = (j && j.protocol) ? j.protocol : ""
            if (!scr.hasProtocol && scr.shell.selectedId === scr.meetingId)
                scr.shell.showNoProtocol({
                    "id": scr.meetingId, "name": scr.meetingName,
                    "audio_path": scr.audioPath,
                    "has_transcript": scr.hasTranscript,
                    "created_at": scr.createdAt
                })
        }
        onFail: function (s, e) { scr.actionNote = qsTr("Ошибка: %1").arg(e) }
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

    MeetyMenu {
        id: actionMenu
        MeetyMenuItem {
            text: qsTr("Перетранскрибировать")
            iconName: "refresh"
            enabled: scr.audioPath.length > 0 && scr.activeJobId.length === 0
            onTriggered: scr.reprocess("transcribe")
        }
        MeetyMenuItem {
            text: qsTr("Перегенерировать протокол")
            iconName: "sparkle"
            enabled: scr.activeJobId.length === 0
            onTriggered: scr.reprocess("protocol")
        }
        MenuSeparator {}
        MeetyMenuItem {
            text: qsTr("Удалить аудио (оставить транскрипт)")
            iconName: "trash"
            danger: true
            enabled: scr.audioPath.length > 0
            onTriggered: deleteAudioReq.del("/api/v1/meetings/" + scr.meetingId + "?mode=audio")
        }
        MeetyMenuItem {
            text: qsTr("Удалить встречу")
            iconName: "trash"
            danger: true
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

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // ── content-bar (detail) ─────────────────────────────────────────────
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 24
            Layout.rightMargin: 24
            Layout.topMargin: 16
            Layout.bottomMargin: 16
            spacing: 12

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 4
                Text {
                    Layout.fillWidth: true
                    text: scr.meetingName
                    elide: Text.ElideRight
                    font.family: Theme.fontSerif
                    font.pixelSize: Theme.fsTitle
                    font.weight: Theme.wMedium
                    font.letterSpacing: Theme.tracking(Theme.fsTitle, -0.02)
                    color: Theme.ink
                }
                RowLayout {
                    spacing: 10
                    Text {
                        visible: scr.createdAt > 0
                        text: Qt.formatDateTime(new Date(scr.createdAt * 1000),
                                                "d MMMM, HH:mm")
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsSmall
                        color: Theme.ink3
                    }
                    Rectangle {
                        visible: scr.hasTranscript
                        width: 2.5; height: 2.5; radius: 1.25; color: Theme.ink4
                    }
                    Text {
                        visible: scr.hasTranscript
                        text: qsTr("транскрипт готов")
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsSmall
                        color: Theme.ink3
                    }
                }
            }

            MeetyButton {
                Layout.alignment: Qt.AlignTop
                visible: scr.hasProtocol
                variant: "ghost"
                iconName: "sparkle"
                text: qsTr("Перегенерировать")
                onClicked: scr.shell.showGenerate({
                    "id": scr.meetingId, "name": scr.meetingName,
                    "audio_path": scr.audioPath,
                    "has_transcript": scr.hasTranscript,
                    "created_at": scr.createdAt
                }, false)
            }
            MeetyIconButton {
                Layout.alignment: Qt.AlignTop
                iconName: "more"
                onClicked: actionMenu.popup()
                MeetyToolTip { text: qsTr("Действия"); visible: parent.hovered }
            }
        }

        Rectangle { Layout.fillWidth: true; height: 1; color: Theme.rule }

        // ── inline reprocess progress + note ─────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 24
            Layout.rightMargin: 24
            Layout.topMargin: (scr.activeJobId.length > 0
                               || scr.actionNote.length > 0) ? 16 : 0
            spacing: 10

            MeetyCard {
                visible: scr.activeJobId.length > 0
                Layout.fillWidth: true
                PipelineProgress {
                    width: parent ? parent.width : 0
                    apiClient: api
                    jobId: scr.activeJobId
                    onOpenSettings: scr.shell.showSettings()
                    onFinished: function (status, job) {
                        scr.activeJobId = ""
                        scr.store.refresh()
                        if (status === "done") {
                            scr.actionNote = qsTr("Готово.")
                            scr.loadProtocol()
                        }
                    }
                }
            }
            Text {
                visible: scr.actionNote.length > 0
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                font.family: Theme.fontUi
                font.pixelSize: Theme.fsBody
                color: Theme.ink2
                text: scr.actionNote
            }
        }

        // ── body ─────────────────────────────────────────────────────────────
        ScrollView {
            visible: scr.hasProtocol
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentWidth: availableWidth
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

            Item {
                width: parent.width
                implicitHeight: article.implicitHeight + 120

                ColumnLayout {
                    id: article
                    width: Math.min(680, parent.width - 64)
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: 40
                    spacing: 0

                    // meta line
                    Text {
                        visible: scr.createdAt > 0
                        Layout.bottomMargin: 36
                        text: Qt.formatDateTime(new Date(scr.createdAt * 1000),
                                                "d MMMM yyyy")
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsBody
                        color: Theme.ink3
                    }

                    ProtocolDocument {
                        Layout.fillWidth: true
                        markdown: scr.protocol
                    }
                }
            }
        }
    }
}
