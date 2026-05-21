// Port of .menu popover container. Use with MeetyMenuItem for rows.
import QtQuick
import QtQuick.Controls
import MeetingAssistant

Menu {
    id: control
    padding: 4
    implicitWidth: 240

    background: Rectangle {
        radius: Theme.rMd
        color: Theme.paper
        border.width: 1
        border.color: Theme.rule
        // shadow-2 approximation
        layer.enabled: true
    }
}
