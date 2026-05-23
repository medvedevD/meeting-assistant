import QtQuick
import QtQuick.Layouts
import MeetingAssistant

Item {
    id: control

    property var dialog
    property string cancelText: qsTr("Отмена")
    property string confirmText: qsTr("ОК")
    property string confirmVariant: "accent"
    property bool confirmEnabled: true
    property bool showCancel: true
    property string confirmIconName: ""

    signal accepted()
    signal rejected()

    implicitHeight: actions.implicitHeight + 20

    RowLayout {
        id: actions
        anchors.right: parent.right
        anchors.rightMargin: 20
        anchors.verticalCenter: parent.verticalCenter
        spacing: 8

        MeetyButton {
            visible: control.showCancel
            text: control.cancelText
            variant: "ghost"
            onClicked: {
                control.rejected()
                if (control.dialog)
                    control.dialog.reject()
            }
        }

        MeetyButton {
            text: control.confirmText
            variant: control.confirmVariant
            iconName: control.confirmIconName
            enabled: control.confirmEnabled
            onClicked: {
                control.accepted()
                if (control.dialog)
                    control.dialog.accept()
            }
        }
    }
}
