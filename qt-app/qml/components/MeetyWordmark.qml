// Port of Wordmark — italic serif lowercase "m" with an accent record-dot
// standing in for the tittle. Brand mark for the sidebar / welcome.
import QtQuick
import MeetingAssistant

Item {
    id: root
    property int size: 28
    readonly property real dotR: Math.max(2.4, size * 0.11)

    implicitHeight: size
    implicitWidth: mark.implicitWidth + dotR + 2

    Text {
        id: mark
        anchors.verticalCenter: parent.verticalCenter
        text: "m"
        font.family: Theme.fontSerif
        font.italic: true
        font.weight: Theme.wMedium
        font.pixelSize: Math.round(root.size * 1.05)
        font.letterSpacing: Theme.tracking(root.size * 1.05, -0.03)
        color: Theme.ink
    }

    Rectangle {
        width: root.dotR * 2
        height: root.dotR * 2
        radius: root.dotR
        color: Theme.accent
        x: mark.x + mark.implicitWidth - root.dotR
        y: Math.round(root.size * 0.06)
    }
}
