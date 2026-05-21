// Storage paths. Edits scr.draft.paths {db, meetings_dir, prompts}. Changing
// the DB path or meetings dir is restart-required (decision #2) — the parent
// screen shows the banner; this panel just collects the values.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs

ScrollView {
    id: panel
    property var scr
    clip: true
    contentWidth: availableWidth

    function p() { return scr.draft.paths || (scr.draft.paths = {}) }
    function load() {
        var paths = p()
        dbField.text = paths.db || ""
        meetingsField.text = paths.meetings_dir || ""
        promptsField.text = paths.prompts || ""
    }
    Component.onCompleted: load()
    Connections { target: scr; function onReseeded() { panel.load() } }

    FileDialog {
        id: dbPicker
        title: qsTr("Выберите файл базы данных")
        nameFilters: [qsTr("SQLite (*.db *.sqlite)"), qsTr("Все файлы (*)")]
        onAccepted: {
            var f = selectedFile.toString().replace(/^file:\/\//, "")
            dbField.text = f; panel.p().db = f; scr.touch()
        }
    }
    FolderDialog {
        id: meetingsPicker
        title: qsTr("Каталог встреч")
        onAccepted: {
            var f = selectedFolder.toString().replace(/^file:\/\//, "")
            meetingsField.text = f; panel.p().meetings_dir = f; scr.touch()
        }
    }
    FolderDialog {
        id: promptsPicker
        title: qsTr("Каталог промптов")
        onAccepted: {
            var f = selectedFolder.toString().replace(/^file:\/\//, "")
            promptsField.text = f; panel.p().prompts = f; scr.touch()
        }
    }

    ColumnLayout {
        width: panel.availableWidth
        spacing: 16

        Label {
            Layout.margins: 16
            Layout.bottomMargin: 0
            text: qsTr("Хранилище")
            font.pixelSize: 18
            font.bold: true
        }

        GroupBox {
            title: qsTr("Пути")
            Layout.fillWidth: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            Layout.bottomMargin: 16
            ColumnLayout {
                anchors.fill: parent
                spacing: 14

                Label { text: qsTr("База данных (требует перезапуска)"); opacity: 0.7 }
                RowLayout {
                    Layout.fillWidth: true
                    TextField {
                        id: dbField
                        Layout.fillWidth: true
                        placeholderText: qsTr("По умолчанию")
                        onEditingFinished: { panel.p().db = text.trim() || null; scr.touch() }
                    }
                    Button { text: qsTr("Выбрать…"); onClicked: dbPicker.open() }
                }

                Label { text: qsTr("Каталог встреч (требует перезапуска)"); opacity: 0.7 }
                RowLayout {
                    Layout.fillWidth: true
                    TextField {
                        id: meetingsField
                        Layout.fillWidth: true
                        placeholderText: qsTr("По умолчанию")
                        onEditingFinished: { panel.p().meetings_dir = text.trim() || null; scr.touch() }
                    }
                    Button { text: qsTr("Выбрать…"); onClicked: meetingsPicker.open() }
                }

                Label { text: qsTr("Каталог промптов"); opacity: 0.7 }
                RowLayout {
                    Layout.fillWidth: true
                    TextField {
                        id: promptsField
                        Layout.fillWidth: true
                        placeholderText: qsTr("По умолчанию")
                        onEditingFinished: { panel.p().prompts = text.trim() || null; scr.touch() }
                    }
                    Button { text: qsTr("Выбрать…"); onClicked: promptsPicker.open() }
                }
            }
        }
    }
}
