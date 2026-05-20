// Ready-state shell: persistent meeting sidebar + a StackView content pane.
// Decompose `RootComponent` navigation reimplemented client-side (audit).
// Plain Fusion + sane spacing only — no restyling (section guardrail).
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: shell

    // Owns the list state machine + in-session protocol cache.
    MeetingStore { id: store }

    property string selectedId: ""

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

    RowLayout {
        anchors.fill: parent
        spacing: 0

        // ── sidebar ─────────────────────────────────────────────────────────
        Pane {
            Layout.preferredWidth: 280
            Layout.fillHeight: true
            padding: 0

            ColumnLayout {
                anchors.fill: parent
                spacing: 0

                // header
                RowLayout {
                    Layout.fillWidth: true
                    Layout.margins: 10
                    Label {
                        text: qsTr("Встречи")
                        font.pixelSize: 16
                        font.bold: true
                        Layout.fillWidth: true
                    }
                    ToolButton {
                        text: qsTr("⟳")
                        ToolTip.text: qsTr("Обновить")
                        ToolTip.visible: hovered
                        onClicked: store.refresh()
                    }
                    ToolButton {
                        text: qsTr("＋")
                        ToolTip.text: qsTr("Новая запись")
                        ToolTip.visible: hovered
                        onClicked: shell.showNewRecording()
                    }
                }
                MenuSeparator { Layout.fillWidth: true }

                // list — the four data-states
                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    BusyIndicator {
                        anchors.centerIn: parent
                        running: true
                        visible: store.status === "loading"
                    }

                    Label {
                        anchors.centerIn: parent
                        width: parent.width - 32
                        visible: store.status === "empty"
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.WordWrap
                        opacity: 0.7
                        text: qsTr("Нет встреч")
                    }

                    ColumnLayout {
                        anchors.centerIn: parent
                        width: parent.width - 24
                        visible: store.status === "error"
                        spacing: 8
                        Label {
                            Layout.fillWidth: true
                            horizontalAlignment: Text.AlignHCenter
                            wrapMode: Text.WordWrap
                            color: shell.palette.toolTipText
                            text: qsTr("Ошибка: %1").arg(store.errorMessage)
                        }
                        Button {
                            Layout.alignment: Qt.AlignHCenter
                            text: qsTr("Повторить")
                            onClicked: store.refresh()
                        }
                    }

                    ListView {
                        id: list
                        anchors.fill: parent
                        visible: store.status === "success"
                        clip: true
                        model: store.meetings
                        ScrollBar.vertical: ScrollBar {}
                        delegate: ItemDelegate {
                            required property var modelData
                            width: ListView.view.width
                            highlighted: shell.selectedId === modelData.id
                            onClicked: shell.showDetail(modelData)
                            contentItem: ColumnLayout {
                                spacing: 2
                                Label {
                                    Layout.fillWidth: true
                                    text: modelData.name
                                    elide: Text.ElideRight
                                    font.bold: true
                                }
                                Label {
                                    text: Qt.formatDateTime(
                                        new Date(modelData.created_at * 1000),
                                        Qt.locale(), Locale.ShortFormat)
                                    opacity: 0.7
                                    font.pixelSize: 11
                                }
                                Label {
                                    visible: modelData.has_transcript === true
                                    text: qsTr("Транскрипт")
                                    font.pixelSize: 11
                                    color: shell.palette.highlight
                                }
                            }
                        }
                    }
                }

                MenuSeparator { Layout.fillWidth: true }
                RowLayout {
                    Layout.fillWidth: true
                    Layout.margins: 6
                    Item { Layout.fillWidth: true }
                    ToolButton {
                        text: qsTr("Диагностика")
                        onClicked: shell.showDiagnostics()
                    }
                    ToolButton {
                        text: qsTr("Настройки")
                        onClicked: shell.showSettings()
                    }
                }
            }
        }

        ToolSeparator {
            Layout.fillHeight: true
            padding: 0
        }

        // ── content pane ────────────────────────────────────────────────────
        StackView {
            id: stack
            Layout.fillWidth: true
            Layout.fillHeight: true
            initialItem: MeetingListScreen {}
        }
    }
}
