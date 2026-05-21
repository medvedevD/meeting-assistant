// Port of .card — paperSub panel with hairline border and large radius.
// Content goes inside as children (Frame's default content item).
import QtQuick
import QtQuick.Controls
import MeetingAssistant

Frame {
    id: root
    padding: 20
    leftPadding: 22
    rightPadding: 22

    background: Rectangle {
        radius: Theme.rLg
        color: Theme.paperSub
        border.width: 1
        border.color: Theme.rule
    }
}
