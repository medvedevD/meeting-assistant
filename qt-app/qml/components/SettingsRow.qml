import QtQuick
import QtQuick.Layouts
import MeetingAssistant

Item {
    id: root
    property string title: ""
    property string help: ""
    property bool dividerVisible: true
    property int contentMaximumWidth: 420
    default property alias content: slot.data

    Layout.fillWidth: true
    Layout.leftMargin: 32
    Layout.rightMargin: 32
    implicitHeight: Math.max(labelCol.implicitHeight, slot.implicitHeight) + 33

    RowLayout {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.topMargin: 16
        spacing: 24

        ColumnLayout {
            id: labelCol
            Layout.minimumWidth: 220
            Layout.preferredWidth: 220
            Layout.maximumWidth: 220
            Layout.alignment: Qt.AlignTop
            spacing: 4
            Text {
                Layout.fillWidth: true
                text: root.title
                font.family: Theme.fontUi
                font.pixelSize: Theme.fsBodyLg
                font.weight: Theme.wSemiBold
                color: Theme.ink
                wrapMode: Text.WordWrap
            }
            Text {
                Layout.fillWidth: true
                text: root.help
                font.family: Theme.fontUi
                font.pixelSize: Theme.fsBody
                color: Theme.ink3
                wrapMode: Text.WordWrap
            }
        }

        RowLayout {
            id: slot
            Layout.fillWidth: true
            Layout.maximumWidth: root.contentMaximumWidth
            Layout.alignment: Qt.AlignTop
            spacing: 10
        }
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 1
        color: Theme.rule
        visible: root.dividerVisible
    }
}
