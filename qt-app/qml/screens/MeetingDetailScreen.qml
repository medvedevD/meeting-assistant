// MeetingDetailScreen behavior port. Protocol view is the app's core output:
// rendered with the built-in Qt MarkdownText path (audit: chosen renderer +
// limits documented). `protocolLoad` has no sidecar route → the in-session
// MeetingStore cache stands in (audit: scoped reimplement).
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Page {
    id: scr
    property var shell
    property var store
    property string meetingId
    property string meetingName
    property string audioPath
    property bool hasTranscript: false
    property double createdAt: 0

    readonly property string protocol: store ? store.protocolFor(meetingId) : ""
    readonly property bool hasProtocol: protocol.length > 0

    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            ToolButton {
                text: qsTr("‹ Назад")
                onClicked: scr.shell.showList()
            }
            Label {
                text: scr.meetingName
                font.bold: true
                elide: Text.ElideRight
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
            }
            ToolButton {
                text: qsTr("Генерировать")
                onClicked: scr.shell.showGenerate({
                    "id": scr.meetingId, "name": scr.meetingName,
                    "audio_path": scr.audioPath,
                    "has_transcript": scr.hasTranscript,
                    "created_at": scr.createdAt
                }, false)
            }
            ToolButton {
                text: qsTr("⟳")
                onClicked: scr.store.refresh()
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // meta header
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 16
            Label {
                Layout.fillWidth: true
                opacity: 0.7
                text: scr.createdAt > 0
                      ? Qt.formatDateTime(new Date(scr.createdAt * 1000),
                                          Qt.locale(), Locale.ShortFormat)
                      : ""
            }
            Label {
                visible: scr.hasTranscript
                text: qsTr("Транскрипт")
                font.pixelSize: 11
                color: scr.palette.highlight
            }
        }
        MenuSeparator { Layout.fillWidth: true }

        // protocol present → markdown render
        ScrollView {
            visible: scr.hasProtocol
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

            TextEdit {
                padding: 24
                readOnly: true
                selectByMouse: true
                wrapMode: TextEdit.Wrap
                textFormat: TextEdit.MarkdownText
                color: scr.palette.text
                text: scr.protocol
            }
        }

        // no protocol → CTA (parity with Compose NoProtocolPane)
        ColumnLayout {
            visible: !scr.hasProtocol
            Layout.fillWidth: true
            Layout.fillHeight: true
            Item { Layout.fillHeight: true }
            Label {
                Layout.alignment: Qt.AlignHCenter
                text: qsTr("Протокол ещё не сгенерирован")
                font.pixelSize: 16
                opacity: 0.7
            }
            Button {
                visible: scr.audioPath.length > 0
                Layout.alignment: Qt.AlignHCenter
                text: qsTr("Сгенерировать протокол")
                highlighted: true
                onClicked: scr.shell.showGenerate({
                    "id": scr.meetingId, "name": scr.meetingName,
                    "audio_path": scr.audioPath,
                    "has_transcript": scr.hasTranscript,
                    "created_at": scr.createdAt
                }, false)
            }
            Label {
                visible: scr.audioPath.length === 0
                Layout.alignment: Qt.AlignHCenter
                opacity: 0.6
                text: qsTr("Запишите встречу, чтобы сгенерировать протокол")
            }
            Item { Layout.fillHeight: true }
        }
    }
}
