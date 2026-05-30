import QtQuick
import QtQuick.Controls.Basic
import MeetingAssistant

ToolTip {
    id: control

    property string placement: "bottom"

    delay: 420
    timeout: 4500
    padding: 8
    leftPadding: 10
    rightPadding: 10
    topPadding: 7
    bottomPadding: 7

    x: {
        if (!parent)
            return 0
        if (placement === "right")
            return parent.width + 8
        return Math.round((parent.width - implicitWidth) / 2)
    }
    y: {
        if (!parent)
            return 0
        if (placement === "top")
            return -implicitHeight - 8
        if (placement === "right")
            return Math.round((parent.height - implicitHeight) / 2)
        return parent.height + 8
    }

    contentItem: Text {
        text: control.text
        font.family: Theme.fontUi
        font.pixelSize: Theme.fsSmall
        font.weight: Theme.wMedium
        color: Theme.paper
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        wrapMode: Text.NoWrap
    }

    background: Rectangle {
        radius: Theme.rSm
        color: Theme.ink
        border.width: 1
        border.color: Qt.rgba(Theme.paper.r, Theme.paper.g, Theme.paper.b, 0.14)
    }

    enter: Transition {
        NumberAnimation {
            property: "opacity"
            from: 0
            to: 1
            duration: Theme.durFast
            easing.type: Easing.OutCubic
        }
    }
    exit: Transition {
        NumberAnimation {
            property: "opacity"
            to: 0
            duration: Theme.durFast
            easing.type: Easing.OutCubic
        }
    }
}
