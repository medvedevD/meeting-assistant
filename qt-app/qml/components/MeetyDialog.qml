import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import MeetingAssistant

Dialog {
    id: control

    default property alias bodyData: body.data
    property int preferredWidth: 460
    property int maximumDialogHeight: Overlay.overlay ? Overlay.overlay.height - 48 : 640

    modal: true
    anchors.centerIn: Overlay.overlay
    width: Math.min(preferredWidth, Overlay.overlay ? Overlay.overlay.width - 48 : preferredWidth)
    padding: 20
    topPadding: 16
    bottomPadding: 20
    standardButtons: Dialog.NoButton

    background: Rectangle {
        radius: Theme.rLg
        color: Theme.paper
        border.width: 1
        border.color: Theme.rule
    }

    header: Item {
        implicitHeight: titleText.implicitHeight + 28

        Text {
            id: titleText
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: 20
            anchors.rightMargin: 20
            anchors.verticalCenter: parent.verticalCenter
            text: control.title
            elide: Text.ElideRight
            font.family: Theme.fontSerif
            font.pixelSize: Theme.fsTitle
            font.weight: Theme.wMedium
            font.letterSpacing: Theme.tracking(Theme.fsTitle, -0.02)
            color: Theme.ink
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 1
            color: Theme.rule
        }
    }

    contentItem: ColumnLayout {
        id: body
        spacing: 12
    }
}
