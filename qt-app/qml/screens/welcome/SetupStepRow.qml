// One step of the first-run setup: a numbered badge, a title, a one-line
// summary of where the step stands, and the controls that complete it. The
// controls fold away once the step is done, so a finished step is one line.
import QtQuick
import QtQuick.Layouts
import MeetingAssistant

ColumnLayout {
    id: row

    property string number
    property string title
    // "todo" | "busy" | "done" | "attention"
    property string status: "todo"
    // What the step is waiting for, or what it found.
    property string summary
    // Items placed inside the row are its controls.
    default property alias controls: body.data

    spacing: 8

    RowLayout {
        Layout.fillWidth: true
        spacing: 12

        Rectangle {
            Layout.preferredWidth: 22
            Layout.preferredHeight: 22
            Layout.alignment: Qt.AlignTop
            Layout.topMargin: 1
            radius: 11
            color: row.status === "done" ? Theme.ok
                 : row.status === "attention" ? Theme.warn
                 : Theme.accentTint

            MeetyIcon {
                anchors.centerIn: parent
                visible: row.status === "done"
                name: "check"
                size: 13
                strokeWidth: 2.2
                color: Theme.accentInk
            }
            Text {
                anchors.centerIn: parent
                visible: row.status !== "done"
                text: row.number
                font.family: Theme.fontUi
                font.pixelSize: Theme.fsMicro
                font.weight: Theme.wBold
                color: row.status === "attention" ? Theme.accentInk : Theme.accent2
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2

            Text {
                Layout.fillWidth: true
                text: row.title
                wrapMode: Text.WordWrap
                font.family: Theme.fontUi
                font.pixelSize: Theme.fsBodyLg
                font.weight: Theme.wSemiBold
                color: Theme.ink
            }
            Text {
                objectName: "stepSummary"
                Layout.fillWidth: true
                visible: text.length > 0
                text: row.summary
                wrapMode: Text.WordWrap
                lineHeight: 1.4
                font.family: Theme.fontUi
                font.pixelSize: Theme.fsBody
                color: row.status === "attention" ? Theme.ink2 : Theme.ink3
            }
        }
    }

    ColumnLayout {
        id: body
        Layout.fillWidth: true
        Layout.leftMargin: 34
        visible: row.status !== "done"
        spacing: 8
    }
}
