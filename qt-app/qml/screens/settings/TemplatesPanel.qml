// Templates CRUD (Phase 2 backend). List + body editor with New/Save/Rename/
// Delete over REST (/api/v1/templates*). The "default template" selector writes
// to scr.draft.default_template and is persisted by the screen's global Save
// (it is part of the settings document, decision #7 clears it server-side if
// the referenced template is deleted).
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ScrollView {
    id: panel
    property var scr
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

    Dialog {
        id: nameDialog
        anchors.centerIn: Overlay.overlay
        modal: true
        title: mode === "new" ? qsTr("Новый шаблон") : qsTr("Переименовать шаблон")
        property string mode: "new"   // "new" | "rename"
        standardButtons: Dialog.Ok | Dialog.Cancel
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
        ColumnLayout {
            TextField {
                id: nameInput
                Layout.preferredWidth: 320
                placeholderText: qsTr("Имя шаблона")
            }
        }
    }

    RowLayout {
        width: panel.availableWidth
        height: panel.height
        spacing: 0

        // ── template list ────────────────────────────────────────────────
        ColumnLayout {
            Layout.preferredWidth: 200
            Layout.fillHeight: true
            spacing: 0
            Label {
                Layout.margins: 12
                text: qsTr("Шаблоны")
                font.pixelSize: 16
                font.bold: true
            }
            ListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: panel.templates
                ScrollBar.vertical: ScrollBar {}
                delegate: ItemDelegate {
                    required property var modelData
                    width: ListView.view.width
                    text: modelData.name
                    highlighted: panel.selected === modelData.name
                    onClicked: panel.select(modelData.name)
                }
            }
            RowLayout {
                Layout.fillWidth: true
                Layout.margins: 8
                Button {
                    text: qsTr("Новый")
                    onClicked: { nameDialog.mode = "new"; nameInput.text = ""; nameDialog.open() }
                }
                Button {
                    text: qsTr("Переименовать")
                    enabled: panel.selected.length > 0
                    onClicked: { nameDialog.mode = "rename"; nameInput.text = panel.selected; nameDialog.open() }
                }
            }
        }

        ToolSeparator { Layout.fillHeight: true }

        // ── editor + default selector ────────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.margins: 12
            spacing: 10

            RowLayout {
                Layout.fillWidth: true
                Label { text: qsTr("Шаблон по умолчанию:"); opacity: 0.7 }
                ComboBox {
                    id: defaultBox
                    Layout.fillWidth: true
                    property var opts: [qsTr("(встроенный)")].concat(panel.names())
                    model: opts
                    function sync() {
                        var cur = (scr.draft.default_template || "")
                        var idx = cur.length ? opts.indexOf(cur) : 0
                        currentIndex = idx >= 0 ? idx : 0
                    }
                    onModelChanged: sync()
                    onActivated: {
                        scr.draft.default_template = currentIndex === 0 ? null : currentText
                        scr.touch()
                    }
                    Component.onCompleted: sync()
                    Connections { target: scr; function onReseeded() { defaultBox.sync() } }
                }
            }

            Label {
                text: panel.selected.length ? panel.selected : qsTr("Выберите шаблон")
                font.bold: true
            }
            ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                TextArea {
                    id: editor
                    wrapMode: TextEdit.Wrap
                    selectByMouse: true
                    enabled: panel.selected.length > 0
                    placeholderText: qsTr("Текст шаблона (Markdown)")
                }
            }
            RowLayout {
                Layout.fillWidth: true
                Label { Layout.fillWidth: true; opacity: 0.7; font.pixelSize: 12; text: panel.status }
                Button {
                    text: qsTr("Удалить")
                    enabled: panel.selected.length > 0
                    onClicked: delReq.del("/api/v1/templates/" + encodeURIComponent(panel.selected))
                }
                Button {
                    text: qsTr("Сохранить шаблон")
                    highlighted: true
                    enabled: panel.selected.length > 0
                    onClicked: saveReq.put("/api/v1/templates/" + encodeURIComponent(panel.selected),
                                           { "body": editor.text })
                }
            }
        }
    }
}
