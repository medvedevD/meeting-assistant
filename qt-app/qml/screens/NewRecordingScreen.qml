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
    property int visualTick: 0
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
        return (m < 10 ? "0" : "") + m + ":" + (sec < 10 ? "0" : "") + sec
    }

    function levelHeight(index, total, tick) {
        var center = (total - 1) / 2
        var distance = Math.abs(index - center) / center
        var base = 0.32 + (1.0 - distance) * 0.28
        var pulse = Math.sin(tick * 0.42 + index * 0.72) * 0.18
        var ripple = Math.sin(tick * 0.18 - index * 0.35) * 0.08
        return Math.max(0.2, Math.min(1.0, base + pulse + ripple))
    }

    function sourceIndex() {
        if (source === "mic") return 0
        if (source === "system") return 1
        return 2
    }

    function sourceLabel() {
        if (source === "mic") return qsTr("Микрофон")
        if (source === "system") return qsTr("Система")
        return qsTr("Оба источника")
    }

    function setSourceIndex(index) {
        source = index === 0 ? "mic" : (index === 1 ? "system" : "mixed")
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
            scr.visualTick = 0
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
    Timer {
        running: scr.st === "recording"
        interval: 100
        repeat: true
        onTriggered: scr.visualTick += 1
    }

    MeetyDialog {
        id: permissionDialog
        preferredWidth: 520
        title: qsTr("Нужно разрешение macOS")

        Text {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            lineHeight: 1.35
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBodyLg
            color: Theme.ink2
            text: qsTr("Для записи системного звука macOS требует разрешение «Запись экрана» (Screen Recording). Несмотря на название, Meeting Assistant записывает только звук и никогда не делает снимки экрана.")
        }

        footer: MeetyDialogActions {
            dialog: permissionDialog
            cancelText: qsTr("Отмена")
            confirmText: qsTr("Открыть настройки")
            confirmVariant: "accent"
            confirmIconName: "gear"
            onAccepted: Qt.openUrlExternally(scr.screenRecordingSettingsUrl)
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
    MeetyDialog {
        id: scanDialog
        preferredWidth: 640
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

        Text {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            lineHeight: 1.35
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            color: Theme.ink3
            text: qsTr("Аудиофайлы в каталоге встреч, ещё не добавленные как встречи. Отмеченные будут зарегистрированы на месте и поставлены в очередь на транскрипцию.")
        }

        BusyIndicator {
            Layout.alignment: Qt.AlignHCenter
            running: scanDialog.loading
            visible: scanDialog.loading
        }

        Text {
            Layout.fillWidth: true
            visible: scanDialog.errorText.length > 0
            color: Theme.rec
            wrapMode: Text.WordWrap
            lineHeight: 1.35
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            text: scanDialog.errorText
        }

        Text {
            Layout.fillWidth: true
            visible: !scanDialog.loading && scanModel.count === 0 && scanDialog.errorText.length === 0
            color: Theme.ink3
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            text: qsTr("Новых файлов не найдено.")
        }

        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: scanModel
            ScrollBar.vertical: ScrollBar {}
            delegate: MeetyCheckDelegate {
                width: ListView.view.width
                text: model.name
                checked: model.checked
                onToggled: scanModel.setProperty(index, "checked", checked)
            }
        }

        footer: MeetyDialogActions {
            dialog: scanDialog
            cancelText: qsTr("Отмена")
            confirmText: qsTr("Импортировать выбранные")
            confirmVariant: "accent"
            confirmIconName: "download"
            onAccepted: {
                for (var i = 0; i < scanModel.count; ++i) {
                    var it = scanModel.get(i)
                    if (it.checked)
                        scr.store.importFile(it.path, false, true) // register in place
                }
                scr.store.refresh()
            }
        }
    }

    background: Rectangle { color: Theme.paper }

    // ── idle (with drag&drop overlay) ─────────────────────────────────────────
    Item {
        visible: scr.st === "idle"
        anchors.fill: parent

        ColumnLayout {
            anchors.fill: parent
            spacing: 0

            RowLayout {
                Layout.fillWidth: true
                Layout.leftMargin: 24
                Layout.rightMargin: 24
                Layout.topMargin: 16
                Layout.bottomMargin: 16
                spacing: 12

                MeetyButton {
                    variant: "ghost"
                    iconName: "arrow-left"
                    text: qsTr("Назад")
                    onClicked: scr.shell.showList()
                }
                Text {
                    Layout.fillWidth: true
                    text: qsTr("Новая запись")
                    font.family: Theme.fontSerif
                    font.pixelSize: Theme.fsTitle
                    font.weight: Theme.wMedium
                    font.letterSpacing: Theme.tracking(Theme.fsTitle, -0.02)
                    color: Theme.ink
                    elide: Text.ElideRight
                }
            }

            Rectangle { Layout.fillWidth: true; height: 1; color: Theme.rule }

            ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                contentWidth: availableWidth
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                ColumnLayout {
                    width: Math.min(640, parent.width - 64)
                    anchors.horizontalCenter: parent.horizontalCenter
                    spacing: 0

                    Item { Layout.preferredHeight: 40 }

                    Text {
                        Layout.fillWidth: true
                        text: qsTr("Готовы записать?")
                        font.family: Theme.fontSerif
                        font.pixelSize: Theme.fsH1
                        font.weight: Theme.wMedium
                        font.letterSpacing: Theme.tracking(Theme.fsH1, -0.02)
                        color: Theme.ink
                    }
                    Text {
                        Layout.fillWidth: true
                        Layout.bottomMargin: 28
                        text: qsTr("Дайте встрече название — остальное meety сделает сам.")
                        font.family: Theme.fontSerif
                        font.pixelSize: 16
                        font.italic: true
                        color: Theme.ink2
                        wrapMode: Text.WordWrap
                    }

                    Text {
                        Layout.fillWidth: true
                        Layout.bottomMargin: 8
                        text: qsTr("Название встречи")
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsSmall
                        font.weight: Theme.wSemiBold
                        color: Theme.ink3
                    }
                    MeetyField {
                        id: nameField
                        Layout.fillWidth: true
                        placeholderText: qsTr("Например, «Дейлик с командой Платформы»")
                    }

                    Item { Layout.preferredHeight: 18 }

                    Text {
                        Layout.fillWidth: true
                        Layout.bottomMargin: 8
                        text: qsTr("Источник звука")
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsSmall
                        font.weight: Theme.wSemiBold
                        color: Theme.ink3
                    }
                    MeetySegmented {
                        Layout.fillWidth: true
                        model: [qsTr("Микрофон"), qsTr("Система"), qsTr("Оба")]
                        currentIndex: scr.sourceIndex()
                        onActivated: function (index) { scr.setSourceIndex(index) }
                    }
                    Text {
                        Layout.fillWidth: true
                        Layout.topMargin: 8
                        Layout.bottomMargin: 18
                        text: qsTr("Запись системного звука требует разрешения «Запись экрана» в macOS — meety записывает только звук.")
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsSmall
                        color: Theme.ink3
                        wrapMode: Text.WordWrap
                    }

                    Rectangle { Layout.fillWidth: true; height: 1; color: Theme.rule }
                    RowLayout {
                        Layout.fillWidth: true
                        Layout.topMargin: 14
                        Layout.bottomMargin: 14
                        spacing: 16

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 2
                            Text {
                                Layout.fillWidth: true
                                text: qsTr("Подавление эха")
                                font.family: Theme.fontUi
                                font.pixelSize: Theme.fsBodyLg
                                font.weight: Theme.wMedium
                                color: Theme.ink
                            }
                            Text {
                                Layout.fillWidth: true
                                text: qsTr("Рекомендуется при записи через микрофон.")
                                font.family: Theme.fontUi
                                font.pixelSize: Theme.fsSmall
                                color: Theme.ink3
                                wrapMode: Text.WordWrap
                            }
                        }
                        MeetySwitch {
                            checked: scr.echoCancel
                            onToggled: scr.echoCancel = checked
                        }
                    }
                    Rectangle { Layout.fillWidth: true; height: 1; color: Theme.rule }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.topMargin: 16
                        Layout.bottomMargin: 18
                        implicitHeight: 118
                        radius: Theme.rLg
                        color: dropArea.containsDrag ? Theme.accentTint : Theme.paperSub
                        border.width: 1.5
                        border.color: dropArea.containsDrag || dropMouse.containsMouse
                                      ? Theme.accent : Theme.rule2

                        ColumnLayout {
                            anchors.centerIn: parent
                            width: parent.width - 56
                            spacing: 4
                            Text {
                                Layout.fillWidth: true
                                text: qsTr("Перетащите аудиофайл сюда")
                                horizontalAlignment: Text.AlignHCenter
                                font.family: Theme.fontUi
                                font.pixelSize: Theme.fsBodyLg
                                font.weight: Theme.wSemiBold
                                color: Theme.ink
                            }
                            Text {
                                Layout.fillWidth: true
                                text: qsTr("wav · mp3 · m4a · flac · ogg — meety импортирует и поставит в очередь на транскрипцию")
                                horizontalAlignment: Text.AlignHCenter
                                wrapMode: Text.WordWrap
                                font.family: Theme.fontUi
                                font.pixelSize: Theme.fsBody
                                color: Theme.ink3
                            }
                        }
                        MouseArea {
                            id: dropMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: filePicker.open()
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

                    MeetyButton {
                        Layout.fillWidth: true
                        variant: "accent"
                        large: true
                        iconName: "mic"
                        text: qsTr("Начать запись")
                        onClicked: scr.start(nameField.text)
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        Layout.topMargin: 10
                        spacing: 10
                        MeetyButton {
                            Layout.fillWidth: true
                            iconName: "doc"
                            text: qsTr("Импорт файла…")
                            onClicked: filePicker.open()
                        }
                        MeetyButton {
                            Layout.fillWidth: true
                            iconName: "folder"
                            text: qsTr("Из папки…")
                            onClicked: scanDialog.openScan()
                        }
                    }

                    Item { Layout.preferredHeight: 60 }
                }
            }
        }
    }

    // ── recording ───────────────────────────────────────────────────────────
    Item {
        visible: scr.st === "recording"
        anchors.fill: parent

        ColumnLayout {
            anchors.fill: parent
            spacing: 0

            RowLayout {
                Layout.fillWidth: true
                Layout.leftMargin: 24
                Layout.rightMargin: 24
                Layout.topMargin: 16
                Layout.bottomMargin: 16
                spacing: 12

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Rectangle {
                        width: 8; height: 8; radius: 4
                        color: Theme.rec
                        SequentialAnimation on opacity {
                            running: scr.st === "recording"
                            loops: Animation.Infinite
                            NumberAnimation { from: 1.0; to: 0.35; duration: 700 }
                            NumberAnimation { from: 0.35; to: 1.0; duration: 700 }
                        }
                    }
                    Text {
                        text: qsTr("Запись")
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsSmall
                        font.weight: Theme.wSemiBold
                        font.letterSpacing: Theme.tracking(Theme.fsSmall, 0.18)
                        color: Theme.rec
                    }
                }
                Text {
                    text: qsTr("%1 · %2").arg(scr.sourceLabel()).arg(scr.echoCancel ? qsTr("AEC вкл") : qsTr("AEC выкл"))
                    font.family: Theme.fontMono
                    font.pixelSize: Theme.fsSmall
                    color: Theme.ink3
                }
            }

            Rectangle { Layout.fillWidth: true; height: 1; color: Theme.rule }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.leftMargin: 40
                Layout.rightMargin: 40
                Layout.topMargin: 24
                Layout.bottomMargin: 32
                spacing: 16

                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                }

                RowLayout {
                    Layout.alignment: Qt.AlignHCenter
                    spacing: 10

                    Rectangle {
                        width: 10
                        height: 10
                        radius: 5
                        color: Theme.rec
                        SequentialAnimation on opacity {
                            running: scr.st === "recording"
                            loops: Animation.Infinite
                            NumberAnimation { from: 1.0; to: 0.35; duration: 700 }
                            NumberAnimation { from: 0.35; to: 1.0; duration: 700 }
                        }
                    }
                    Text {
                        text: qsTr("Запись идёт")
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsSmall
                        font.weight: Theme.wSemiBold
                        font.letterSpacing: Theme.tracking(Theme.fsSmall, 0.18)
                        color: Theme.rec
                    }
                }

                Text {
                    Layout.alignment: Qt.AlignHCenter
                    text: scr.fmt(scr.elapsed)
                    font.family: Theme.fontSerif
                    font.pixelSize: 76
                    font.weight: Theme.wRegular
                    font.letterSpacing: Theme.tracking(76, -0.025)
                    color: Theme.ink
                }
                Item {
                    Layout.alignment: Qt.AlignHCenter
                    Layout.preferredWidth: Math.min(320, parent.width * 0.6)
                    Layout.preferredHeight: 34
                    Accessible.ignored: true

                    readonly property int barCount: 23

                    Rectangle {
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width
                        height: 1
                        radius: 0.5
                        color: Theme.rule
                    }
                    Row {
                        anchors.centerIn: parent
                        height: parent.height
                        spacing: 5

                        Repeater {
                            model: parent.parent.barCount
                            Rectangle {
                                anchors.verticalCenter: parent.verticalCenter
                                width: 3
                                height: 4 + 28 * scr.levelHeight(index, parent.parent.barCount, scr.visualTick)
                                radius: 1.5
                                color: index === Math.floor(parent.parent.barCount / 2)
                                       ? Theme.rec
                                       : Qt.rgba(Theme.rec.r, Theme.rec.g, Theme.rec.b, 0.46)
                            }
                        }
                    }
                }
                Text {
                    Layout.alignment: Qt.AlignHCenter
                    Layout.maximumWidth: 520
                    text: qsTr("«%1»").arg(scr.recName)
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.WordWrap
                    font.family: Theme.fontSerif
                    font.pixelSize: 20
                    font.italic: true
                    color: Theme.ink2
                }
                RowLayout {
                    Layout.alignment: Qt.AlignHCenter
                    Layout.topMargin: 4
                    spacing: 10
                    MeetyButton {
                        large: true
                        iconName: "stop"
                        variant: "accent"
                        text: qsTr("Остановить")
                        onClicked: scr.stop()
                    }
                }
                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                }
            }
        }
    }

    // ── stopping / importing ──────────────────────────────────────────────────
    ColumnLayout {
        visible: scr.st === "stopping" || scr.st === "importing"
        anchors.centerIn: parent
        spacing: 16
        BusyIndicator { Layout.alignment: Qt.AlignHCenter; running: true }
        Text {
            Layout.alignment: Qt.AlignHCenter
            text: scr.st === "importing" ? qsTr("Импорт файла…") : qsTr("Сохранение записи…")
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBodyLg
            color: Theme.ink3
        }
    }

    // ── error ───────────────────────────────────────────────────────────────
    ColumnLayout {
        visible: scr.st === "error"
        anchors.centerIn: parent
        width: Math.min(parent.width - 64, 560)
        spacing: 16
        Text {
            Layout.fillWidth: true
            text: scr.errorMsg
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBodyLg
            color: Theme.rec
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
        }
        MeetyButton {
            Layout.alignment: Qt.AlignHCenter
            text: qsTr("Попробовать снова")
            onClicked: scr.st = "idle"
        }
    }
}
