// Ready-state shell: persistent meeting sidebar + a StackView content pane.
// Sidebar restyled to the Meety design (qt-redesign Phase 3): wordmark + local
// search filter + date-grouped rows + footer. Navigation logic unchanged.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: shell

    // Owns the list state machine + in-session protocol cache.
    MeetingStore { id: store }

    property string selectedId: ""
    property string query: ""

    // AppShell is instantiated eagerly by the StackLayout in Main.qml — well
    // before the sidecar finishes its handshake/health gate. Refreshing on
    // Component.onCompleted hits an unconfigured ApiClient. Gate on
    // api.configured (not sidecar.state): SidecarManager emits stateChanged
    // *before* the ready() signal that drives ApiClient::configure, so listening
    // to state would still fire one tick too early.
    Component.onCompleted: {
        if (api.configured)
            store.refresh()
    }

    Connections {
        target: api
        function onConfiguredChanged() {
            if (api.configured)
                store.refresh()
        }
    }

    // ── navigation (always reset to the list root, then push target) ─────────
    function showList() {
        selectedId = ""
        stack.pop(null)
    }
    function showDetail(m) {
        selectedId = m.id
        stack.pop(null)
        stack.push(detailComp, {
            "shell": shell, "store": store,
            "meetingId": m.id, "meetingName": m.name,
            "audioPath": m.audio_path, "hasTranscript": m.has_transcript === true,
            "createdAt": m.created_at
        })
    }
    function showNewRecording() {
        selectedId = ""
        stack.pop(null)
        stack.push(newRecComp, { "shell": shell, "store": store })
    }
    function showGenerate(m, autoStart) {
        selectedId = m.id
        stack.pop(null)
        stack.push(genComp, {
            "shell": shell, "store": store,
            "meetingId": m.id, "meetingName": m.name,
            "audioPath": m.audio_path, "hasTranscript": m.has_transcript === true,
            "autoStart": autoStart === true
        })
    }
    function showSettings() {
        selectedId = ""
        stack.pop(null)
        stack.push(settingsComp, { "shell": shell })
    }
    function showDiagnostics() {
        selectedId = ""
        stack.pop(null)
        stack.push(diagComp, { "shell": shell })
    }

    Component { id: detailComp;   MeetingDetailScreen {} }
    Component { id: newRecComp;   NewRecordingScreen {} }
    Component { id: genComp;      GenerateProtocolScreen {} }
    Component { id: settingsComp; SettingsScreen {} }
    Component { id: diagComp;     DiagnosticsScreen {} }

    // ── date helpers (port of Sidebar.jsx groupByDate / formatTime) ──────────
    function _sameDay(a, b) {
        return a.getFullYear() === b.getFullYear()
            && a.getMonth() === b.getMonth()
            && a.getDate() === b.getDate()
    }
    function rowTime(unixSec) {
        var d = new Date((unixSec || 0) * 1000)
        var now = new Date()
        if (_sameDay(d, now)) return Qt.formatTime(d, "HH:mm")
        if ((now - d) / 86400000 < 7) return Qt.formatDateTime(d, "ddd")
        return Qt.formatDateTime(d, "d MMM")
    }
    function computeGroups(meetings, q) {
        var list = meetings || []
        if (q && q.trim().length) {
            var qq = q.toLowerCase()
            list = list.filter(function (m) {
                return (m.name || "").toLowerCase().indexOf(qq) >= 0
            })
        }
        list = list.slice().sort(function (a, b) {
            return (b.created_at || 0) - (a.created_at || 0)
        })
        var now = new Date()
        var yesterday = new Date(now.getTime() - 86400000)
        var today = [], yest = [], week = [], earlier = []
        for (var i = 0; i < list.length; ++i) {
            var d = new Date((list[i].created_at || 0) * 1000)
            if (_sameDay(d, now)) today.push(list[i])
            else if (_sameDay(d, yesterday)) yest.push(list[i])
            else if ((now - d) / 86400000 < 7) week.push(list[i])
            else earlier.push(list[i])
        }
        var g = []
        if (today.length) g.push({ "label": qsTr("Сегодня"), "items": today })
        if (yest.length) g.push({ "label": qsTr("Вчера"), "items": yest })
        if (week.length) g.push({ "label": qsTr("На этой неделе"), "items": week })
        if (earlier.length) g.push({ "label": qsTr("Ранее"), "items": earlier })
        return g
    }
    readonly property var filteredGroups: computeGroups(store.meetings, query)
    readonly property bool noMatches: store.status === "success"
                                      && filteredGroups.length === 0

    RowLayout {
        anchors.fill: parent
        spacing: 0

        // ── sidebar ─────────────────────────────────────────────────────────
        Rectangle {
            Layout.preferredWidth: Theme.sidebarWidth
            Layout.fillHeight: true
            color: Theme.paperSub

            // right hairline
            Rectangle {
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                width: 1
                color: Theme.rule
            }

            ColumnLayout {
                anchors.fill: parent
                spacing: 0

                // header: wordmark + brand + refresh + new
                RowLayout {
                    Layout.fillWidth: true
                    Layout.leftMargin: 16
                    Layout.rightMargin: 12
                    Layout.topMargin: 14
                    Layout.bottomMargin: 10
                    spacing: 8

                    MeetyWordmark { size: 26 }
                    Text {
                        text: "meety"
                        font.family: Theme.fontSerif
                        font.weight: Theme.wMedium
                        font.pixelSize: 22
                        font.letterSpacing: Theme.tracking(22, -0.02)
                        color: Theme.ink
                    }
                    Item { Layout.fillWidth: true }
                    MeetyIconButton {
                        iconName: "refresh"
                        ToolTip.text: qsTr("Обновить"); ToolTip.visible: hovered
                        onClicked: store.refresh()
                    }
                    MeetyIconButton {
                        iconName: "plus"; iconSize: 17
                        ToolTip.text: qsTr("Новая запись"); ToolTip.visible: hovered
                        onClicked: shell.showNewRecording()
                    }
                }

                // search (local filter; the ⌘K palette is deferred)
                MeetyField {
                    Layout.fillWidth: true
                    Layout.leftMargin: 14
                    Layout.rightMargin: 14
                    Layout.bottomMargin: 12
                    placeholderText: qsTr("Поиск встреч…")
                    onTextChanged: shell.query = text
                }

                // list — the four data-states
                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    BusyIndicator {
                        anchors.centerIn: parent
                        running: true
                        visible: store.status === "loading"
                    }

                    Text {
                        anchors.centerIn: parent
                        width: parent.width - 32
                        visible: store.status === "empty" || shell.noMatches
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.WordWrap
                        font.family: Theme.fontSerif
                        font.italic: true
                        font.pixelSize: 13
                        color: Theme.ink4
                        text: shell.noMatches ? qsTr("Ничего не найдено")
                                              : qsTr("Нет встреч")
                    }

                    ColumnLayout {
                        anchors.centerIn: parent
                        width: parent.width - 24
                        visible: store.status === "error"
                        spacing: 8
                        Text {
                            Layout.fillWidth: true
                            horizontalAlignment: Text.AlignHCenter
                            wrapMode: Text.WordWrap
                            font.family: Theme.fontUi
                            font.pixelSize: Theme.fsBody
                            color: Theme.ink2
                            text: qsTr("Ошибка: %1").arg(store.errorMessage)
                        }
                        MeetyButton {
                            Layout.alignment: Qt.AlignHCenter
                            text: qsTr("Повторить")
                            onClicked: store.refresh()
                        }
                    }

                    ScrollView {
                        anchors.fill: parent
                        visible: store.status === "success" && !shell.noMatches
                        clip: true
                        contentWidth: availableWidth
                        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                        ColumnLayout {
                            width: parent.width
                            spacing: 6

                            Repeater {
                                model: shell.filteredGroups
                                delegate: ColumnLayout {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    spacing: 1

                                    // section header
                                    RowLayout {
                                        Layout.fillWidth: true
                                        Layout.leftMargin: 18
                                        Layout.rightMargin: 18
                                        Layout.topMargin: 14
                                        Layout.bottomMargin: 6
                                        MeetySectionLabel {
                                            label: modelData.label
                                            trackingEm: 0.08
                                        }
                                        Item { Layout.fillWidth: true }
                                        Text {
                                            text: modelData.items.length
                                            font.family: Theme.fontUi
                                            font.pixelSize: Theme.fsMicro
                                            font.weight: Theme.wMedium
                                            color: Theme.ink4
                                        }
                                    }

                                    // rows
                                    Repeater {
                                        model: modelData.items
                                        delegate: Rectangle {
                                            id: row
                                            required property var modelData
                                            readonly property bool selected:
                                                shell.selectedId === modelData.id
                                            readonly property bool hasTx:
                                                modelData.has_transcript === true

                                            Layout.fillWidth: true
                                            Layout.leftMargin: 8
                                            Layout.rightMargin: 8
                                            implicitHeight: rowContent.implicitHeight
                                                            + 2 * Theme.rowPy
                                            radius: Theme.rMd
                                            color: selected ? Theme.paper4
                                                 : rowHover.hovered ? Theme.paper3
                                                 : "transparent"
                                            border.width: selected ? 1 : 0
                                            border.color: Theme.rule2

                                            // accent left bar (selected)
                                            Rectangle {
                                                visible: row.selected
                                                x: -4
                                                anchors.verticalCenter: parent.verticalCenter
                                                width: 2.5
                                                height: parent.height - 18
                                                radius: 2
                                                color: Theme.accent
                                            }

                                            HoverHandler { id: rowHover }
                                            TapHandler {
                                                onTapped: shell.showDetail(row.modelData)
                                            }

                                            RowLayout {
                                                id: rowContent
                                                anchors.left: parent.left
                                                anchors.right: parent.right
                                                anchors.verticalCenter: parent.verticalCenter
                                                anchors.leftMargin: 12
                                                anchors.rightMargin: 12
                                                spacing: 10

                                                ColumnLayout {
                                                    Layout.fillWidth: true
                                                    spacing: 2
                                                    Text {
                                                        Layout.fillWidth: true
                                                        text: row.modelData.name
                                                        elide: Text.ElideRight
                                                        font.family: Theme.fontUi
                                                        font.pixelSize: 13
                                                        font.weight: row.selected
                                                                     ? Theme.wSemiBold
                                                                     : Theme.wMedium
                                                        color: Theme.ink
                                                    }
                                                    Text {
                                                        text: row.hasTx
                                                              ? qsTr("транскрипт")
                                                              : qsTr("нет транскрипта")
                                                        font.family: Theme.fontUi
                                                        font.pixelSize: 12
                                                        color: row.hasTx ? Theme.ink3
                                                                         : Theme.warn
                                                    }
                                                }
                                                Text {
                                                    Layout.alignment: Qt.AlignTop
                                                    text: shell.rowTime(row.modelData.created_at)
                                                    font.family: Theme.fontUi
                                                    font.pixelSize: 12
                                                    color: Theme.ink3
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Item { Layout.fillWidth: true; height: 8 }
                        }
                    }
                }

                // footer: sync indicator + diagnostics + settings
                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: 1
                    color: Theme.rule
                }
                RowLayout {
                    Layout.fillWidth: true
                    Layout.leftMargin: 14
                    Layout.rightMargin: 10
                    Layout.topMargin: 8
                    Layout.bottomMargin: 8
                    spacing: 7

                    Rectangle {
                        width: 6; height: 6; radius: 3; color: Theme.ok
                    }
                    Text {
                        text: qsTr("Локально")
                        font.family: Theme.fontUi
                        font.pixelSize: 12
                        color: Theme.ink3
                    }
                    Item { Layout.fillWidth: true }
                    MeetyIconButton {
                        iconName: "cpu"
                        ToolTip.text: qsTr("Диагностика"); ToolTip.visible: hovered
                        onClicked: shell.showDiagnostics()
                    }
                    MeetyIconButton {
                        iconName: "gear"
                        ToolTip.text: qsTr("Настройки"); ToolTip.visible: hovered
                        onClicked: shell.showSettings()
                    }
                }
            }
        }

        // ── content pane ────────────────────────────────────────────────────
        StackView {
            id: stack
            Layout.fillWidth: true
            Layout.fillHeight: true
            initialItem: MeetingListScreen {}
            background: Rectangle { color: Theme.paper }
        }
    }
}
