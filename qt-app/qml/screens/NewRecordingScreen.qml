// RecordingViewModel + Compose NewRecordingScreen behavior, reimplemented
// client-side (audit). Record path → POST /api/v1/recordings(/:id/stop).
// Import path → POST /api/v1/jobs tracked by JobPoller (keeps the async route
// + section-03 JobPoller exercised). Fusion only, no restyling.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtCore
import MeetingAssistant

Page {
    id: scr
    property var shell
    property var store

    // "idle" | "recording" | "stopping" | "error"
    property string st: "idle"
    property string recId: ""
    property string recName: ""
    property int elapsed: 0
    property string errorMsg: ""
    property string pendingOpenId: ""

    // Recording defaults persisted client-side (audit: Settings reimplemented
    // via QtCore.Settings); shared with SettingsScreen.
    Settings {
        id: prefs
        category: "recording"
        property string source: "mixed"
        property bool echoCancel: false
    }

    function fmt(s) {
        var m = Math.floor(s / 60)
        var sec = s % 60
        return m + ":" + (sec < 10 ? "0" : "") + sec
    }

    function start(name, source, echoCancel) {
        if (st !== "idle")
            return
        recName = name.trim().length ? name.trim() : qsTr("Встреча")
        startReq.post("/api/v1/recordings", {
            "name": recName, "source": source, "echo_cancel": echoCancel
        })
    }

    function stop() {
        if (st !== "recording")
            return
        st = "stopping"
        stopReq.post("/api/v1/recordings/" + recId + "/stop", {})
    }

    Request {
        id: startReq
        onOk: function (j) {
            scr.recId = j.id
            scr.recName = j.name
            scr.elapsed = 0
            scr.st = "recording"
        }
        onFail: function (s, e) {
            scr.errorMsg = qsTr("Ошибка старта записи: %1").arg(e)
            scr.st = "error"
        }
    }

    Request {
        id: stopReq
        onOk: function (j) {
            scr.store.refresh()
            scr.shell.showGenerate(j, true)
        }
        onFail: function (s, e) {
            scr.errorMsg = qsTr("Ошибка остановки записи: %1").arg(e)
            scr.st = "error"
        }
    }

    Timer {
        running: scr.st === "recording"
        interval: 1000
        repeat: true
        onTriggered: scr.elapsed += 1
    }

    // ── Import: async transcription job + JobPoller (background) ─────────────
    Request {
        id: importReq
        onOk: function (j) {
            jobPoller.jobId = j.id
            jobPoller.start()
            scr.pendingOpenId = j.meeting_id
            scr.store.refresh()
        }
        onFail: function (s, e) {
            scr.errorMsg = qsTr("Не удалось импортировать файл: %1").arg(e)
            scr.st = "error"
        }
    }

    JobPoller {
        id: jobPoller
        api: api
        intervalMs: 2000
        onStatusChanged: function (status, job) {
            if (status === "done" || status === "failed")
                scr.store.refresh()
        }
    }

    // After import, jump to the meeting once the refreshed list contains it.
    Connections {
        target: scr.store
        function onMeetingsChanged() {
            if (scr.pendingOpenId.length === 0)
                return
            var m = scr.store.meetingById(scr.pendingOpenId)
            if (m) {
                scr.pendingOpenId = ""
                scr.shell.showDetail(m)
            }
        }
    }

    FileDialog {
        id: filePicker
        title: qsTr("Выберите аудиофайл")
        nameFilters: [qsTr("Аудио (*.wav *.mp3 *.m4a *.flac *.ogg)"), qsTr("Все файлы (*)")]
        onAccepted: {
            var path = selectedFile.toString().replace(/^file:\/\//, "")
            importReq.post("/api/v1/jobs", { "audio_path": path, "name": nameField.text })
        }
    }

    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            ToolButton {
                text: qsTr("‹ Назад")
                enabled: scr.st === "idle" || scr.st === "error"
                onClicked: scr.shell.showList()
            }
            Label {
                text: qsTr("Новая запись")
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

        TextField {
            id: nameField
            Layout.fillWidth: true
            placeholderText: qsTr("Название встречи")
        }

        Label { text: qsTr("Источник звука"); opacity: 0.7 }
        RowLayout {
            spacing: 16
            ButtonGroup { id: srcGroup }
            RadioButton {
                text: qsTr("Микрофон"); ButtonGroup.group: srcGroup
                checked: prefs.source === "mic"
                onCheckedChanged: if (checked) prefs.source = "mic"
            }
            RadioButton {
                text: qsTr("Система"); ButtonGroup.group: srcGroup
                checked: prefs.source === "system"
                onCheckedChanged: if (checked) prefs.source = "system"
            }
            RadioButton {
                text: qsTr("Оба"); ButtonGroup.group: srcGroup
                checked: prefs.source === "mixed"
                onCheckedChanged: if (checked) prefs.source = "mixed"
            }
        }

        RowLayout {
            Layout.fillWidth: true
            ColumnLayout {
                Layout.fillWidth: true
                Label { text: qsTr("Подавление эха") }
                Label {
                    text: qsTr("Рекомендуется при записи через микрофон")
                    opacity: 0.6
                    font.pixelSize: 11
                }
            }
            Switch {
                checked: prefs.echoCancel
                onToggled: prefs.echoCancel = checked
            }
        }

        Item { Layout.fillHeight: true }

        Button {
            Layout.fillWidth: true
            text: qsTr("Начать запись")
            highlighted: true
            onClicked: scr.start(nameField.text, prefs.source, prefs.echoCancel)
        }
        Button {
            Layout.fillWidth: true
            text: qsTr("Импортировать аудиофайл…")
            onClicked: filePicker.open()
        }
    }

    // ── recording ───────────────────────────────────────────────────────────
    ColumnLayout {
        visible: scr.st === "recording"
        anchors.centerIn: parent
        spacing: 16
        Label {
            Layout.alignment: Qt.AlignHCenter
            text: qsTr("●  ЗАПИСЬ")
            color: scr.palette.toolTipText
            SequentialAnimation on opacity {
                running: scr.st === "recording"
                loops: Animation.Infinite
                NumberAnimation { from: 1.0; to: 0.3; duration: 800 }
                NumberAnimation { from: 0.3; to: 1.0; duration: 800 }
            }
        }
        Label {
            Layout.alignment: Qt.AlignHCenter
            text: scr.fmt(scr.elapsed)
            font.pixelSize: 44
        }
        Label {
            Layout.alignment: Qt.AlignHCenter
            text: scr.recName
            opacity: 0.7
        }
        Button {
            Layout.alignment: Qt.AlignHCenter
            Layout.preferredWidth: 220
            text: qsTr("Остановить запись")
            onClicked: scr.stop()
        }
    }

    // ── stopping ────────────────────────────────────────────────────────────
    ColumnLayout {
        visible: scr.st === "stopping"
        anchors.centerIn: parent
        spacing: 16
        BusyIndicator { Layout.alignment: Qt.AlignHCenter; running: true }
        Label {
            Layout.alignment: Qt.AlignHCenter
            text: qsTr("Сохранение записи…")
            opacity: 0.7
        }
    }

    // ── error ───────────────────────────────────────────────────────────────
    ColumnLayout {
        visible: scr.st === "error"
        anchors.centerIn: parent
        spacing: 16
        Label {
            Layout.alignment: Qt.AlignHCenter
            text: scr.errorMsg
            color: scr.palette.toolTipText
            wrapMode: Text.WordWrap
        }
        Button {
            Layout.alignment: Qt.AlignHCenter
            text: qsTr("Попробовать снова")
            onClicked: scr.st = "idle"
        }
    }
}
