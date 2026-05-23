// 3-step pipeline progress (Phase 5). Driven by a JobPoller polling
// `GET /api/v1/jobs/:id`: the merged response carries live `progress`
// {stage, sub, percent} (in-memory) while active and a terminal `error_class`
// on failure. Collapses the 7 backend PipelineStage values into 3 visible
// steps (Подготовка → Транскрипция → Протокол) with the active step lit, a
// percent bar, and a sub-status line. On failure, renders the localized
// error (i18n/errors.js) and optionally an "Открыть настройки" button.
import QtQuick
import QtQuick.Layouts
import MeetingAssistant
import "../i18n/errors.js" as Errors

ColumnLayout {
    id: root

    // Required wiring.
    property string jobId: ""
    property var apiClient: null
    // Optional: invoked when the tracked job reaches a terminal state.
    signal finished(string status, var job)
    // Optional: invoked when the user clicks "Открыть настройки".
    signal openSettings()

    spacing: 14

    // Latest decoded job snapshot.
    property var job: ({})
    readonly property string status: job.status || "pending"
    readonly property var progress: job.progress || null
    readonly property string stage: progress ? progress.stage : "queued"
    readonly property int percent: progress ? progress.percent : 0
    readonly property string sub: progress ? progress.sub : ""
    readonly property string errorClass: job.error_class || ""
    readonly property bool failed: status === "failed"
    readonly property bool done: status === "done"
    readonly property int displayPercent: done ? 100 : Math.max(0, Math.min(100, percent))
    readonly property bool indeterminate: !failed && !done
                                      && (displayPercent === 0 || stage === "queued"
                                          || stage === "generating_protocol")
    property bool terminalEmitted: false

    // Map the 7 backend stages onto 3 visible steps. -1 = not started.
    // 0 Подготовка | 1 Транскрипция | 2 Протокол.
    function activeStep() {
        switch (root.stage) {
        case "queued":
        case "loading_model":
        case "decoding_audio":
            return 0
        case "transcribing":
        case "writing_transcript":
            return 1
        case "generating_protocol":
            return 2
        case "done":
            return 3
        }
        return 0
    }

    function statusLabel() {
        if (failed) return qsTr("Ошибка")
        if (done) return qsTr("Готово")
        switch (activeStep()) {
        case 0: return qsTr("Подготовка")
        case 1: return qsTr("Распознавание речи")
        case 2: return qsTr("Составление протокола")
        }
        return qsTr("В работе")
    }

    function statusSubline() {
        if (done) return qsTr("Протокол сохранён локально")
        if (sub.length > 0) return sub
        switch (stage) {
        case "queued": return qsTr("В очереди…")
        case "loading_model": return qsTr("Загрузка модели…")
        case "decoding_audio": return qsTr("Подготовка аудио…")
        case "transcribing": return qsTr("Whisper распознаёт речь…")
        case "writing_transcript": return qsTr("Сохраняю транскрипт…")
        case "generating_protocol": return qsTr("LLM сводит встречу в протокол…")
        }
        return qsTr("В работе…")
    }

    function start() {
        if (jobId.length === 0)
            return
        terminalEmitted = false
        poller.jobId = jobId
        poller.start()
    }
    function stop() { poller.stop() }

    function applyJobUpdate(status, j) {
        root.job = j || ({})
        if ((status === "done" || status === "failed") && !root.terminalEmitted) {
            root.terminalEmitted = true
            root.finished(status, root.job)
        }
    }

    JobPoller {
        id: poller
        api: root.apiClient
        intervalMs: 1000
        onStatusChanged: function (status, j) {
            root.applyJobUpdate(status, j)
        }
        onJobUpdated: function (status, j) {
            root.applyJobUpdate(status, j)
        }
        onFailed: function (e) {
            root.job = { "status": "failed", "error_class": "unknown",
                         "last_error": e }
            if (!root.terminalEmitted) {
                root.terminalEmitted = true
                root.finished("failed", root.job)
            }
        }
    }

    Component.onCompleted: start()
    onJobIdChanged: start()

    // ── steps row ────────────────────────────────────────────────────────────
    component Step: ColumnLayout {
        property int index: 0
        property string label: ""
        readonly property bool active: !root.failed && root.activeStep() === index
        readonly property bool complete: root.done || root.activeStep() > index
        Layout.fillWidth: true
        spacing: 9
        Rectangle {
            Layout.alignment: Qt.AlignHCenter
            width: 36; height: 36; radius: 18
            color: complete ? Theme.accent : Theme.paper
            border.width: 1
            border.color: complete || active ? Theme.accent : Theme.rule2
            Rectangle {
                anchors.centerIn: parent
                visible: active && !complete
                width: 44; height: 44; radius: 22
                color: Qt.rgba(Theme.accentTint.r, Theme.accentTint.g, Theme.accentTint.b, 0.75)
                z: -1
            }
            MeetyIcon {
                visible: complete
                anchors.centerIn: parent
                name: "check"
                size: 14
                color: Theme.accentInk
            }
            Text {
                visible: !complete
                anchors.centerIn: parent
                text: index + 1
                font.family: Theme.fontUi
                font.pixelSize: Theme.fsBody
                font.weight: Theme.wSemiBold
                color: active ? Theme.accent : Theme.ink3
            }
        }
        Text {
            Layout.fillWidth: true
            text: label
            horizontalAlignment: Text.AlignHCenter
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsSmall
            font.weight: active ? Theme.wSemiBold : Theme.wMedium
            color: active ? Theme.ink : ((complete && !root.failed) ? Theme.ink2 : Theme.ink3)
            elide: Text.ElideRight
        }
    }

    Rectangle {
        Layout.fillWidth: true
        implicitHeight: steps.implicitHeight
        color: "transparent"

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: Math.max(0, parent.width * 0.16)
            anchors.rightMargin: Math.max(0, parent.width * 0.16)
            y: 17
            height: 1
            color: Theme.rule
        }
        RowLayout {
            id: steps
            anchors.fill: parent
            spacing: 0
            Step { index: 0; label: qsTr("Подготовка") }
            Step { index: 1; label: qsTr("Транскрипция") }
            Step { index: 2; label: qsTr("Протокол") }
        }
    }

    // ── progress bar + sub-status (while running) ─────────────────────────────
    ColumnLayout {
        Layout.fillWidth: true
        visible: !root.failed && !root.done
        spacing: 12
        Rectangle {
            id: progressTrack
            Layout.fillWidth: true
            implicitHeight: 4
            radius: 2
            color: Theme.paper3
            clip: true

            Rectangle {
                id: progressFill
                width: root.indeterminate
                       ? Math.max(80, progressTrack.width * 0.35)
                       : progressTrack.width * root.displayPercent / 100
                height: parent.height
                radius: 2
                color: Theme.accent
                x: root.indeterminate ? -width : 0
                Behavior on width { NumberAnimation { duration: 350; easing.type: Easing.OutCubic } }
                SequentialAnimation on x {
                    running: root.indeterminate && progressTrack.visible
                    loops: Animation.Infinite
                    NumberAnimation {
                        from: -progressFill.width
                        to: progressTrack.width + progressFill.width
                        duration: 1400
                        easing.type: Easing.InOutCubic
                    }
                }
            }
        }
        RowLayout {
            Layout.fillWidth: true
            Text {
                Layout.fillWidth: true
                font.family: Theme.fontMono
                font.pixelSize: Theme.fsSmall
                color: Theme.ink3
                text: root.statusSubline()
                elide: Text.ElideRight
            }
            Text {
                visible: root.displayPercent > 0 && !root.indeterminate
                font.family: Theme.fontMono
                font.pixelSize: Theme.fsSmall
                color: Theme.ink3
                text: root.displayPercent + "%"
            }
        }
    }

    // ── failure ───────────────────────────────────────────────────────────────
    ColumnLayout {
        Layout.fillWidth: true
        visible: root.failed
        spacing: 8
        Text {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            color: Theme.rec
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBodyLg
            font.weight: Theme.wSemiBold
            text: Errors.describe(root.errorClass, root.job.last_error).title
        }
        Text {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            color: Theme.ink2
            text: Errors.describe(root.errorClass, root.job.last_error).hint
        }
        Rectangle {
            visible: Errors.describe(root.errorClass, root.job.last_error).settings
            Layout.alignment: Qt.AlignLeft
            implicitWidth: settingsLabel.implicitWidth + 26
            implicitHeight: 34
            radius: Theme.rMd
            color: settingsMouse.containsMouse ? Theme.paper3 : Theme.paper
            border.width: 1
            border.color: Theme.rule
            Text {
                id: settingsLabel
                anchors.centerIn: parent
                text: qsTr("Открыть настройки")
                font.family: Theme.fontUi
                font.pixelSize: Theme.fsBody
                font.weight: Theme.wMedium
                color: Theme.ink
            }
            MouseArea {
                id: settingsMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.openSettings()
            }
        }
    }
}
