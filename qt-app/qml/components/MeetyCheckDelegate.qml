import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import MeetingAssistant

ItemDelegate {
    id: control

    checkable: true
    implicitHeight: 40
    hoverEnabled: true
    activeFocusOnTab: true
    leftPadding: 10
    rightPadding: 10

    contentItem: RowLayout {
        spacing: 10

        Rectangle {
            Layout.preferredWidth: 18
            Layout.preferredHeight: 18
            radius: Theme.rSm
            color: control.checked ? Theme.accent : Theme.paper
            border.width: 1
            border.color: control.checked ? Theme.accent : Theme.rule2

            MeetyIcon {
                anchors.centerIn: parent
                visible: control.checked
                name: "check"
                size: 12
                color: Theme.accentInk
                strokeWidth: 2
            }
        }

        Text {
            Layout.fillWidth: true
            text: control.text
            elide: Text.ElideMiddle
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            color: control.enabled ? Theme.ink : Theme.ink4
        }
    }

    background: Rectangle {
        radius: Theme.rSm
        color: control.highlighted || control.hovered || control.activeFocus
               ? Theme.paper3
               : "transparent"
        border.width: control.activeFocus ? 1 : 0
        border.color: Theme.focus
    }

}
