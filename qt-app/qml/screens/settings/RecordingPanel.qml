// Default recording settings. Edits scr.draft.recording {source, echo_cancel}
// — now server-authoritative (decision #13), shared with NewRecordingScreen.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ScrollView {
    id: panel
    property var scr
    clip: true
    contentWidth: availableWidth

    function r() { return scr.draft.recording || (scr.draft.recording = { source: "mic", echo_cancel: false }) }
    function load() {
        var rec = r()
        switch (rec.source) {
        case "mic":    micBtn.checked = true; break
        case "system": sysBtn.checked = true; break
        default:       mixBtn.checked = true
        }
        echoSwitch.checked = rec.echo_cancel === true
    }
    Component.onCompleted: load()
    Connections { target: scr; function onReseeded() { panel.load() } }

    ColumnLayout {
        width: panel.availableWidth
        spacing: 16

        Label {
            Layout.margins: 16
            Layout.bottomMargin: 0
            text: qsTr("Запись по умолчанию")
            font.pixelSize: 18
            font.bold: true
        }

        GroupBox {
            title: qsTr("Источник звука")
            Layout.fillWidth: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            ColumnLayout {
                anchors.fill: parent
                spacing: 8
                ButtonGroup { id: g }
                RadioButton {
                    id: micBtn; text: qsTr("Микрофон"); ButtonGroup.group: g
                    onCheckedChanged: if (checked) { panel.r().source = "mic"; scr.touch() }
                }
                RadioButton {
                    id: sysBtn; text: qsTr("Система"); ButtonGroup.group: g
                    onCheckedChanged: if (checked) { panel.r().source = "system"; scr.touch() }
                }
                RadioButton {
                    id: mixBtn; text: qsTr("Оба"); ButtonGroup.group: g
                    onCheckedChanged: if (checked) { panel.r().source = "mixed"; scr.touch() }
                }
            }
        }

        GroupBox {
            title: qsTr("Обработка")
            Layout.fillWidth: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            Layout.bottomMargin: 16
            RowLayout {
                anchors.fill: parent
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
                    id: echoSwitch
                    onToggled: { panel.r().echo_cancel = checked; scr.touch() }
                }
            }
        }
    }
}
