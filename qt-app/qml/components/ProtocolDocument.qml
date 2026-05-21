// Editorial Markdown renderer. The core emits the protocol as Markdown, but the
// design's "editorial" look (serif H1, uppercase tracked H2 + hairline, serif
// body, action-items table) can't come from Qt's MarkdownText. So we parse a
// pragmatic Markdown subset into styled blocks: # / ## / ###, paragraphs,
// bullet + numbered lists, GFM pipe tables, and inline **bold** / *italic* /
// `code`.
import QtQuick
import QtQuick.Layouts
import MeetingAssistant

ColumnLayout {
    id: doc
    property string markdown: ""
    spacing: 0

    function _escape(s) {
        return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
    }
    // inline → StyledText html
    function _inline(s) {
        var t = _escape(s)
        t = t.replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>")
        t = t.replace(/\*([^*]+)\*/g, "<i>$1</i>")
        t = t.replace(/`([^`]+)`/g, '<font face="' + Theme.fontMono + '">$1</font>')
        return t
    }
    function _isSep(cells) {
        for (var i = 0; i < cells.length; ++i)
            if (!/^:?-{2,}:?$/.test(cells[i])) return false
        return cells.length > 0
    }
    function _cells(rowStr) {
        var parts = rowStr.split("|")
        parts = parts.slice(1, parts.length - 1)  // drop edges from leading/trailing pipe
        return parts.map(function (c) { return c.trim() })
    }

    function _parse(md) {
        var lines = (md || "").split(/\r?\n/)
        var blocks = []
        var i = 0
        while (i < lines.length) {
            var t = lines[i].trim()
            if (t.length === 0) { i++; continue }

            // horizontal rule (---, ***, ___)
            if (/^(-{3,}|\*{3,}|_{3,})$/.test(t.replace(/\s/g, ""))) {
                blocks.push({ "type": "hr" }); i++; continue
            }

            // table
            if (t.charAt(0) === "|") {
                var raw = []
                while (i < lines.length && lines[i].trim().charAt(0) === "|") {
                    raw.push(doc._cells(lines[i].trim())); i++
                }
                var header = null, body = raw
                if (raw.length >= 2 && doc._isSep(raw[1])) { header = raw[0]; body = raw.slice(2) }
                blocks.push({ "type": "table", "header": header, "rows": body })
                continue
            }
            if (t.indexOf("### ") === 0) { blocks.push({ "type": "h3", "text": t.substring(4) }); i++; continue }
            if (t.indexOf("## ") === 0)  { blocks.push({ "type": "h2", "text": t.substring(3) }); i++; continue }
            if (t.indexOf("# ") === 0)   { blocks.push({ "type": "h1", "text": t.substring(2) }); i++; continue }

            if (/^[-*]\s+/.test(t)) {
                var items = []
                while (i < lines.length && /^[-*]\s+/.test(lines[i].trim())) {
                    items.push(lines[i].trim().replace(/^[-*]\s+/, "")); i++
                }
                blocks.push({ "type": "ul", "items": items }); continue
            }
            if (/^\d+\.\s+/.test(t)) {
                var nitems = []
                while (i < lines.length && /^\d+\.\s+/.test(lines[i].trim())) {
                    var lt = lines[i].trim()
                    var mm = /^(\d+)\.\s+/.exec(lt)
                    nitems.push({ "num": parseInt(mm[1]), "text": lt.replace(/^\d+\.\s+/, "") })
                    i++
                }
                blocks.push({ "type": "ol", "items": nitems }); continue
            }

            // paragraph — gather until a blank line or a new block starter
            var para = [t]; i++
            while (i < lines.length) {
                var n = lines[i].trim()
                if (n.length === 0 || n.charAt(0) === "|"
                        || /^#{1,3}\s/.test(n) || /^[-*]\s/.test(n) || /^\d+\.\s/.test(n))
                    break
                para.push(n); i++
            }
            blocks.push({ "type": "p", "text": para.join(" ") })
        }
        return blocks
    }

    readonly property var blocks: _parse(markdown)

    // ── block delegates ───────────────────────────────────────────────────────
    Component {
        id: h1C
        Text {
            property var blk
            Layout.fillWidth: true
            Layout.topMargin: 2
            Layout.bottomMargin: 6
            text: blk ? doc._inline(blk.text) : ""
            textFormat: Text.StyledText
            wrapMode: Text.WordWrap
            font.family: Theme.fontSerif
            font.pixelSize: 40
            font.weight: Theme.wMedium
            font.letterSpacing: Theme.tracking(40, -0.025)
            color: Theme.ink
            lineHeight: 1.05
        }
    }
    Component {
        id: h2C
        ColumnLayout {
            property var blk
            Layout.fillWidth: true
            Layout.topMargin: 36
            Layout.bottomMargin: 10
            spacing: 8
            Text {
                Layout.fillWidth: true
                text: blk ? doc._inline(blk.text.toUpperCase()) : ""
                textFormat: Text.StyledText
                wrapMode: Text.WordWrap
                font.family: Theme.fontUi
                font.pixelSize: Theme.fsMicro
                font.weight: Theme.wSemiBold
                font.letterSpacing: Theme.tracking(Theme.fsMicro, 0.12)
                color: Theme.ink3
            }
            Rectangle { Layout.fillWidth: true; height: 1; color: Theme.rule }
        }
    }
    Component {
        id: h3C
        Text {
            property var blk
            Layout.fillWidth: true
            Layout.topMargin: 22
            Layout.bottomMargin: 6
            text: blk ? doc._inline(blk.text) : ""
            textFormat: Text.StyledText
            wrapMode: Text.WordWrap
            font.family: Theme.fontSerif
            font.pixelSize: 19
            font.weight: Theme.wSemiBold
            font.letterSpacing: Theme.tracking(19, -0.01)
            color: Theme.ink
        }
    }
    Component {
        id: pC
        Text {
            property var blk
            Layout.fillWidth: true
            Layout.bottomMargin: 14
            text: blk ? doc._inline(blk.text) : ""
            textFormat: Text.StyledText
            wrapMode: Text.WordWrap
            font.family: Theme.fontSerif
            font.pixelSize: 17
            color: Theme.ink
            lineHeight: 1.45
        }
    }
    Component {
        id: ulC
        ColumnLayout {
            property var blk
            Layout.fillWidth: true
            Layout.bottomMargin: 16
            spacing: 6
            Repeater {
                model: blk ? blk.items : []
                delegate: RowLayout {
                    required property var modelData
                    readonly property var _task: /^\[([ xX])\]\s+/.exec(modelData)
                    readonly property bool isTask: _task !== null
                    readonly property bool taskChecked: isTask && _task[1].toLowerCase() === "x"
                    readonly property string body: isTask
                        ? modelData.replace(/^\[[ xX]\]\s+/, "") : modelData
                    Layout.fillWidth: true
                    spacing: 10

                    // bullet marker
                    Text {
                        visible: !parent.isTask
                        Layout.alignment: Qt.AlignTop
                        text: "•"; color: Theme.ink4
                        font.family: Theme.fontSerif; font.pixelSize: 17
                    }
                    // task checkbox
                    Rectangle {
                        visible: parent.isTask
                        Layout.alignment: Qt.AlignTop
                        Layout.topMargin: 3
                        width: 16; height: 16; radius: 4
                        color: parent.taskChecked ? Theme.accent : "transparent"
                        border.width: 1
                        border.color: parent.taskChecked ? Theme.accent : Theme.rule2
                        MeetyIcon {
                            visible: parent.parent.taskChecked
                            anchors.centerIn: parent
                            name: "check"; size: 11; color: Theme.accentInk
                        }
                    }
                    Text {
                        Layout.fillWidth: true
                        text: doc._inline(parent.body)
                        textFormat: Text.StyledText
                        wrapMode: Text.WordWrap
                        font.family: Theme.fontSerif
                        font.pixelSize: 17
                        color: Theme.ink
                        lineHeight: 1.4
                    }
                }
            }
        }
    }
    Component {
        id: olC
        ColumnLayout {
            property var blk
            Layout.fillWidth: true
            Layout.bottomMargin: 16
            spacing: 6
            Repeater {
                model: blk ? blk.items : []
                delegate: RowLayout {
                    required property var modelData
                    Layout.fillWidth: true
                    spacing: 10
                    Text {
                        Layout.alignment: Qt.AlignTop
                        text: modelData.num + "."
                        color: Theme.ink4
                        font.family: Theme.fontSerif; font.pixelSize: 17
                    }
                    Text {
                        Layout.fillWidth: true
                        text: doc._inline(modelData.text)
                        textFormat: Text.StyledText
                        wrapMode: Text.WordWrap
                        font.family: Theme.fontSerif
                        font.pixelSize: 17
                        color: Theme.ink
                        lineHeight: 1.4
                    }
                }
            }
        }
    }
    Component {
        id: hrC
        Item {
            property var blk
            Layout.fillWidth: true
            Layout.topMargin: 18
            Layout.bottomMargin: 18
            implicitHeight: 1
            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                height: 1
                color: Theme.rule
            }
        }
    }
    Component {
        id: tableC
        ColumnLayout {
            property var blk
            Layout.fillWidth: true
            Layout.topMargin: 6
            Layout.bottomMargin: 20
            spacing: 0

            // header
            RowLayout {
                Layout.fillWidth: true
                visible: blk && blk.header
                spacing: 12
                Repeater {
                    model: blk && blk.header ? blk.header : []
                    delegate: Text {
                        required property var modelData
                        required property int index
                        Layout.fillWidth: index === 0
                        Layout.preferredWidth: index === 0 ? -1 : 120
                        topPadding: 10; bottomPadding: 10
                        text: doc._inline((modelData + "").toUpperCase())
                        textFormat: Text.StyledText
                        wrapMode: Text.WordWrap
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsMicro
                        font.weight: Theme.wSemiBold
                        font.letterSpacing: Theme.tracking(Theme.fsMicro, 0.06)
                        color: Theme.ink3
                    }
                }
            }
            Rectangle { Layout.fillWidth: true; height: 1; color: Theme.rule; visible: blk && blk.header }

            // body rows
            Repeater {
                model: blk ? blk.rows : []
                delegate: ColumnLayout {
                    required property var modelData
                    Layout.fillWidth: true
                    spacing: 0
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12
                        Repeater {
                            model: modelData
                            delegate: Text {
                                required property var modelData
                                required property int index
                                Layout.fillWidth: index === 0
                                Layout.preferredWidth: index === 0 ? -1 : 120
                                Layout.alignment: Qt.AlignTop
                                topPadding: 10; bottomPadding: 10
                                text: doc._inline(modelData + "")
                                textFormat: Text.StyledText
                                wrapMode: Text.WordWrap
                                font.family: Theme.fontUi
                                font.pixelSize: 13
                                color: index === 0 ? Theme.ink : Theme.ink2
                            }
                        }
                    }
                    Rectangle { Layout.fillWidth: true; height: 1; color: Theme.rule }
                }
            }
        }
    }

    Repeater {
        model: doc.blocks
        delegate: Loader {
            required property var modelData
            Layout.fillWidth: true
            sourceComponent: modelData.type === "h1" ? h1C
                           : modelData.type === "h2" ? h2C
                           : modelData.type === "h3" ? h3C
                           : modelData.type === "ul" ? ulC
                           : modelData.type === "ol" ? olC
                           : modelData.type === "table" ? tableC
                           : modelData.type === "hr" ? hrC
                           : pC
            onLoaded: item.blk = modelData
        }
    }
}
