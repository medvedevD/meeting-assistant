// SettingsScreen — partial reimplement (audit). Recording defaults + default
// template are real inputs to /recordings and /protocols, persisted
// client-side via QtCore.Settings (shared with NewRecording/Generate).
// Core-managed keys have no sidecar route → stated honestly, not faked.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtCore

Page {
    id: scr
    property var shell

    Settings { id: rec; category: "recording"
        property string source: "mixed"
        property bool echoCancel: false }
    Settings { id: proto; category: "protocol"
        property string defaultTemplate: "" }

    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            ToolButton { text: qsTr("‹ Назад"); onClicked: scr.shell.showList() }
            Label {
                text: qsTr("Настройки")
                font.bold: true
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
            }
            Item { Layout.preferredWidth: 64 }
        }
    }

    ScrollView {
        anchors.fill: parent
        clip: true
        ColumnLayout {
            width: scr.width
            spacing: 16

            GroupBox {
                title: qsTr("Запись по умолчанию")
                Layout.fillWidth: true
                Layout.margins: 24
                ColumnLayout {
                    anchors.fill: parent
                    spacing: 12
                    Label { text: qsTr("Источник звука"); opacity: 0.7 }
                    RowLayout {
                        spacing: 16
                        ButtonGroup { id: g }
                        RadioButton {
                            text: qsTr("Микрофон"); ButtonGroup.group: g
                            checked: rec.source === "mic"
                            onCheckedChanged: if (checked) rec.source = "mic"
                        }
                        RadioButton {
                            text: qsTr("Система"); ButtonGroup.group: g
                            checked: rec.source === "system"
                            onCheckedChanged: if (checked) rec.source = "system"
                        }
                        RadioButton {
                            text: qsTr("Оба"); ButtonGroup.group: g
                            checked: rec.source === "mixed"
                            onCheckedChanged: if (checked) rec.source = "mixed"
                        }
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        Label { text: qsTr("Подавление эха"); Layout.fillWidth: true }
                        Switch {
                            checked: rec.echoCancel
                            onToggled: rec.echoCancel = checked
                        }
                    }
                }
            }

            GroupBox {
                title: qsTr("Протокол")
                Layout.fillWidth: true
                Layout.leftMargin: 24
                Layout.rightMargin: 24
                ColumnLayout {
                    anchors.fill: parent
                    spacing: 8
                    Label {
                        text: qsTr("Шаблон по умолчанию (пусто = встроенный)")
                        opacity: 0.7
                    }
                    TextField {
                        Layout.fillWidth: true
                        text: proto.defaultTemplate
                        placeholderText: qsTr("По умолчанию")
                        onEditingFinished: proto.defaultTemplate = text
                    }
                }
            }

            GroupBox {
                title: qsTr("Управляется ядром")
                Layout.fillWidth: true
                Layout.leftMargin: 24
                Layout.rightMargin: 24
                Layout.bottomMargin: 24
                Label {
                    width: parent.width
                    wrapMode: Text.WordWrap
                    opacity: 0.7
                    text: qsTr("Модель Whisper, путь к базе, каталог встреч, " +
                               "промпты, ключ Anthropic и параметры " +
                               "транскрайбера задаются ядром и пока не " +
                               "доступны через API сайдкара. Эти настройки " +
                               "появятся здесь после добавления " +
                               "соответствующего маршрута в ядро.")
                }
            }
        }
    }
}
