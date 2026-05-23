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
    function load() { echoSwitch.checked = r().echo_cancel === true }
    Component.onCompleted: load()
    Connections { target: scr; function onReseeded() { panel.load() } }

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
