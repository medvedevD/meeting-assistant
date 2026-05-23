// Ghost icon button (.btn-icon.btn-ghost) — a MeetyIcon in a 32×32 hit target
// that tints to ink and fills paper3 on hover.
import QtQuick
import QtQuick.Controls
import MeetingAssistant

AbstractButton {
    id: control
    property string iconName: ""
    property int iconSize: 16

    activeFocusOnTab: true
    implicitWidth: 32
    implicitHeight: 32
    hoverEnabled: true

    background: Rectangle {
        radius: Theme.rMd
        readonly property color idleFill: Qt.rgba(Theme.paper3.r, Theme.paper3.g, Theme.paper3.b, 0)
        color: control.activeFocus ? Theme.focusTint
             : control.hovered ? Theme.paper3 : idleFill
        border.width: control.activeFocus ? 1 : 0
        border.color: Theme.focus
        Behavior on color { ColorAnimation { duration: Theme.durBase } }
    }

    contentItem: Item {
        MeetyIcon {
            anchors.centerIn: parent
            name: control.iconName
            size: control.iconSize
            color: control.activeFocus || control.hovered ? Theme.ink : Theme.ink3
        }
    }
}
