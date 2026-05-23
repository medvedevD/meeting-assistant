// Ready-state shell: persistent meeting sidebar + a StackView content pane.
// Sidebar restyled to the Meety design (qt-redesign Phase 3): wordmark + local
// search filter + date-grouped rows + footer. Navigation logic unchanged.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: shell

    // Owns the list state machine.
    MeetingStore { id: store }

    property string selectedId: ""
    property string query: ""
    property bool sidebarCompact: false
    readonly property var shellRef: shell

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
        stack.pop(null, StackView.Immediate)
    }
    function showDetail(m) {
        selectedId = m.id
        stack.pop(null, StackView.Immediate)
        stack.push(detailComp, {
            "shell": shell, "store": store,
            "meetingId": m.id, "meetingName": m.name,
            "audioPath": m.audio_path, "hasTranscript": m.has_transcript === true,
            "createdAt": m.created_at
        }, StackView.Immediate)
    }
    function showNoProtocol(m) {
        selectedId = m.id
        stack.replace(noProtocolComp, {
            "shell": shell, "store": store,
            "meetingId": m.id, "meetingName": m.name,
            "audioPath": m.audio_path, "hasTranscript": m.has_transcript === true,
            "createdAt": m.created_at
        }, StackView.Immediate)
    }
    function showNewRecording() {
        selectedId = ""
        stack.pop(null, StackView.Immediate)
        stack.push(newRecComp, { "shell": shell, "store": store },
                   StackView.Immediate)
    }
    function showGenerate(m, autoStart) {
        selectedId = m.id
        stack.pop(null, StackView.Immediate)
        stack.push(genComp, {
            "shell": shell, "store": store,
            "meetingId": m.id, "meetingName": m.name,
            "audioPath": m.audio_path, "hasTranscript": m.has_transcript === true,
            "autoStart": autoStart === true
        }, StackView.Immediate)
    }
    function showSettings() {
        selectedId = ""
        stack.pop(null, StackView.Immediate)
        stack.push(settingsComp, { "shell": shell }, StackView.Immediate)
    }
    function showDiagnostics() {
        selectedId = ""
        stack.pop(null, StackView.Immediate)
        stack.push(diagComp, { "shell": shell }, StackView.Immediate)
    }

    Component { id: detailComp;   MeetingDetailScreen {} }
    Component { id: noProtocolComp; MeetingNoProtocolScreen {} }
    Component { id: newRecComp;   NewRecordingScreen {} }
    Component { id: genComp;      GenerateProtocolScreen {} }
    Component { id: settingsComp; SettingsScreen {} }
    Component { id: diagComp;     DiagnosticsScreen {} }
    Component {
        id: welcomeComp
        WelcomeScreen { shell: shellRef; store: store }
    }

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
    function compactInitials(name) {
        var s = (name || "").trim()
        if (!s.length)
            return "..."
        var words = s.split(/\s+/).filter(function (w) { return w.length > 0 })
        var first = words.length > 0 ? words[0].charAt(0) : ""
        var second = words.length > 1 ? words[1].charAt(0) : ""
        var label = (first + second).toUpperCase()
        return label.length > 0 ? label : s.slice(0, 2).toUpperCase()
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
            Layout.preferredWidth: shell.sidebarCompact
                                   ? Theme.sidebarCompactWidth
                                   : Theme.sidebarWidth
            Layout.fillHeight: true
            color: Theme.paperSub
            Behavior on Layout.preferredWidth {
                NumberAnimation { duration: Theme.durSlow; easing.type: Easing.OutCubic }
            }

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
                    visible: !shell.sidebarCompact
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
                        iconName: "arrow-left"
                        iconSize: 15
                        onClicked: shell.sidebarCompact = true
                        MeetyToolTip { text: qsTr("Свернуть сайдбар"); visible: parent.hovered }
                    }
                    MeetyIconButton {
                        iconName: "refresh"
                        onClicked: store.refresh()
                        MeetyToolTip { text: qsTr("Обновить"); visible: parent.hovered }
                    }
                    MeetyIconButton {
                        iconName: "plus"; iconSize: 17
                        onClicked: shell.showNewRecording()
                        MeetyToolTip { text: qsTr("Новая запись"); visible: parent.hovered }
                    }
                }

                ColumnLayout {
                    visible: shell.sidebarCompact
                    Layout.fillWidth: true
                    Layout.topMargin: 28
                    Layout.bottomMargin: 22
                    spacing: 18

                    Item {
                        Layout.preferredWidth: 48
                        Layout.preferredHeight: 42
                        Layout.alignment: Qt.AlignHCenter

                        MeetyWordmark {
                            anchors.centerIn: parent
                            size: 40
                        }
                        TapHandler {
                            onTapped: shell.sidebarCompact = false
                        }
                        HoverHandler { id: brandHover }
                        MeetyToolTip { text: qsTr("Развернуть сайдбар"); visible: brandHover.hovered }
                    }

                    MeetyIconButton {
                        Layout.alignment: Qt.AlignHCenter
                        iconName: "plus"
                        iconSize: 17
                        onClicked: shell.showNewRecording()
                        MeetyToolTip { text: qsTr("Новая запись"); visible: parent.hovered }
                    }
                    MeetyIconButton {
                        Layout.alignment: Qt.AlignHCenter
                        iconName: "search"
                        iconSize: 18
                        onClicked: {
                            shell.sidebarCompact = false
                            searchField.forceActiveFocus()
                        }
                        MeetyToolTip { text: qsTr("Поиск встреч"); visible: parent.hovered }
                    }
                }

                // search (local filter; the ⌘K palette is deferred)
                MeetyField {
                    id: searchField
                    visible: !shell.sidebarCompact
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
                        ScrollBar.vertical.policy: shell.sidebarCompact
                                                   ? ScrollBar.AlwaysOff
                                                   : ScrollBar.AsNeeded

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
                                        visible: !shell.sidebarCompact
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
                                            readonly property string compactLabel:
                                                shell.compactInitials(modelData.name)

                                            Layout.fillWidth: true
                                            Layout.leftMargin: shell.sidebarCompact ? 18 : 8
                                            Layout.rightMargin: shell.sidebarCompact ? 22 : 8
                                            implicitHeight: shell.sidebarCompact
                                                            ? 48
                                                            : rowContent.implicitHeight
                                                              + 2 * Theme.rowPy
                                            radius: shell.sidebarCompact ? 14 : Theme.rMd
                                            color: shell.sidebarCompact
                                                   ? (selected ? Theme.paper
                                                      : rowHover.hovered ? Theme.paper4
                                                      : Theme.paper3)
                                                   : (selected ? Theme.paper4
                                                      : rowHover.hovered ? Theme.paper3
                                                      : "transparent")
                                            border.width: selected || activeFocus
                                                          ? (shell.sidebarCompact ? 2 : 1)
                                                          : 0
                                            border.color: activeFocus ? Theme.focus
                                                          : Theme.rule2
                                            activeFocusOnTab: true
                                            Keys.onReturnPressed: shell.showDetail(row.modelData)
                                            Keys.onEnterPressed: shell.showDetail(row.modelData)
                                            Keys.onSpacePressed: shell.showDetail(row.modelData)

                                            // accent left bar (selected)
                                            Rectangle {
                                                visible: row.selected
                                                x: shell.sidebarCompact ? -18 : -4
                                                anchors.verticalCenter: parent.verticalCenter
                                                width: shell.sidebarCompact ? 3 : 2.5
                                                height: shell.sidebarCompact
                                                        ? parent.height - 16
                                                        : parent.height - 18
                                                radius: 2
                                                color: Theme.accent
                                            }

                                            HoverHandler { id: rowHover }
                                            TapHandler {
                                                onTapped: shell.showDetail(row.modelData)
                                            }
                                            MeetyToolTip {
                                                text: row.modelData.name
                                                visible: rowHover.hovered && shell.sidebarCompact
                                                placement: "right"
                                            }

                                            Text {
                                                visible: shell.sidebarCompact
                                                anchors.centerIn: parent
                                                text: row.compactLabel
                                                font.family: Theme.fontSerif
                                                font.italic: true
                                                font.pixelSize: 18
                                                font.weight: Theme.wMedium
                                                color: Theme.ink2
                                            }

                                            RowLayout {
                                                id: rowContent
                                                visible: !shell.sidebarCompact
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
                                                    visible: !shell.sidebarCompact
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

                // footer: diagnostics + settings
                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: 1
                    color: Theme.rule
                }
                RowLayout {
                    Layout.fillWidth: true
                    Layout.leftMargin: shell.sidebarCompact ? 0 : 14
                    Layout.rightMargin: shell.sidebarCompact ? 0 : 10
                    Layout.topMargin: 8
                    Layout.bottomMargin: 8
                    spacing: shell.sidebarCompact ? 6 : 7

                    Item { Layout.fillWidth: true }
                    MeetyIconButton {
                        visible: !shell.sidebarCompact
                        iconName: "cpu"
                        onClicked: shell.showDiagnostics()
                        MeetyToolTip {
                            text: qsTr("Диагностика")
                            visible: parent.hovered
                            placement: "top"
                        }
                    }
                    MeetyIconButton {
                        Layout.alignment: shell.sidebarCompact ? Qt.AlignHCenter : Qt.AlignVCenter
                        iconName: "gear"
                        onClicked: shell.showSettings()
                        MeetyToolTip {
                            text: qsTr("Настройки")
                            visible: parent.hovered
                            placement: "top"
                        }
                    }
                    Item { visible: shell.sidebarCompact; Layout.fillWidth: true }
                }
            }
        }

        // ── content pane ────────────────────────────────────────────────────
        StackView {
            id: stack
            Layout.fillWidth: true
            Layout.fillHeight: true
            initialItem: welcomeComp
            background: Rectangle { color: Theme.paper }
            pushEnter: Transition {}
            pushExit: Transition {}
            popEnter: Transition {}
            popExit: Transition {}
            replaceEnter: Transition {}
            replaceExit: Transition {}
        }
    }
}
