// Storage paths. Edits scr.draft.paths {db, meetings_dir, prompts}.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import MeetingAssistant

ScrollView {
    id: panel
    property var scr
    font.family: Theme.fontUi
    font.pixelSize: Theme.fsBody
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
        spacing: 0

        Text {
            Layout.fillWidth: true
            Layout.leftMargin: 32
            Layout.rightMargin: 32
            Layout.topMargin: 32
            text: qsTr("Хранилище")
            font.family: Theme.fontSerif
            font.pixelSize: 26
            font.weight: Theme.wMedium
            font.letterSpacing: 0
            color: Theme.ink
        }
        Text {
            Layout.fillWidth: true
            Layout.leftMargin: 32
            Layout.rightMargin: 32
            Layout.topMargin: 4
            Layout.bottomMargin: 28
            text: qsTr("Где meety хранит базу, аудио, транскрипты и prompt-шаблоны.")
            wrapMode: Text.WordWrap
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            color: Theme.ink3
        }

        SettingsRow {
            title: qsTr("Папка встреч")
            help: qsTr("Аудиозаписи, транскрипты и сгенерированные протоколы. Изменение требует перезапуска.")
            MeetyField {
                id: meetingsField
                Layout.fillWidth: true
                placeholderText: qsTr("По умолчанию")
                onEditingFinished: { panel.p().meetings_dir = text.trim() || null; scr.touch() }
            }
            MeetyButton {
                iconName: "folder"
                text: qsTr("Выбрать")
                onClicked: meetingsPicker.open()
            }
        }

        SettingsRow {
            title: qsTr("База данных")
            help: qsTr("SQLite-файл. Изменение пути требует перезапуска приложения.")
            MeetyField {
                id: dbField
                Layout.fillWidth: true
                placeholderText: qsTr("По умолчанию")
                onEditingFinished: { panel.p().db = text.trim() || null; scr.touch() }
            }
            MeetyButton {
                iconName: "folder"
                text: qsTr("Выбрать")
                onClicked: dbPicker.open()
            }
        }

        SettingsRow {
            title: qsTr("Каталог промптов")
            help: qsTr("Папка с Markdown-шаблонами для генерации протоколов.")
            dividerVisible: false
            MeetyField {
                id: promptsField
                Layout.fillWidth: true
                placeholderText: qsTr("По умолчанию")
                onEditingFinished: { panel.p().prompts = text.trim() || null; scr.touch() }
            }
            MeetyButton {
                iconName: "folder"
                text: qsTr("Выбрать")
                onClicked: promptsPicker.open()
            }
        }
    }
}
