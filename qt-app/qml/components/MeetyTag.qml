// Port of .tag (pill) and .chip (mono variant). Accent-tinted pill label.
import QtQuick
import MeetingAssistant

Rectangle {
    id: root
    property string text: ""
    property bool mono: false       // .chip uses the mono face

    implicitWidth: label.implicitWidth + 14
    implicitHeight: mono ? 20 : 18
    radius: 999
    color: Theme.accentTint

    Text {
        id: label
        anchors.centerIn: parent
        text: root.text
        font.family: root.mono ? Theme.fontMono : Theme.fontUi
        font.pixelSize: Theme.fsMicro
        font.weight: root.mono ? Theme.wRegular : Theme.wMedium
        color: Theme.accent2
    }
}
