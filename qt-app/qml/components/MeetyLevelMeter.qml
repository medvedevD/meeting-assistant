// A horizontal input-level bar for the device test. `level` is linear 0.0–1.0;
// the fill animates and turns warning-coloured near clipping.
import QtQuick
import MeetingAssistant

Item {
    id: meter
    property real level: 0
    property bool active: false

    implicitHeight: 8
    implicitWidth: 160

    Rectangle {
        anchors.fill: parent
        radius: height / 2
        color: Theme.rule
    }

    Rectangle {
        height: parent.height
        radius: height / 2
        width: parent.width * Math.max(0, Math.min(1, meter.level))
        color: !meter.active ? Theme.ink4
             : meter.level > 0.85 ? Theme.rec
             : Theme.accent
        Behavior on width { NumberAnimation { duration: 90; easing.type: Easing.OutQuad } }
        Behavior on color { ColorAnimation { duration: 120 } }
    }
}
