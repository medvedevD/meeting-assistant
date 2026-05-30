// Port of .field — single-line text input. bg paper, hairline border that
// darkens to ink3 on focus, ink4 placeholder.
import QtQuick
import QtQuick.Controls
import MeetingAssistant

TextField {
    id: control

    font.family: Theme.fontUi
    font.pixelSize: Theme.fsBodyLg
    color: Theme.ink
    placeholderTextColor: Theme.ink4
    selectionColor: Theme.accentTint
    selectedTextColor: Theme.ink
    activeFocusOnTab: true

    leftPadding: 12
    rightPadding: 12
    topPadding: 8
    bottomPadding: 8

    background: Rectangle {
        implicitWidth: 200
        implicitHeight: 38
        radius: Theme.rMd
        color: Theme.paper
        border.width: 1
        border.color: control.activeFocus ? Theme.focus : Theme.rule
        Behavior on border.color { ColorAnimation { duration: Theme.durBase } }
    }
}
