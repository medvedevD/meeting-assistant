// Templates CRUD (Phase 2 backend). List + body editor with New/Save/Rename/
// Delete over REST (/api/v1/templates*). The "default template" selector writes
// to scr.draft.default_template and is persisted by the screen's global Save.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import MeetingAssistant

ScrollView {
    id: panel
    property var scr
    font.family: Theme.fontUi
    font.pixelSize: Theme.fsBody
    clip: true
    contentWidth: availableWidth

    property var templates: []          // [{name, body}]
    property string selected: ""
    property string status: ""

    function bodyOf(name) {
        for (var i = 0; i < templates.length; ++i)
            if (templates[i].name === name) return templates[i].body
        return ""
    }
    function names() { return templates.map(function (t) { return t.name }) }
    function selectedBody() { return bodyOf(selected) }
    function isDefault(name) { return (scr.draft.default_template || "") === name }

    function refresh() { listReq.get("/api/v1/templates") }
    Component.onCompleted: refresh()

    function select(name) {
        selected = name
        editor.text = bodyOf(name)
    }

    Request {
        id: listReq
        onOk: function (j) {
            panel.templates = (j && j.templates) ? j.templates : []
            if (panel.selected.length === 0 && panel.templates.length > 0)
                panel.select(panel.templates[0].name)
            else if (panel.selected.length > 0)
                panel.editor.text = panel.bodyOf(panel.selected)
        }
        onFail: function (s, e) { panel.status = qsTr("Ошибка загрузки шаблонов: %1").arg(e) }
    }
    Request {
        id: saveReq
        onOk: function (j) { panel.status = qsTr("Шаблон сохранён"); panel.refresh() }
        onFail: function (s, e) { panel.status = qsTr("Ошибка сохранения: %1").arg(e) }
    }
    Request {
        id: delReq
        onOk: function (j) {
            panel.status = (j && j.warning)
                ? qsTr("Удалено. %1").arg(j.warning)
                : qsTr("Шаблон удалён")
            panel.selected = ""
            panel.refresh()
        }
        onFail: function (s, e) { panel.status = qsTr("Ошибка удаления: %1").arg(e) }
    }
    Request {
        id: renameReq
        onOk: function (j) { panel.status = qsTr("Переименовано"); panel.refresh() }
        onFail: function (s, e) { panel.status = qsTr("Ошибка переименования: %1").arg(e) }
    }

    MeetyDialog {
        id: nameDialog
        preferredWidth: 380
        title: mode === "new" ? qsTr("Новый шаблон") : qsTr("Переименовать шаблон")
        property string mode: "new"
        onAccepted: {
            var n = nameInput.text.trim()
            if (n.length === 0) return
            if (mode === "new")
                saveReq.put("/api/v1/templates/" + encodeURIComponent(n), { "body": "" })
            else
                renameReq.post("/api/v1/templates/" + encodeURIComponent(panel.selected) + "/rename",
                               { "new_name": n })
            if (mode === "new") panel.selected = n
        }

        MeetyField {
            id: nameInput
            Layout.fillWidth: true
            placeholderText: qsTr("Имя шаблона")
            onAccepted: nameDialog.accept()
        }

        footer: MeetyDialogActions {
            dialog: nameDialog
            cancelText: qsTr("Отмена")
            confirmText: nameDialog.mode === "new" ? qsTr("Создать") : qsTr("Переименовать")
            confirmVariant: "accent"
            confirmIconName: nameDialog.mode === "new" ? "plus" : "edit"
            confirmEnabled: nameInput.text.trim().length > 0
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
            text: qsTr("Шаблоны")
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
            text: qsTr("Каждый шаблон — это инструкция для модели. Можно редактировать prompt и добавлять свои.")
            wrapMode: Text.WordWrap
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            color: Theme.ink3
        }

        SettingsRow {
            title: qsTr("Шаблон по умолчанию")
            help: qsTr("Используется при генерации протокола, если шаблон не выбран явно.")
            MeetyComboBox {
                id: defaultBox
                Layout.fillWidth: true
                property var opts: []
                model: opts
                function sync() {
                    opts = [qsTr("(встроенный)")].concat(panel.names())
                    var cur = scr.draft.default_template || ""
                    var idx = cur.length ? opts.indexOf(cur) : 0
                    currentIndex = idx >= 0 ? idx : 0
                }
                onActivated: {
                    scr.draft.default_template = currentIndex === 0 ? null : currentText
                    scr.touch()
                }
                Component.onCompleted: sync()
                Connections {
                    target: panel
                    function onTemplatesChanged() { defaultBox.sync() }
                }
                Connections {
                    target: scr
                    function onReseeded() { defaultBox.sync() }
                }
            }
        }

        RowLayout {
            Layout.leftMargin: 32
            Layout.rightMargin: 32
            Layout.topMargin: 16
            Layout.bottomMargin: 16
            spacing: 10
            MeetyButton {
                iconName: "plus"
                text: qsTr("Новый шаблон")
                onClicked: { nameDialog.mode = "new"; nameInput.text = ""; nameDialog.open() }
            }
            MeetyButton {
                variant: "ghost"
                iconName: "edit"
                text: qsTr("Переименовать")
                enabled: panel.selected.length > 0
                onClicked: { nameDialog.mode = "rename"; nameInput.text = panel.selected; nameDialog.open() }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.leftMargin: 32
            Layout.rightMargin: 32
            implicitHeight: templateList.implicitHeight + 8
            radius: Theme.rMd
            color: Theme.paperSub
            border.width: 1
            border.color: Theme.rule

            ColumnLayout {
                id: templateList
                anchors.fill: parent
                anchors.margins: 4
                spacing: 4

                Repeater {
                    model: panel.templates
                    delegate: Rectangle {
                        required property var modelData
                        readonly property bool active: panel.selected === modelData.name
                        Layout.fillWidth: true
                        implicitHeight: 64
                        radius: Theme.rSm
                        color: active ? Theme.paper
                             : cardMouse.containsMouse ? Theme.paper3 : "transparent"
                        border.width: active ? 1 : 0
                        border.color: Theme.rule

                        Rectangle {
                            visible: parent.active
                            x: 4; y: 12
                            width: 2
                            height: parent.height - 24
                            radius: 1
                            color: Theme.accent
                        }
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 14
                            anchors.rightMargin: 14
                            spacing: 14
                            Rectangle {
                                Layout.preferredWidth: 36
                                Layout.preferredHeight: 36
                                radius: Theme.rMd
                                color: parent.parent.active ? Theme.accentTint : Theme.paper3
                                MeetyIcon {
                                    anchors.centerIn: parent
                                    name: "doc"
                                    size: 16
                                    color: parent.parent.active ? Theme.accent2 : Theme.ink2
                                }
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2
                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 8
                                    Text {
                                        Layout.fillWidth: true
                                        text: modelData.name
                                        elide: Text.ElideRight
                                        font.family: Theme.fontUi
                                        font.pixelSize: Theme.fsBodyLg
                                        font.weight: Theme.wSemiBold
                                        color: Theme.ink
                                    }
                                    MeetyTag {
                                        visible: panel.isDefault(modelData.name)
                                        text: qsTr("По умолчанию")
                                    }
                                }
                                Text {
                                    Layout.fillWidth: true
                                    text: qsTr("%1 символов prompt").arg((modelData.body || "").length)
                                    elide: Text.ElideRight
                                    font.family: Theme.fontUi
                                    font.pixelSize: Theme.fsSmall
                                    color: Theme.ink3
                                }
                            }
                            Text {
                                text: (modelData.body || "").split(/\r?\n/).length
                                font.family: Theme.fontMono
                                font.pixelSize: Theme.fsSmall
                                color: Theme.ink3
                            }
                        }
                        MouseArea {
                            id: cardMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: panel.select(modelData.name)
                        }
                    }
                }
            }
        }

        Item { Layout.preferredHeight: 16 }

        Rectangle {
            Layout.fillWidth: true
            Layout.leftMargin: 32
            Layout.rightMargin: 32
            Layout.bottomMargin: 80
            implicitHeight: 420
            radius: Theme.rMd
            color: Theme.paperSub
            border.width: 1
            border.color: Theme.rule
            clip: true

            ColumnLayout {
                anchors.fill: parent
                spacing: 0

                RowLayout {
                    Layout.fillWidth: true
                    Layout.leftMargin: 16
                    Layout.rightMargin: 16
                    Layout.topMargin: 14
                    Layout.bottomMargin: 12
                    spacing: 10
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Text {
                            Layout.fillWidth: true
                            text: panel.selected.length ? panel.selected : qsTr("Выберите шаблон")
                            font.family: Theme.fontUi
                            font.pixelSize: Theme.fsBodyLg
                            font.weight: Theme.wSemiBold
                            color: Theme.ink
                            elide: Text.ElideRight
                        }
                        Text {
                            Layout.fillWidth: true
                            text: qsTr("Этот текст отправляется в LLM вместе с транскрипцией")
                            font.family: Theme.fontUi
                            font.pixelSize: Theme.fsSmall
                            color: Theme.ink3
                            elide: Text.ElideRight
                        }
                    }
                    MeetyButton {
                        variant: "ghost"
                        iconName: "trash"
                        text: qsTr("Удалить")
                        enabled: panel.selected.length > 0
                        onClicked: delReq.del("/api/v1/templates/" + encodeURIComponent(panel.selected))
                    }
                    MeetyButton {
                        variant: "ghost"
                        iconName: "check"
                        text: qsTr("Сохранить шаблон")
                        enabled: panel.selected.length > 0
                        onClicked: saveReq.put("/api/v1/templates/" + encodeURIComponent(panel.selected),
                                               { "body": editor.text })
                    }
                }
                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 1
                    color: Theme.rule
                }

                TextArea {
                    id: editor
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    enabled: panel.selected.length > 0
                    selectByMouse: true
                    wrapMode: TextEdit.Wrap
                    placeholderText: qsTr("Текст шаблона (Markdown)")
                    font.family: Theme.fontMono
                    font.pixelSize: Theme.fsSmall
                    color: Theme.ink2
                    selectedTextColor: Theme.ink
                    selectionColor: Theme.accentTint
                    background: Rectangle { color: Theme.paperSub }
                    leftPadding: 20
                    rightPadding: 20
                    topPadding: 16
                    bottomPadding: 16
                }
                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 1
                    color: Theme.rule
                }

                RowLayout {
                    Layout.fillWidth: true
                    Layout.leftMargin: 14
                    Layout.rightMargin: 14
                    Layout.topMargin: 10
                    Layout.bottomMargin: 10
                    spacing: 8
                    MeetyTag { mono: true; text: "{meeting_name}" }
                    MeetyTag { mono: true; text: "{transcript}" }
                    Text {
                        Layout.fillWidth: true
                        text: panel.status.length > 0 ? panel.status : qsTr("подставляются автоматически")
                        horizontalAlignment: Text.AlignRight
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsSmall
                        color: Theme.ink4
                        elide: Text.ElideRight
                    }
                }
            }
        }
    }
}
