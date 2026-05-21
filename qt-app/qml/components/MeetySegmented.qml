// Port of .segmented — pill-grouped toggle. bg paper3 trough; a single
// highlight rectangle slides between segments (smoother than per-segment color
// swaps, and matches the "lifted active pill" look).
import QtQuick
import MeetingAssistant

Rectangle {
    id: root

    property var model: []          // array of label strings
    property int currentIndex: 0
    signal activated(int index)

    implicitHeight: 30
    implicitWidth: row.implicitWidth + 6
    radius: Theme.rMd
    color: Theme.paper3

    Item {
        anchors.fill: parent
        anchors.margins: 3

        // sliding active pill — tracks the current segment's geometry
        Rectangle {
            readonly property Item seg: rep.count > 0 ? rep.itemAt(root.currentIndex) : null
            visible: seg !== null
            x: seg ? seg.x : 0
            width: seg ? seg.width : 0
            height: parent.height
            radius: 6
            color: Theme.paper
            border.width: 1
            border.color: Theme.rule
            Behavior on x { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
            Behavior on width { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
        }

        Row {
            id: row
            height: parent.height
            spacing: 2

            Repeater {
                id: rep
                model: root.model
                delegate: Item {
                    required property int index
                    required property var modelData
                    readonly property bool active: root.currentIndex === index

                    height: row.height
                    implicitWidth: lbl.implicitWidth + 24

                    Text {
                        id: lbl
                        anchors.centerIn: parent
                        text: modelData
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsBody
                        font.weight: Theme.wMedium
                        color: parent.active ? Theme.ink : Theme.ink2
                        Behavior on color { ColorAnimation { duration: Theme.durBase } }
                    }
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: { root.currentIndex = index; root.activated(index) }
                    }
                }
            }
        }
    }
}
