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

    // Active reprocess job (transcribe / protocol) lives in ActiveJobsStore so it
    // survives navigation; `version` makes these re-evaluate on every poll.
    readonly property var jobEntry: (ActiveJobsStore.version,
                                     ActiveJobsStore.entryFor(meetingId))
    readonly property bool jobActive: jobEntry !== null && jobEntry.terminalAt === 0
    property string actionNote: ""

    background: Rectangle { color: Theme.paper }

    Component.onCompleted: loadProtocol()

    function loadProtocol() {
        protocolReq.get("/api/v1/meetings/" + meetingId)
    }

    function reprocess(kind) {
        scr.actionNote = ""
        ActiveJobsStore.reprocess(meetingId, kind, "")
    }

    Connections {
        target: ActiveJobsStore
        function onJobFinished(meetingId, status, job, kind) {
            if (meetingId !== scr.meetingId)
                return
            scr.store.refresh()
            if (status === "done") {
                scr.actionNote = qsTr("Готово.")
                scr.loadProtocol()
            }
        }
        function onEnqueueFailed(meetingId, error) {
            if (meetingId === scr.meetingId)
                scr.actionNote = qsTr("Ошибка: %1").arg(error)
        }
    }

    Request {
        id: protocolReq
        onOk: function (j) {
            scr.protocol = (j && j.protocol) ? j.protocol : ""
            if (!scr.hasProtocol && scr.shell.selectedId === scr.meetingId
                    && !ActiveJobsStore.isActive(scr.meetingId))
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
            enabled: scr.audioPath.length > 0 && !scr.jobActive
            onTriggered: scr.reprocess("transcribe")
        }
        MeetyMenuItem {
            text: qsTr("Перегенерировать протокол")
            iconName: "sparkle"
            enabled: !scr.jobActive
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

    MeetyDialog {
        id: confirmCancel
        preferredWidth: 420
        title: qsTr("Прервать обработку?")
        onAccepted: ActiveJobsStore.cancel(scr.meetingId)

        Text {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            lineHeight: 1.35
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBodyLg
            color: Theme.ink2
            text: qsTr("Текущая задача будет помечена как отменённая. Уже выполненная часть транскрипта или протокола сохранится.")
        }

        footer: MeetyDialogActions {
            dialog: confirmCancel
            cancelText: qsTr("Назад")
            confirmText: qsTr("Прервать")
            confirmVariant: "accent"
            confirmIconName: "close"
        }
    }

    MeetyDialog {
        id: confirmDelete
        preferredWidth: 420
        title: qsTr("Удалить встречу?")
        onAccepted: deleteReq.del("/api/v1/meetings/" + scr.meetingId + "?mode=full")

        Text {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            lineHeight: 1.35
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBodyLg
            color: Theme.ink2
            text: qsTr("Встреча, её транскрипт, протокол и аудиофайл будут удалены безвозвратно.")
        }

        footer: MeetyDialogActions {
            dialog: confirmDelete
            cancelText: qsTr("Отмена")
            confirmText: qsTr("Удалить")
            confirmVariant: "accent"
            confirmIconName: "trash"
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
                id: actionButton
                Layout.alignment: Qt.AlignTop
                iconName: "more"
                onClicked: actionMenu.popupFromButton(actionButton)
                MeetyToolTip { text: qsTr("Действия"); visible: parent.hovered }
            }
        }

        Rectangle { Layout.fillWidth: true; height: 1; color: Theme.rule }

        // ── inline reprocess progress + note ─────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 24
            Layout.rightMargin: 24
            Layout.topMargin: (scr.jobEntry !== null
                               || scr.actionNote.length > 0) ? 16 : 0
            spacing: 10

            MeetyCard {
                visible: scr.jobEntry !== null
                Layout.fillWidth: true
                ColumnLayout {
                    width: parent ? parent.width : 0
                    spacing: 12
                    PipelineProgress {
                        Layout.fillWidth: true
                        apiClient: api
                        sourceJob: scr.jobEntry ? scr.jobEntry.job : null
                        onOpenSettings: scr.shell.showSettings()
                    }
                    // Cancel affordance lives next to the progress so the
                    // user sees the way to stop a running job in the same
                    // visual context as what's happening. Hidden once the
                    // user has clicked it (transient "Прерывание…" state)
                    // and after the job reaches a terminal state.
                    RowLayout {
                        Layout.fillWidth: true
                        visible: scr.jobActive
                        Item { Layout.fillWidth: true }
                        MeetyButton {
                            variant: "ghost"
                            iconName: "close"
                            text: (scr.jobEntry && scr.jobEntry.cancelRequested === true)
                                  ? qsTr("Прерывание…")
                                  : qsTr("Прервать")
                            enabled: !(scr.jobEntry && scr.jobEntry.cancelRequested === true)
                            onClicked: confirmCancel.open()
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
