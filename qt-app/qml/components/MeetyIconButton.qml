// Ghost icon button (.btn-icon.btn-ghost) — a MeetyIcon in a 32×32 hit target
// that tints to ink and fills paper3 on hover.
import QtQuick
import QtQuick.Controls
import MeetingAssistant

AbstractButton {
    id: control
    property string iconName: ""
    property int iconSize: 16

    implicitWidth: 32
    implicitHeight: 32
    hoverEnabled: true

    background: Rectangle {
        radius: Theme.rMd
        color: control.hovered ? Theme.paper3 : "transparent"
        Behavior on color { ColorAnimation { duration: Theme.durBase } }
    }

    contentItem: Item {
        MeetyIcon {
            anchors.centerIn: parent
            name: control.iconName
            size: control.iconSize
            color: control.hovered ? Theme.ink : Theme.ink3
        }
    }
}
