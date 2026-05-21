// New recording + import. Record path → POST /api/v1/recordings(/:id/stop).
// Import path → MeetingStore.importFile (POST /api/v1/meetings/import, with
// dedup + auto-transcribe). Three ways in: drag&drop (text/uri-list), the
// "Импорт файла" FileDialog, and "Из папки" (GET /meetings/scan → checkbox
// list). Recording defaults come from the server-side SettingsStore now
// (decision #13); the radio/switch here are a per-recording override.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import MeetingAssistant

Page {
    id: scr
    property var shell
    property var store

    // "idle" | "recording" | "stopping" | "importing" | "error"
    property string st: "idle"
    property string recId: ""
    property string recName: ""
    property int elapsed: 0
    property string errorMsg: ""
    // Meeting to open once the refreshed list contains it (import is async).
    property string pendingOpenId: ""

    // Per-recording override, seeded from the server defaults.
    property string source: "mixed"
    property bool echoCancel: false

    function seedDefaults() {
        if (!SettingsStore.loaded) return
        var rec = SettingsStore.recording()
        source = rec.source || "mixed"
        echoCancel = rec.echo_cancel === true
    }
    Component.onCompleted: {
        if (SettingsStore.loaded) seedDefaults(); else SettingsStore.refresh()
    }
    Connections {
        target: SettingsStore
        function onLoadedChanged() { if (SettingsStore.loaded) scr.seedDefaults() }
    }

    function fmt(s) {
        var m = Math.floor(s / 60)
        var sec = s % 60
        return m + ":" + (sec < 10 ? "0" : "") + sec
    }

    function start(name) {
        if (st !== "idle")
            return
        recName = name.trim().length ? name.trim() : qsTr("Встреча")
        startReq.post("/api/v1/recordings", {
            "name": recName, "source": scr.source, "echo_cancel": scr.echoCancel
        })
    }

    function stop() {
        if (st !== "recording")
            return
        st = "stopping"
        stopReq.post("/api/v1/recordings/" + recId + "/stop", {})
    }

    function importPath(path, copy) {
        if (!path || !path.length) return
        scr.st = "importing"
        scr.store.importFile(path, copy === undefined ? true : copy, true)
    }

    // ── import wiring via MeetingStore signals ────────────────────────────────
    Connections {
        target: scr.store
        function onImportDone(meetingId, jobId) {
            var m = scr.store.meetingById(meetingId)
            if (m) scr.shell.showDetail(m)
            else scr.pendingOpenId = meetingId // open when the refreshed list lands
        }
        function onMeetingsChanged() {
            if (scr.pendingOpenId.length === 0) return
            var m = scr.store.meetingById(scr.pendingOpenId)
            if (m) { scr.pendingOpenId = ""; scr.shell.showDetail(m) }
        }
        function onImportFailed(message) {
            scr.errorMsg = qsTr("Не удалось импортировать файл: %1").arg(message)
            scr.st = "error"
        }
        function onImportConflict(existingId) {
            var m = scr.store.meetingById(existingId)
            if (m) scr.shell.showDetail(m)
            else {
                scr.errorMsg = qsTr("Этот файл уже импортирован.")
                scr.st = "error"
            }
        }
        function onScanDone(candidates) {
            scanModel.clear()
            for (var i = 0; i < candidates.length; ++i)
                scanModel.append({ "path": candidates[i].path,
                                   "name": candidates[i].name, "checked": false })
            scanDialog.loading = false
        }
        function onScanFailed(message) {
            scanDialog.loading = false
            scanDialog.errorText = message
        }
    }

    // ScreenCaptureKit permission deep-link (unchanged from the record path).
    readonly property string screenRecordingSettingsUrl:
        "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
    function isPermissionError(msg) {
        return msg && msg.indexOf(scr.screenRecordingSettingsUrl) !== -1
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
            if (scr.isPermissionError(e)) {
                permissionDialog.open()
                scr.st = "idle"
                return
            }
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

    Dialog {
        id: permissionDialog
        modal: true
        anchors.centerIn: Overlay.overlay
        width: Math.min(scr.width - 64, 520)
        title: qsTr("Нужно разрешение macOS")
        standardButtons: Dialog.NoButton
        contentItem: ColumnLayout {
            spacing: 12
            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: qsTr("Для записи системного звука macOS требует разрешение «Запись экрана» (Screen Recording). Несмотря на название, Meeting Assistant записывает только звук и никогда не делает снимки экрана.")
            }
        }
        footer: DialogButtonBox {
            Button {
                text: qsTr("Отмена")
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
            Button {
                text: qsTr("Открыть настройки")
                highlighted: true
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                onClicked: Qt.openUrlExternally(scr.screenRecordingSettingsUrl)
            }
        }
    }

    FileDialog {
        id: filePicker
        title: qsTr("Выберите аудиофайл")
        nameFilters: [qsTr("Аудио (*.wav *.mp3 *.m4a *.flac *.ogg)"), qsTr("Все файлы (*)")]
        onAccepted: scr.importPath(selectedFile.toString().replace(/^file:\/\//, ""), true)
    }

    // ── "Из папки" scan dialog ────────────────────────────────────────────────
    ListModel { id: scanModel }
    Dialog {
        id: scanDialog
        modal: true
        anchors.centerIn: Overlay.overlay
        width: Math.min(scr.width - 48, 640)
        height: Math.min(scr.height - 64, 520)
        title: qsTr("Импорт из папки")
        property bool loading: false
        property string errorText: ""

        function openScan() {
            errorText = ""
            loading = true
            scanModel.clear()
            scr.store.scanFolder("")   // server default = recordings_dir
            open()
        }

        contentItem: ColumnLayout {
            spacing: 10
            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                opacity: 0.7
                text: qsTr("Аудиофайлы в каталоге встреч, ещё не добавленные как встречи. Отмеченные будут зарегистрированы на месте и поставлены в очередь на транскрипцию.")
            }
            BusyIndicator { Layout.alignment: Qt.AlignHCenter; running: scanDialog.loading; visible: scanDialog.loading }
            Label {
                Layout.fillWidth: true
                visible: scanDialog.errorText.length > 0
                color: scr.palette.toolTipText
                wrapMode: Text.WordWrap
                text: scanDialog.errorText
            }
            Label {
                Layout.fillWidth: true
                visible: !scanDialog.loading && scanModel.count === 0 && scanDialog.errorText.length === 0
                opacity: 0.7
                text: qsTr("Новых файлов не найдено.")
            }
            ListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: scanModel
                ScrollBar.vertical: ScrollBar {}
                delegate: CheckDelegate {
                    width: ListView.view.width
                    text: model.name
                    checked: model.checked
                    onToggled: scanModel.setProperty(index, "checked", checked)
                }
            }
        }
        footer: DialogButtonBox {
            Button {
                text: qsTr("Отмена")
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
            Button {
                text: qsTr("Импортировать выбранные")
                highlighted: true
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                onClicked: {
                    for (var i = 0; i < scanModel.count; ++i) {
                        var it = scanModel.get(i)
                        if (it.checked)
                            scr.store.importFile(it.path, false, true) // register in place
                    }
                    scr.store.refresh()
                }
            }
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

    // ── idle (with drag&drop overlay) ─────────────────────────────────────────
    Item {
        visible: scr.st === "idle"
        anchors.fill: parent

        ColumnLayout {
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
                    checked: scr.source === "mic"
                    onCheckedChanged: if (checked) scr.source = "mic"
                }
                RadioButton {
                    text: qsTr("Система"); ButtonGroup.group: srcGroup
                    checked: scr.source === "system"
                    onCheckedChanged: if (checked) scr.source = "system"
                }
                RadioButton {
                    text: qsTr("Оба"); ButtonGroup.group: srcGroup
                    checked: scr.source === "mixed"
                    onCheckedChanged: if (checked) scr.source = "mixed"
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
                    checked: scr.echoCancel
                    onToggled: scr.echoCancel = checked
                }
            }

            // drop zone
            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                radius: 8
                color: dropArea.containsDrag ? scr.palette.highlight : "transparent"
                opacity: dropArea.containsDrag ? 0.2 : 1.0
                border.width: 1
                border.color: scr.palette.mid
                Label {
                    anchors.centerIn: parent
                    opacity: 0.6
                    horizontalAlignment: Text.AlignHCenter
                    text: qsTr("Перетащите сюда аудиофайл,\nчтобы импортировать")
                }
                DropArea {
                    id: dropArea
                    anchors.fill: parent
                    keys: ["text/uri-list"]
                    onDropped: function (drop) {
                        if (drop.hasUrls && drop.urls.length > 0) {
                            scr.importPath(drop.urls[0].toString().replace(/^file:\/\//, ""), true)
                            drop.accept()
                        }
                    }
                }
            }

            Button {
                Layout.fillWidth: true
                text: qsTr("Начать запись")
                highlighted: true
                onClicked: scr.start(nameField.text)
            }
            RowLayout {
                Layout.fillWidth: true
                Button {
                    Layout.fillWidth: true
                    text: qsTr("Импорт файла…")
                    onClicked: filePicker.open()
                }
                Button {
                    Layout.fillWidth: true
                    text: qsTr("Из папки…")
                    onClicked: scanDialog.openScan()
                }
            }
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

    // ── stopping / importing ──────────────────────────────────────────────────
    ColumnLayout {
        visible: scr.st === "stopping" || scr.st === "importing"
        anchors.centerIn: parent
        spacing: 16
        BusyIndicator { Layout.alignment: Qt.AlignHCenter; running: true }
        Label {
            Layout.alignment: Qt.AlignHCenter
            text: scr.st === "importing" ? qsTr("Импорт файла…") : qsTr("Сохранение записи…")
            opacity: 0.7
        }
    }

    // ── error ───────────────────────────────────────────────────────────────
    ColumnLayout {
        visible: scr.st === "error"
        anchors.centerIn: parent
        width: Math.min(parent.width - 64, 560)
        spacing: 16
        Label {
            Layout.fillWidth: true
            text: scr.errorMsg
            color: scr.palette.toolTipText
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
        }
        Button {
            Layout.alignment: Qt.AlignHCenter
            text: qsTr("Попробовать снова")
            onClicked: scr.st = "idle"
        }
    }
}
