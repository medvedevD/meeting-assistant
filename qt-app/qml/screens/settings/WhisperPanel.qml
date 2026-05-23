// Whisper transcriber settings. Edits scr.draft.transcriber.
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

    function t() { return scr.draft.transcriber || (scr.draft.transcriber = {}) }
    function load() {
        var tr = t()
        modelField.text   = tr.model_path || ""
        langBox.setValue(tr.language || "ru")
        beamSpin.value    = tr.beam_size || 1
        threadsSpin.value = tr.n_threads || 0
    }
    Component.onCompleted: load()
    Connections { target: scr; function onReseeded() { panel.load() } }

    FileDialog {
        id: modelPicker
        title: qsTr("Выберите файл модели Whisper")
        nameFilters: [qsTr("Модель Whisper (*.bin)"), qsTr("Все файлы (*)")]
        onAccepted: {
            var p = selectedFile.toString().replace(/^file:\/\//, "")
            modelField.text = p
            panel.t().model_path = p
            scr.touch()
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
            text: qsTr("Транскрипция")
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
            text: qsTr("Whisper работает локально на вашем устройстве. Аудио никогда не покидает компьютер.")
            wrapMode: Text.WordWrap
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            color: Theme.ink3
        }

        SettingsRow {
            title: qsTr("Файл модели")
            help: qsTr("Пусто — использовать модель ядра по умолчанию.")
            MeetyField {
                id: modelField
                Layout.fillWidth: true
                placeholderText: qsTr("По умолчанию")
                onEditingFinished: {
                    var path = text.trim()
                    panel.t().model_path = path.length > 0 ? path : null
                    scr.touch()
                }
            }
            MeetyButton {
                iconName: "folder"
                text: qsTr("Выбрать")
                onClicked: modelPicker.open()
            }
        }

        SettingsRow {
            title: qsTr("Язык")
            help: qsTr("«Автоопределение» выберет язык по содержимому.")
            MeetyComboBox {
                id: langBox
                Layout.fillWidth: true
                textRole: "label"
                valueRole: "code"
                model: [
                    { code: "auto", label: qsTr("Автоопределение") },
                    { code: "ru",   label: qsTr("Русский") },
                    { code: "en",   label: qsTr("Английский") },
                    { code: "de",   label: qsTr("Немецкий") },
                    { code: "fr",   label: qsTr("Французский") },
                    { code: "es",   label: qsTr("Испанский") }
                ]
                function setValue(code) {
                    var i = indexOfValue(code)
                    currentIndex = i >= 0 ? i : 1
                }
                onActivated: { panel.t().language = currentValue; scr.touch() }
            }
        }

        SettingsRow {
            title: qsTr("Beam size")
            help: qsTr("Больше — потенциально точнее, но медленнее.")
            MeetySpinBox {
                id: beamSpin
                Layout.fillWidth: true
                from: 1; to: 8
                onValueModified: { panel.t().beam_size = value; scr.touch() }
            }
        }

        SettingsRow {
            title: qsTr("Потоки CPU")
            help: qsTr("0 — автоматический выбор.")
            dividerVisible: false
            MeetySpinBox {
                id: threadsSpin
                Layout.fillWidth: true
                from: 0; to: 64
                onValueModified: { panel.t().n_threads = value; scr.touch() }
            }
        }
    }
}
