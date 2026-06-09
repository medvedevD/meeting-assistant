// Default recording settings. Edits scr.draft.recording {source, echo_cancel}.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import MeetingAssistant

ScrollView {
    id: panel
    property var scr
    font.family: Theme.fontUi
    font.pixelSize: Theme.fsBody
    clip: true
    contentWidth: availableWidth

    function r() { return scr.draft.recording || (scr.draft.recording = { source: "mic", echo_cancel: false }) }
    function sourceIndex() {
        switch (r().source) {
        case "mic": return 0
        case "system": return 1
        default: return 2
        }
    }
    function setSourceIndex(index) {
        r().source = index === 0 ? "mic" : (index === 1 ? "system" : "mixed")
        scr.touch()
    }

    // ── default-device pickers (mirror NewRecordingScreen) ────────────────────
    function deviceModel(list) {
        var out = [qsTr("По умолчанию")]
        for (var i = 0; i < list.length; ++i) {
            var d = list[i]
            out.push(d.is_default ? qsTr("%1 (по умолчанию)").arg(d.label) : d.label)
        }
        return out
    }
    function deviceIndex(list, id) {
        if (!id || !id.length) return 0
        for (var i = 0; i < list.length; ++i)
            if (list[i].id === id) return i + 1
        return 0
    }
    function deviceIdAt(list, index) {
        return index <= 0 ? "" : (list[index - 1] ? list[index - 1].id : "")
    }

    function load() { echoSwitch.checked = r().echo_cancel === true }
    Component.onCompleted: { load(); AudioDevicesStore.ensureLoaded() }
    Connections { target: scr; function onReseeded() { panel.load() } }

    // ── live device test (level meter) ────────────────────────────────────────
    // Only one leg is metered at a time. Start posts a monitor session; a timer
    // polls its level; stop tears it down. The session is always stopped when the
    // panel is destroyed so no capture thread leaks in the sidecar.
    property string testLeg: ""        // "" | "mic" | "system"
    property string monitorId: ""
    property real testLevel: 0
    property real testDb: -60

    function startTest(leg) {
        if (testLeg === leg) { stopTest(); return }
        stopTest()
        monitorId = "test-" + Date.now() + "-" + Math.floor(Math.random() * 1e6)
        var body = { "id": monitorId, "source": leg }
        if (leg === "mic" && r().mic_device) body.mic_device = r().mic_device
        if (leg === "system" && r().system_device) body.system_device = r().system_device
        testLeg = leg
        testLevel = 0
        testDb = -60
        _testStart.post("/api/v1/audio/monitor", body)
    }
    function stopTest() {
        pollTimer.stop()
        if (monitorId.length) _testStop.del("/api/v1/audio/monitor/" + monitorId)
        monitorId = ""
        testLeg = ""
        testLevel = 0
        testDb = -60
    }
    Component.onDestruction: stopTest()

    property Request _testStart: Request {
        onOk: function (j) { pollTimer.start() }
        onFail: function (s, e) { panel.testLeg = ""; panel.monitorId = "" }
    }
    property Request _testPoll: Request {
        onOk: function (j) {
            panel.testLevel = (j && j.level) || 0
            panel.testDb = (j && j.peak_db !== undefined) ? j.peak_db : -60
        }
        onFail: function (s, e) { /* transient; keep polling */ }
    }
    property Request _testStop: Request {
        onOk: function (j) {}
        onFail: function (s, e) {}
    }
    property Timer pollTimer: Timer {
        interval: 90
        repeat: true
        onTriggered: if (panel.monitorId.length)
                         panel._testPoll.get("/api/v1/audio/monitor/" + panel.monitorId)
    }

    ColumnLayout {
        width: panel.availableWidth
        spacing: 0

        Text {
            Layout.fillWidth: true
            Layout.leftMargin: 32
            Layout.rightMargin: 32
            Layout.topMargin: 32
            text: qsTr("Запись")
            font.family: Theme.fontSerif
            font.pixelSize: 26
            font.weight: Theme.wMedium
            font.letterSpacing: 0
            color: Theme.ink
        }
        Text {
            Layout.fillWidth: true
            Layout.leftMargin: 32
            Layout.rightMargin: 32
            Layout.topMargin: 4
            Layout.bottomMargin: 28
            text: qsTr("Настройки по умолчанию для новых записей. Их можно переопределить перед каждой записью.")
            wrapMode: Text.WordWrap
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            color: Theme.ink3
        }

        SettingsRow {
            title: qsTr("Источник звука")
            help: qsTr("Выберите источник, который будет предложен по умолчанию.")
            MeetySegmented {
                Layout.fillWidth: true
                model: [qsTr("Микрофон"), qsTr("Система"), qsTr("Оба источника")]
                currentIndex: panel.sourceIndex()
                onActivated: function (index) { panel.setSourceIndex(index) }
            }
        }

        SettingsRow {
            title: qsTr("Микрофон по умолчанию")
            help: qsTr("Какой вход использовать, если не выбран другой перед записью.")
            MeetyComboBox {
                Layout.fillWidth: true
                model: panel.deviceModel(AudioDevicesStore.inputs)
                currentIndex: panel.deviceIndex(AudioDevicesStore.inputs, panel.r().mic_device)
                onPressedChanged: if (pressed) AudioDevicesStore.refresh()
                onActivated: function (index) {
                    panel.r().mic_device = panel.deviceIdAt(AudioDevicesStore.inputs, index)
                    if (panel.testLeg === "mic") panel.stopTest()
                    scr.touch()
                }
            }
        }

        SettingsRow {
            title: qsTr("Проверка микрофона")
            help: qsTr("Нажмите и говорите — полоска покажет уровень сигнала.")
            MeetyButton {
                text: panel.testLeg === "mic" ? qsTr("Стоп") : qsTr("Проверить")
                iconName: panel.testLeg === "mic" ? "stop" : "mic"
                onClicked: panel.startTest("mic")
            }
            MeetyLevelMeter {
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignVCenter
                level: panel.testLeg === "mic" ? panel.testLevel : 0
                active: panel.testLeg === "mic"
            }
            Text {
                Layout.preferredWidth: 56
                visible: panel.testLeg === "mic"
                text: qsTr("%1 dB").arg(Math.round(panel.testDb))
                horizontalAlignment: Text.AlignRight
                font.family: Theme.fontMono
                font.pixelSize: Theme.fsSmall
                color: Theme.ink3
            }
        }

        SettingsRow {
            title: qsTr("Системный звук по умолчанию")
            help: AudioDevicesStore.systemSelectable
                  ? qsTr("Какой системный источник использовать по умолчанию.")
                  : qsTr("На macOS выбор конкретного устройства недоступен.")
            visible: AudioDevicesStore.systemSelectable
            MeetyComboBox {
                Layout.fillWidth: true
                model: panel.deviceModel(AudioDevicesStore.outputs)
                currentIndex: panel.deviceIndex(AudioDevicesStore.outputs, panel.r().system_device)
                onPressedChanged: if (pressed) AudioDevicesStore.refresh()
                onActivated: function (index) {
                    panel.r().system_device = panel.deviceIdAt(AudioDevicesStore.outputs, index)
                    if (panel.testLeg === "system") panel.stopTest()
                    scr.touch()
                }
            }
        }

        SettingsRow {
            title: qsTr("Проверка системного звука")
            help: qsTr("Включите воспроизведение — полоска покажет уровень.")
            visible: AudioDevicesStore.systemSelectable
            MeetyButton {
                text: panel.testLeg === "system" ? qsTr("Стоп") : qsTr("Проверить")
                iconName: panel.testLeg === "system" ? "stop" : "play"
                onClicked: panel.startTest("system")
            }
            MeetyLevelMeter {
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignVCenter
                level: panel.testLeg === "system" ? panel.testLevel : 0
                active: panel.testLeg === "system"
            }
            Text {
                Layout.preferredWidth: 56
                visible: panel.testLeg === "system"
                text: qsTr("%1 dB").arg(Math.round(panel.testDb))
                horizontalAlignment: Text.AlignRight
                font.family: Theme.fontMono
                font.pixelSize: Theme.fsSmall
                color: Theme.ink3
            }
        }

        SettingsRow {
            title: qsTr("Подавление эха")
            help: qsTr("Рекомендуется при записи через микрофон.")
            dividerVisible: false
            Item { Layout.fillWidth: true }
            MeetySwitch {
                id: echoSwitch
                onToggled: { panel.r().echo_cancel = checked; scr.touch() }
            }
        }
    }
}
