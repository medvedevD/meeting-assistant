// Protocol generation as a two-step job chain. Both steps go through the
// persisting job pipeline (POST /meetings/:id/reprocess), so the worker writes
// live stage/percent into the shared LiveProgress map and the same
// `PipelineProgress` component renders both:
//   1. kind:"transcribe" — only when the meeting has no transcript yet.
//   2. kind:"protocol"   — enqueued after the transcribe job reaches "done".
// Enqueue + the transcribe->protocol chain live in ActiveJobsStore, so leaving
// this screen mid-run keeps both the chain and the visible progress alive.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import MeetingAssistant
import "../i18n/errors.js" as Errors

Page {
    id: scr
    property var shell
    property var store
    property string meetingId
    property string meetingName
    property string audioPath
    property bool hasTranscript: false
    property bool autoStart: false

    // Live job for this meeting, owned by the store; `version` re-evaluates
    // these on every poll.
    readonly property var jobEntry: (ActiveJobsStore.version,
                                     ActiveJobsStore.entryFor(meetingId))
    readonly property bool jobLive: jobEntry !== null && jobEntry.terminalAt === 0
    // The user asked to generate (or autoStart fired) this session.
    property bool started: false
    property string errorMsg: ""
    // Sidecar error_class for the last failure, so the failed view can render an
    // actionable message (e.g. "LLM не настроена" + settings link) via errors.js.
    property string errorClass: ""
    // Kind of the job that last failed, to resume the right step on retry.
    property string lastFailedKind: ""

    // "idle" | "running" | "failed" — derived from the store + local intent.
    function uiState() {
        if (errorMsg.length > 0) return "failed"
        if (jobLive) return "running"
        if (started) return "running"   // enqueue in flight
        return "idle"
    }

    // Per-generation template override, seeded from the server default
    // (decision #13). Editing it does NOT change the global default.
    property string templateName: ""
    Connections {
        target: SettingsStore
        function onLoadedChanged() {
            if (SettingsStore.loaded && scr.templateName.length === 0)
                scr.templateName = SettingsStore.defaultTemplate()
        }
    }

    function stateLabel() {
        if (errorMsg.length > 0) return qsTr("Ошибка")
        if (jobEntry && jobEntry.kind === "transcribe") return qsTr("Распознавание речи")
        if (jobEntry && jobEntry.kind === "protocol")   return qsTr("Составление протокола")
        if (started) return qsTr("Запуск…")
        return qsTr("Готово к запуску")
    }

    function generate() {
        if (jobLive)
            return
        errorMsg = ""
        errorClass = ""
        started = true
        // On retry after a protocol-step failure the transcript already exists,
        // so skip straight to the protocol job.
        var ht = (lastFailedKind === "protocol") || hasTranscript
        ActiveJobsStore.startGeneration(meetingId, ht, templateName.trim())
    }

    Connections {
        target: ActiveJobsStore
        // The store chains transcribe->protocol internally; this only fires on
        // the terminal job (protocol done/failed, or a transcribe failure).
        function onJobFinished(meetingId, status, job, kind) {
            if (meetingId !== scr.meetingId)
                return
            if (status === "done") {
                scr.store.refresh()
                scr.shell.showDetail({
                    "id": scr.meetingId, "name": scr.meetingName,
                    "audio_path": scr.audioPath, "has_transcript": true,
                    "created_at": 0
                })
            } else {
                scr.lastFailedKind = kind
                scr.errorClass = (job && job.error_class) ? job.error_class : ""
                scr.errorMsg = (job && job.last_error)
                        ? job.last_error
                        : (kind === "transcribe"
                           ? qsTr("Не удалось распознать речь")
                           : qsTr("Не удалось сгенерировать протокол"))
            }
        }
        function onEnqueueFailed(meetingId, error) {
            if (meetingId === scr.meetingId) {
                scr.errorClass = ""
                scr.errorMsg = error
            }
        }
    }

    Component.onCompleted: {
        if (SettingsStore.loaded) templateName = SettingsStore.defaultTemplate()
        else SettingsStore.refresh()
        if (autoStart && uiState() === "idle") generate()
    }

    background: Rectangle { color: Theme.paper }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: 64
            Layout.maximumHeight: 64
            Layout.leftMargin: 24
            Layout.rightMargin: 24
            Layout.topMargin: 16
            Layout.bottomMargin: 16
            spacing: 12

            MeetyButton {
                variant: "ghost"
                iconName: "arrow-left"
                text: qsTr("Назад")
                enabled: scr.uiState() === "idle" || scr.uiState() === "failed"
                onClicked: scr.shell.showList()
            }
            Text {
                Layout.fillWidth: true
                text: qsTr("Генерация протокола")
                font.family: Theme.fontSerif
                font.pixelSize: Theme.fsTitle
                font.weight: Theme.wMedium
                font.letterSpacing: Theme.tracking(Theme.fsTitle, -0.02)
                color: Theme.ink
                elide: Text.ElideRight
            }
            Rectangle {
                radius: 999
                implicitWidth: pillRow.implicitWidth + 20
                implicitHeight: 26
                color: scr.uiState() === "failed" ? Theme.paper3 : Theme.accentTint
                Row {
                    id: pillRow
                    anchors.centerIn: parent
                    spacing: 7
                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 6; height: 6; radius: 3
                        color: scr.uiState() === "failed" ? Theme.rec : Theme.accent
                        SequentialAnimation on opacity {
                            running: scr.uiState() === "running"
                            loops: Animation.Infinite
                            NumberAnimation { from: 1.0; to: 0.35; duration: 700 }
                            NumberAnimation { from: 0.35; to: 1.0; duration: 700 }
                        }
                    }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: scr.stateLabel()
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsSmall
                        font.weight: Theme.wSemiBold
                        color: scr.uiState() === "failed" ? Theme.rec : Theme.accent2
                    }
                }
            }
        }

        Rectangle { Layout.fillWidth: true; height: 1; color: Theme.rule }

        // ── idle ────────────────────────────────────────────────────────────
        ScrollView {
            visible: scr.uiState() === "idle"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentWidth: availableWidth
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

            ColumnLayout {
                width: Math.min(720, parent.width - 48)
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 22

                Item { Layout.preferredHeight: 24 }
                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: idleCard.implicitHeight + 40
                    radius: Theme.rLg
                    color: Theme.paperSub
                    border.width: 1
                    border.color: Theme.rule

                    ColumnLayout {
                        id: idleCard
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.margins: 28
                        spacing: 0

                        Text {
                            Layout.fillWidth: true
                            text: scr.templateName.length > 0
                                  ? scr.templateName : qsTr("Простой протокол")
                            horizontalAlignment: Text.AlignHCenter
                            font.family: Theme.fontUi
                            font.pixelSize: Theme.fsMicro
                            font.weight: Theme.wSemiBold
                            font.letterSpacing: Theme.tracking(Theme.fsMicro, 0.16)
                            color: Theme.ink3
                        }
                        Text {
                            Layout.fillWidth: true
                            Layout.topMargin: 6
                            text: scr.meetingName
                            horizontalAlignment: Text.AlignHCenter
                            wrapMode: Text.WordWrap
                            font.family: Theme.fontSerif
                            font.pixelSize: Theme.fsTitle
                            font.weight: Theme.wMedium
                            font.letterSpacing: Theme.tracking(Theme.fsTitle, -0.02)
                            color: Theme.ink
                        }
                        Text {
                            Layout.fillWidth: true
                            Layout.topMargin: 4
                            Layout.bottomMargin: 18
                            text: scr.hasTranscript
                                  ? qsTr("Транскрипт готов")
                                  : qsTr("Транскрипция будет запущена автоматически")
                            horizontalAlignment: Text.AlignHCenter
                            font.family: Theme.fontUi
                            font.pixelSize: Theme.fsSmall
                            color: Theme.ink3
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            Layout.bottomMargin: 18
                            spacing: 0
                            Repeater {
                                model: [qsTr("Подготовка"), qsTr("Транскрипция"), qsTr("Протокол")]
                                delegate: ColumnLayout {
                                    required property var modelData
                                    required property int index
                                    Layout.fillWidth: true
                                    spacing: 9
                                    Rectangle {
                                        Layout.alignment: Qt.AlignHCenter
                                        width: 36; height: 36; radius: 18
                                        color: index === 0 ? Theme.paper : Theme.paper
                                        border.width: 1
                                        border.color: index === 0 ? Theme.accent : Theme.rule2
                                        Rectangle {
                                            anchors.centerIn: parent
                                            visible: index === 0
                                            width: 44; height: 44; radius: 22
                                            color: Qt.rgba(Theme.accentTint.r, Theme.accentTint.g, Theme.accentTint.b, 0.75)
                                            z: -1
                                        }
                                        Text {
                                            anchors.centerIn: parent
                                            text: index + 1
                                            font.family: Theme.fontUi
                                            font.pixelSize: Theme.fsBody
                                            font.weight: Theme.wSemiBold
                                            color: index === 0 ? Theme.accent : Theme.ink3
                                        }
                                    }
                                    Text {
                                        Layout.fillWidth: true
                                        text: modelData
                                        horizontalAlignment: Text.AlignHCenter
                                        font.family: Theme.fontUi
                                        font.pixelSize: Theme.fsSmall
                                        font.weight: index === 0 ? Theme.wSemiBold : Theme.wMedium
                                        color: index === 0 ? Theme.ink : Theme.ink3
                                    }
                                }
                            }
                        }

                        Text {
                            Layout.fillWidth: true
                            Layout.bottomMargin: 8
                            text: qsTr("Шаблон протокола")
                            font.family: Theme.fontUi
                            font.pixelSize: Theme.fsSmall
                            font.weight: Theme.wSemiBold
                            color: Theme.ink3
                        }
                        MeetyField {
                            Layout.fillWidth: true
                            text: scr.templateName
                            placeholderText: qsTr("По умолчанию")
                            onEditingFinished: scr.templateName = text
                        }

                        MeetyButton {
                            Layout.fillWidth: true
                            Layout.topMargin: 18
                            variant: "accent"
                            large: true
                            iconName: "sparkle"
                            text: qsTr("Сгенерировать протокол")
                            onClicked: scr.generate()
                        }
                    }
                }
                Item { Layout.preferredHeight: 48 }
            }
        }

        // ── running (both stages rendered via PipelineProgress) ──────────────
        ColumnLayout {
            visible: scr.uiState() === "running"
            Layout.alignment: Qt.AlignCenter
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.preferredWidth: Math.min(720, parent.width - 48)
            Layout.maximumWidth: 720
            spacing: 22

            Item { Layout.fillHeight: true }
            Rectangle {
                Layout.fillWidth: true
                implicitHeight: progressCard.implicitHeight + 40
                radius: Theme.rLg
                color: Theme.paperSub
                border.width: 1
                border.color: Theme.rule

                ColumnLayout {
                    id: progressCard
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.margins: 28
                    spacing: 16

                    Text {
                        Layout.fillWidth: true
                        text: scr.templateName.length > 0
                              ? scr.templateName : qsTr("Простой протокол")
                        horizontalAlignment: Text.AlignHCenter
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsMicro
                        font.weight: Theme.wSemiBold
                        font.letterSpacing: Theme.tracking(Theme.fsMicro, 0.16)
                        color: Theme.ink3
                    }
                    Text {
                        Layout.fillWidth: true
                        text: scr.meetingName
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.WordWrap
                        font.family: Theme.fontSerif
                        font.pixelSize: Theme.fsTitle
                        font.weight: Theme.wMedium
                        font.letterSpacing: Theme.tracking(Theme.fsTitle, -0.02)
                        color: Theme.ink
                    }

                    PipelineProgress {
                        Layout.fillWidth: true
                        apiClient: api
                        // Store-driven: the snapshot follows whichever job the
                        // store is polling, including the transcribe->protocol
                        // hand-off. `finished` is handled via the store's
                        // jobFinished signal above, not here.
                        sourceJob: scr.jobEntry ? scr.jobEntry.job : null
                        onOpenSettings: scr.shell.showSettings()
                    }
                }
            }
            Item { Layout.fillHeight: true }
        }

        // ── error (any failed stage) ────────────────────────────────────────
        ColumnLayout {
            id: errBlock
            visible: scr.uiState() === "failed"
            // Actionable, class-aware copy (e.g. "LLM не настроена" + settings).
            readonly property var err: Errors.describe(scr.errorClass, scr.errorMsg)
            Layout.alignment: Qt.AlignCenter
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.preferredWidth: Math.min(560, parent.width - 64)
            Layout.maximumWidth: 560
            spacing: 12
            Item { Layout.fillHeight: true }
            Text {
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
                font.family: Theme.fontUi
                font.pixelSize: Theme.fsBodyLg
                font.weight: Theme.wSemiBold
                color: errBlock.err.neutral === true ? Theme.ink : Theme.rec
                text: errBlock.err.title
            }
            Text {
                Layout.fillWidth: true
                visible: errBlock.err.hint && errBlock.err.hint.length > 0
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
                font.family: Theme.fontUi
                font.pixelSize: Theme.fsBody
                color: Theme.ink3
                text: errBlock.err.hint
            }
            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: 4
                spacing: 8
                MeetyButton {
                    visible: errBlock.err.settings === true
                    variant: "accent"
                    text: qsTr("Открыть настройки")
                    onClicked: scr.shell.showSettings()
                }
                MeetyButton {
                    text: qsTr("Повторить")
                    onClicked: scr.generate()
                }
            }
            Item { Layout.fillHeight: true }
        }
    }
}
