// Empty content pane shown when no meeting is open (Compose MeetingListScreen
// — the list itself lives in the sidebar). Behavior parity, not design copy.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Page {
    ColumnLayout {
        anchors.centerIn: parent
        spacing: 8
        Label {
            Layout.alignment: Qt.AlignHCenter
            text: qsTr("Выберите встречу")
            font.pixelSize: 22
            font.bold: true
        }
        Label {
            Layout.alignment: Qt.AlignHCenter
            opacity: 0.7
            text: qsTr("или создайте новую запись кнопкой ＋ в боковой панели")
        }
    }
}
