// 3-step pipeline progress (Phase 5). Driven by a JobPoller polling
// `GET /api/v1/jobs/:id`: the merged response carries live `progress`
// {stage, sub, percent} (in-memory) while active and a terminal `error_class`
// on failure. Collapses the 7 backend PipelineStage values into 3 visible
// steps (Подготовка → Транскрипция → Протокол) with the active step lit, a
// percent bar, and a sub-status line. On failure, renders the localized
// error (i18n/errors.js) and optionally an "Открыть настройки" button.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import MeetingAssistant
import "../i18n/errors.js" as Errors

ColumnLayout {
    id: root

    // Required wiring.
    property string jobId: ""
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

    function start() {
        if (jobId.length === 0)
            return
        poller.jobId = jobId
        poller.start()
    }
    function stop() { poller.stop() }

    JobPoller {
        id: poller
        api: api
        intervalMs: 1000
        onStatusChanged: function (status, j) {
            root.job = j || ({})
            if (status === "done" || status === "failed")
                root.finished(status, root.job)
        }
        onFailed: function (e) {
            root.job = { "status": "failed", "error_class": "unknown",
                         "last_error": e }
            root.finished("failed", root.job)
        }
    }

    Component.onCompleted: start()
    onJobIdChanged: start()

    // ── steps row ────────────────────────────────────────────────────────────
    component Step: RowLayout {
        property int index: 0
        property string label: ""
        readonly property bool active: !root.failed && root.activeStep() === index
        readonly property bool complete: root.done || root.activeStep() > index
        spacing: 8
        Rectangle {
            Layout.alignment: Qt.AlignVCenter
            width: 22; height: 22; radius: 11
            color: complete ? root.palette.highlight
                 : active   ? root.palette.highlight
                            : "transparent"
            border.width: 1
            border.color: active || complete
                          ? root.palette.highlight : root.palette.mid
            Label {
                anchors.centerIn: parent
                text: complete ? "✓" : (index + 1)
                color: (active || complete) ? root.palette.highlightedText
                                            : root.palette.text
                font.pixelSize: 12
            }
        }
        Label {
            text: label
            font.bold: active
            opacity: (active || complete) ? 1.0 : 0.55
        }
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 18
        Step { index: 0; label: qsTr("Подготовка") }
        Step { index: 1; label: qsTr("Транскрипция") }
        Step { index: 2; label: qsTr("Протокол") }
        Item { Layout.fillWidth: true }
    }

    // ── progress bar + sub-status (while running) ─────────────────────────────
    ColumnLayout {
        Layout.fillWidth: true
        visible: !root.failed && !root.done
        spacing: 4
        ProgressBar {
            Layout.fillWidth: true
            // Whisper reports percent only during transcription; show an
            // indeterminate bar for the other stages.
            indeterminate: root.percent === 0 || root.stage === "queued"
            from: 0; to: 100
            value: root.percent
        }
        RowLayout {
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true
                opacity: 0.7
                font.pixelSize: 12
                text: root.sub.length > 0 ? root.sub : qsTr("В очереди…")
                elide: Text.ElideRight
            }
            Label {
                visible: root.percent > 0
                opacity: 0.7
                font.pixelSize: 12
                text: root.percent + "%"
            }
        }
    }

    // ── failure ───────────────────────────────────────────────────────────────
    ColumnLayout {
        Layout.fillWidth: true
        visible: root.failed
        spacing: 6
        Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            color: root.palette.toolTipText
            font.bold: true
            text: Errors.describe(root.errorClass, root.job.last_error).title
        }
        Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            opacity: 0.8
            font.pixelSize: 12
            text: Errors.describe(root.errorClass, root.job.last_error).hint
        }
        Button {
            visible: Errors.describe(root.errorClass, root.job.last_error).settings
            text: qsTr("Открыть настройки")
            onClicked: root.openSettings()
        }
    }
}
