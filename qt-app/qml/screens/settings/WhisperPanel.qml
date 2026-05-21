// Whisper transcriber settings. Edits scr.draft.transcriber {model_path,
// language, beam_size, n_threads}. (VAD was dropped from the backend DTO in
// Phase 1, so it is intentionally absent here.)
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs

ScrollView {
    id: panel
    property var scr
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
        spacing: 16

        Label {
            Layout.margins: 16
            Layout.bottomMargin: 0
            text: qsTr("Транскрипция")
            font.pixelSize: 18
            font.bold: true
        }

        GroupBox {
            title: qsTr("Модель")
            Layout.fillWidth: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            ColumnLayout {
                anchors.fill: parent
                spacing: 8
                Label {
                    text: qsTr("Путь к файлу модели (пусто = модель ядра по умолчанию)")
                    opacity: 0.7
                }
                RowLayout {
                    Layout.fillWidth: true
                    TextField {
                        id: modelField
                        Layout.fillWidth: true
                        placeholderText: qsTr("По умолчанию")
                        onEditingFinished: { panel.t().model_path = text; scr.touch() }
                    }
                    Button { text: qsTr("Выбрать…"); onClicked: modelPicker.open() }
                }
            }
        }

        GroupBox {
            title: qsTr("Параметры распознавания")
            Layout.fillWidth: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            Layout.bottomMargin: 16
            GridLayout {
                anchors.fill: parent
                columns: 2
                columnSpacing: 16
                rowSpacing: 12

                Label { text: qsTr("Язык") }
                ComboBox {
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
                        currentIndex = i >= 0 ? i : 1 // default to ru
                    }
                    onActivated: { panel.t().language = currentValue; scr.touch() }
                }

                Label { text: qsTr("Размер луча (beam size)") }
                SpinBox {
                    id: beamSpin
                    Layout.fillWidth: true
                    from: 1; to: 8
                    onValueModified: { panel.t().beam_size = value; scr.touch() }
                }

                Label { text: qsTr("Потоки CPU (0 = авто)") }
                SpinBox {
                    id: threadsSpin
                    Layout.fillWidth: true
                    from: 0; to: 64
                    onValueModified: { panel.t().n_threads = value; scr.touch() }
                }
            }
        }
    }
}
