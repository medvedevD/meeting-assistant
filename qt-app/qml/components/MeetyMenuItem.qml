// Port of .menu button (+ .is-danger). Row inside a MeetyMenu.
import QtQuick
import QtQuick.Controls
import MeetingAssistant

MenuItem {
    id: control
    property bool danger: false
    property string iconName: ""

    implicitHeight: 32
    leftPadding: 10
    rightPadding: 10
    topPadding: 7
    bottomPadding: 7

    readonly property color _fg: danger ? "#C0341D" : Theme.ink

    contentItem: Row {
        spacing: 10
        opacity: control.enabled ? 1.0 : 0.45
        MeetyIcon {
            visible: control.iconName.length > 0
            anchors.verticalCenter: parent.verticalCenter
            name: control.iconName
            size: 14
            color: control._fg
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: control.text
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            font.weight: Theme.wMedium
            verticalAlignment: Text.AlignVCenter
            color: control._fg
        }
    }

    background: Rectangle {
        radius: Theme.rSm
        color: control.highlighted
               ? (control.danger ? "#FAE3DC" : Theme.paper3)
               : "transparent"
    }
}
