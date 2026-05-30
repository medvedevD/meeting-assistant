// Editorial Markdown renderer. The core emits the protocol as Markdown, but the
// design's "editorial" look (serif H1, uppercase tracked H2 + hairline, serif
// body, action-items table) can't come from Qt's MarkdownText. So we parse a
// pragmatic Markdown subset into styled blocks: # / ## / ### / ####, paragraphs,
// blockquotes, fenced code, bullet + numbered lists, GFM pipe tables, and inline
// **bold** / *italic* / `code` / links.
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
    function _unescapePipes(s) {
        return s.replace(/\\\|/g, "|")
    }
    // inline → StyledText html
    function _inline(s) {
        var t = _escape(s)
        var codes = []
        t = t.replace(/`([^`]+)`/g, function (_, code) {
            codes.push('<font face="' + Theme.fontMono + '">' + code + '</font>')
            return "\u0000" + (codes.length - 1) + "\u0000"
        })
        t = t.replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, '<a href="$2">$1</a>')
        t = t.replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>")
        t = t.replace(/__([^_]+)__/g, "<b>$1</b>")
        t = t.replace(/\*([^*]+)\*/g, "<i>$1</i>")
        t = t.replace(/_([^_]+)_/g, "<i>$1</i>")
        t = t.replace(/~~([^~]+)~~/g, "<s>$1</s>")
        t = t.replace(/\u0000(\d+)\u0000/g, function (_, idx) { return codes[parseInt(idx)] })
        return t
    }
    function _inlineMultiline(s) {
        return _inline(s).replace(/\n/g, "<br/>")
    }
    function _isSep(cells) {
        for (var i = 0; i < cells.length; ++i)
            if (!/^:?-{2,}:?$/.test(cells[i])) return false
        return cells.length > 0
    }
    function _cells(rowStr) {
        var parts = []
        var cur = ""
        for (var i = 0; i < rowStr.length; ++i) {
            var ch = rowStr.charAt(i)
            if (ch === "\\" && i + 1 < rowStr.length && rowStr.charAt(i + 1) === "|") {
                cur += "\\|"; i++; continue
            }
            if (ch === "|") {
                parts.push(cur); cur = ""; continue
            }
            cur += ch
        }
        parts.push(cur)
        if (parts.length > 0 && parts[0].trim().length === 0)
            parts.shift()
        if (parts.length > 0 && parts[parts.length - 1].trim().length === 0)
            parts.pop()
        return parts.map(function (c) { return doc._unescapePipes(c.trim()) })
    }
    function _isTableStart(lines, i) {
        if (i + 1 >= lines.length)
            return false
        var first = lines[i].trim()
        var second = lines[i + 1].trim()
        return first.indexOf("|") >= 0 && second.indexOf("|") >= 0
                && doc._isSep(doc._cells(second))
    }
    function _isBlockStart(lines, i) {
        var n = lines[i].trim()
        return n.length === 0 || doc._isTableStart(lines, i)
                || /^(`{3,}|~{3,})/.test(n)
                || /^#{1,6}\s/.test(n)
                || /^>\s?/.test(n)
                || /^[-*]\s/.test(n)
                || /^\d+\.\s/.test(n)
                || /^(-{3,}|\*{3,}|_{3,})$/.test(n.replace(/\s/g, ""))
    }
    function _headingText(s) {
        return s.replace(/\s+#+\s*$/, "")
    }
    function _plain(s) {
        return (s || "")
            .replace(/\*\*([^*]+)\*\*/g, "$1")
            .replace(/__([^_]+)__/g, "$1")
            .replace(/\*([^*]+)\*/g, "$1")
            .replace(/_([^_]+)_/g, "$1")
            .trim()
    }
    function _looksLikeMetaHeading(s) {
        return /^(\*\*)?[^:]{1,28}:(\*\*)?/.test(doc._plain(s))
    }
    function _shape(blocks) {
        var out = []
        var firstHeading = -1
        var firstLevel = 1
        for (var i = 0; i < blocks.length; ++i) {
            if (blocks[i].level) {
                firstHeading = i
                firstLevel = blocks[i].level
                break
            }
        }

        var shift = Math.max(0, firstLevel - 2)
        var leadMeta = []
        var collectingLeadMeta = false
        for (var j = 0; j < blocks.length; ++j) {
            var b = blocks[j]
            if (j === firstHeading && b.level) {
                out.push({ "type": "h1", "level": b.level, "text": b.text })
                collectingLeadMeta = true
                continue
            }

            if (collectingLeadMeta && b.level >= 4 && doc._looksLikeMetaHeading(b.text)) {
                leadMeta.push(b.text)
                continue
            }
            if (collectingLeadMeta && b.type === "hr" && leadMeta.length > 0) {
                out.push({ "type": "meta", "items": leadMeta })
                leadMeta = []
                collectingLeadMeta = false
                continue
            }
            if (collectingLeadMeta && leadMeta.length > 0) {
                out.push({ "type": "meta", "items": leadMeta })
                leadMeta = []
            }
            collectingLeadMeta = false

            if (b.level) {
                var shapedLevel = Math.max(1, Math.min(4, b.level - shift))
                b = { "type": "h" + shapedLevel, "level": b.level, "text": b.text }
            }
            out.push(b)
        }
        if (leadMeta.length > 0)
            out.push({ "type": "meta", "items": leadMeta })
        return out
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

            // fenced code block
            var fence = /^(`{3,}|~{3,})\s*([A-Za-z0-9_-]+)?\s*$/.exec(t)
            if (fence) {
                var marker = fence[1].charAt(0)
                var lang = fence[2] || ""
                var code = []
                i++
                while (i < lines.length && lines[i].trim().indexOf(marker + marker + marker) !== 0) {
                    code.push(lines[i]); i++
                }
                if (i < lines.length)
                    i++
                blocks.push({ "type": "code", "text": code.join("\n"), "lang": lang })
                continue
            }

            // table
            if (doc._isTableStart(lines, i)) {
                var raw = []
                while (i < lines.length && lines[i].trim().indexOf("|") >= 0) {
                    raw.push(doc._cells(lines[i].trim())); i++
                }
                var header = null, body = raw
                if (raw.length >= 2 && doc._isSep(raw[1])) { header = raw[0]; body = raw.slice(2) }
                blocks.push({ "type": "table", "header": header, "rows": body })
                continue
            }
            var heading = /^(#{1,6})\s+(.+)$/.exec(t)
            if (heading) {
                blocks.push({
                    "type": "h" + Math.min(heading[1].length, 4),
                    "level": heading[1].length,
                    "text": doc._headingText(heading[2])
                })
                i++; continue
            }

            if (/^>\s?/.test(t)) {
                var quote = []
                while (i < lines.length && /^>\s?/.test(lines[i].trim())) {
                    quote.push(lines[i].trim().replace(/^>\s?/, "")); i++
                }
                blocks.push({ "type": "quote", "text": quote.join("\n") }); continue
            }

            if (/^\s*[-*]\s+/.test(lines[i])) {
                var items = []
                while (i < lines.length && /^\s*[-*]\s+/.test(lines[i])) {
                    var um = /^(\s*)[-*]\s+(.+)$/.exec(lines[i])
                    items.push({
                        "depth": Math.floor(um[1].replace(/\t/g, "  ").length / 2),
                        "text": um[2]
                    })
                    i++
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
                if (doc._isBlockStart(lines, i))
                    break
                para.push(lines[i].trim()); i++
            }
            blocks.push({ "type": "p", "text": para.join(" ") })
        }
        return blocks
    }

    readonly property var blocks: _shape(_parse(markdown))

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
        id: metaC
        ColumnLayout {
            property var blk
            Layout.fillWidth: true
            Layout.bottomMargin: 34
            spacing: 6
            Repeater {
                model: blk ? blk.items : []
                delegate: Text {
                    required property var modelData
                    Layout.fillWidth: true
                    text: doc._inline(modelData)
                    textFormat: Text.StyledText
                    wrapMode: Text.WordWrap
                    font.family: Theme.fontUi
                    font.pixelSize: Theme.fsSmall
                    color: Theme.ink3
                    lineHeight: 1.35
                }
            }
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
        id: h4C
        Text {
            property var blk
            Layout.fillWidth: true
            Layout.topMargin: 16
            Layout.bottomMargin: 8
            text: blk ? doc._inline(blk.text) : ""
            textFormat: Text.StyledText
            wrapMode: Text.WordWrap
            font.family: Theme.fontSerif
            font.pixelSize: 17
            font.weight: Theme.wSemiBold
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
                    readonly property string rawText: (typeof modelData === "string") ? modelData : modelData.text
                    readonly property int depth: (typeof modelData === "string") ? 0 : modelData.depth
                    readonly property var _task: /^\[([ xX])\]\s+/.exec(rawText)
                    readonly property bool isTask: _task !== null
                    readonly property bool taskChecked: isTask && _task[1].toLowerCase() === "x"
                    readonly property string body: isTask
                        ? rawText.replace(/^\[[ xX]\]\s+/, "") : rawText
                    Layout.fillWidth: true
                    Layout.leftMargin: depth * 22
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
        id: quoteC
        RowLayout {
            property var blk
            Layout.fillWidth: true
            Layout.topMargin: 8
            Layout.bottomMargin: 18
            spacing: 14
            Rectangle {
                Layout.preferredWidth: 3
                Layout.fillHeight: true
                radius: 1.5
                color: Theme.accent
            }
            Text {
                Layout.fillWidth: true
                text: blk ? doc._inlineMultiline(blk.text) : ""
                textFormat: Text.StyledText
                wrapMode: Text.WordWrap
                font.family: Theme.fontSerif
                font.pixelSize: 17
                font.italic: true
                color: Theme.ink2
                lineHeight: 1.45
            }
        }
    }
    Component {
        id: codeC
        Rectangle {
            property var blk
            Layout.fillWidth: true
            Layout.topMargin: 6
            Layout.bottomMargin: 18
            implicitHeight: codeText.implicitHeight + 24
            radius: Theme.r8
            color: Theme.paperSub
            border.width: 1
            border.color: Theme.rule

            Text {
                id: codeText
                anchors.fill: parent
                anchors.margins: 12
                text: blk ? blk.text : ""
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
                font.family: Theme.fontMono
                font.pixelSize: 13
                color: Theme.ink2
                lineHeight: 1.35
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
                           : modelData.type === "meta" ? metaC
                           : modelData.type === "h3" ? h3C
                           : modelData.type === "h4" ? h4C
                           : modelData.type === "ul" ? ulC
                           : modelData.type === "ol" ? olC
                           : modelData.type === "quote" ? quoteC
                           : modelData.type === "code" ? codeC
                           : modelData.type === "table" ? tableC
                           : modelData.type === "hr" ? hrC
                           : pC
            onLoaded: item.blk = modelData
        }
    }
}
