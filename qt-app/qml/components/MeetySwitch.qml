// Port of .switch — 32×18 track, 14px knob, accent when on.
// The control's footprint == the track so the whole visible switch is clickable
// (no stray padding/content area that swallows clicks).
import QtQuick
import QtQuick.Controls
import MeetingAssistant

Switch {
    id: control
    padding: 0
    spacing: 0
    implicitWidth: 32
    implicitHeight: 18

    indicator: Rectangle {
        width: 32
        height: 18
        radius: 9
        color: control.checked ? Theme.accent : Theme.rule2
        Behavior on color { ColorAnimation { duration: Theme.durBase } }

        Rectangle {
            width: 14
            height: 14
            radius: 7
            y: 2
            x: control.checked ? 16 : 2
            color: "#FFFFFF"
            Behavior on x { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
        }
    }

    contentItem: Item { implicitWidth: 0; implicitHeight: 0 }
}
