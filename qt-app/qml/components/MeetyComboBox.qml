// Styled ComboBox for the redesign. Keeps the native ComboBox API while
// replacing the Fusion visuals with the meety field/menu treatment.
import QtQuick
import QtQuick.Controls.Basic
import MeetingAssistant

ComboBox {
    id: control

    font.family: Theme.fontUi
    font.pixelSize: Theme.fsBodyLg
    font.weight: Theme.wRegular
    hoverEnabled: true
    activeFocusOnTab: true

    leftPadding: 12
    rightPadding: 36
    topPadding: 8
    bottomPadding: 8

    delegate: ItemDelegate {
        id: item
        required property int index

        width: control.popup.width
        implicitHeight: 34
        hoverEnabled: true

        contentItem: Text {
            text: control.textAt(item.index)
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            font.weight: control.currentIndex === item.index ? Theme.wSemiBold : Theme.wRegular
            color: control.currentIndex === item.index ? Theme.ink : Theme.ink2
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
        }

        background: Rectangle {
            radius: Theme.rSm
            color: control.currentIndex === item.index ? Theme.paper4
                 : item.hovered ? Theme.paper3 : "transparent"
        }
    }

    indicator: MeetyIcon {
        x: control.width - width - 12
        y: control.topPadding + (control.availableHeight - height) / 2
        name: control.popup.visible ? "chevron-up" : "chevron-down"
        size: 15
        color: control.enabled ? Theme.ink3 : Theme.ink4
    }

    contentItem: TextInput {
        text: control.editable ? control.editText : control.displayText
        enabled: control.editable
        readOnly: !control.editable
        selectByMouse: control.editable
        clip: true
        font: control.font
        color: control.enabled ? Theme.ink : Theme.ink4
        selectionColor: Theme.accentTint
        selectedTextColor: Theme.ink
        verticalAlignment: TextInput.AlignVCenter
        leftPadding: 0
        rightPadding: 0
        onTextEdited: control.editText = text
        Keys.onReturnPressed: control.accepted()
        Keys.onEnterPressed: control.accepted()
    }

    background: Rectangle {
        implicitWidth: 200
        implicitHeight: 38
        radius: Theme.rMd
        color: control.hovered || control.popup.visible ? Theme.paper3 : Theme.paper
        border.width: 1
        border.color: control.activeFocus ? Theme.focus
                    : control.popup.visible ? Theme.ink3 : Theme.rule
        opacity: control.enabled ? 1.0 : 0.5
        Behavior on color { ColorAnimation { duration: Theme.durBase } }
        Behavior on border.color { ColorAnimation { duration: Theme.durBase } }
    }

    popup: Popup {
        y: control.height + 6
        width: control.width
        implicitHeight: Math.min(contentItem.implicitHeight + 8, 260)
        padding: 4

        contentItem: ListView {
            clip: true
            implicitHeight: contentHeight
            model: control.popup.visible ? control.delegateModel : null
            currentIndex: control.highlightedIndex
        }

        background: Rectangle {
            radius: Theme.rMd
            color: Theme.paper
            border.width: 1
            border.color: Theme.rule
        }
    }
}
