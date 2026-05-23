// Styled numeric input for compact settings rows.
import QtQuick
import QtQuick.Controls.Basic
import MeetingAssistant

SpinBox {
    id: control

    font.family: Theme.fontUi
    font.pixelSize: Theme.fsBodyLg
    font.weight: Theme.wRegular
    hoverEnabled: true
    editable: true
    activeFocusOnTab: true

    leftPadding: 12
    rightPadding: 68
    topPadding: 8
    bottomPadding: 8

    contentItem: TextInput {
        z: 2
        text: control.displayText
        readOnly: !control.editable
        validator: control.validator
        inputMethodHints: Qt.ImhFormattedNumbersOnly
        selectByMouse: control.editable
        font: control.font
        color: control.enabled ? Theme.ink : Theme.ink4
        selectionColor: Theme.accentTint
        selectedTextColor: Theme.ink
        horizontalAlignment: Qt.AlignLeft
        verticalAlignment: Qt.AlignVCenter
        onEditingFinished: {
            if (!acceptableInput)
                return
            var next = control.valueFromText(text, control.locale)
            next = Math.max(control.from, Math.min(control.to, next))
            if (next !== control.value) {
                control.value = next
                control.valueModified()
            }
        }
        Keys.onReturnPressed: editingFinished()
        Keys.onEnterPressed: editingFinished()
    }

    up.indicator: Rectangle {
        x: control.width - 34
        y: 1
        implicitWidth: 33
        implicitHeight: Math.floor((control.height - 2) / 2)
        color: control.up.pressed ? Theme.paper4
             : control.up.hovered ? Theme.paper3 : "transparent"
        radius: Theme.rSm

        MeetyIcon {
            anchors.centerIn: parent
            name: "chevron-up"
            size: 13
            color: control.enabled ? Theme.ink3 : Theme.ink4
        }
    }

    down.indicator: Rectangle {
        x: control.width - 34
        y: Math.ceil(control.height / 2)
        implicitWidth: 33
        implicitHeight: Math.floor((control.height - 2) / 2)
        color: control.down.pressed ? Theme.paper4
             : control.down.hovered ? Theme.paper3 : "transparent"
        radius: Theme.rSm

        MeetyIcon {
            anchors.centerIn: parent
            name: "chevron-down"
            size: 13
            color: control.enabled ? Theme.ink3 : Theme.ink4
        }
    }

    background: Rectangle {
        implicitWidth: 120
        implicitHeight: 38
        radius: Theme.rMd
        color: control.hovered || control.activeFocus ? Theme.paper3 : Theme.paper
        border.width: 1
        border.color: control.activeFocus ? Theme.focus : Theme.rule
        opacity: control.enabled ? 1.0 : 0.5
        Behavior on color { ColorAnimation { duration: Theme.durBase } }
        Behavior on border.color { ColorAnimation { duration: Theme.durBase } }

        Rectangle {
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            anchors.right: parent.right
            anchors.rightMargin: 34
            width: 1
            color: Theme.rule
        }
    }
}
