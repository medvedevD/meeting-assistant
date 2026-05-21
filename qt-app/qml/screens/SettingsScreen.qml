// SettingsScreen — macOS-style category sidebar (ListView) + StackLayout of
// panels (Phase 5, decision #12/#13). Server-side settings are the single
// source of truth: the screen pulls the snapshot via the SettingsStore
// singleton into an editable `draft`, the panels mutate it, and a single
// "Сохранить" button PUTs the whole document. db_path / meetings_dir changes
// are restart-required (decision #2) → a banner, applied on next launch.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import MeetingAssistant

Page {
    id: scr
    property var shell

    // Editable working copy of the snapshot. Panels mutate it in place; Save
    // PUTs it. Re-seeded whenever the store (re)loads.
    property var draft: ({})
    // Bumped on every edit so derived bindings (e.g. the restart banner) refresh.
    property int rev: 0
    function touch() { rev++ }
    // Emitted whenever `draft` is (re)seeded from the store — panels (re)read
    // their control values from it on this, not on every keystroke.
    signal reseeded()

    readonly property bool restartNeeded: {
        rev // depend on edits
        if (!SettingsStore.loaded || !draft.paths)
            return false
        var orig = SettingsStore.paths()
        return (draft.paths.db || "") !== (orig.db || "")
            || (draft.paths.meetings_dir || "") !== (orig.meetings_dir || "")
    }

    function seedDraft() {
        if (!SettingsStore.loaded)
            return
        draft = JSON.parse(JSON.stringify(SettingsStore.snapshot))
        rev++
        reseeded()
    }

    function save() {
        SettingsStore.apply(scr.draft, function (ok, res) {
            if (ok) {
                toast.show(qsTr("Настройки сохранены"))
                scr.seedDraft()
            } else {
                toast.show(qsTr("Ошибка сохранения: %1").arg(res))
            }
        })
    }

    Component.onCompleted: {
        if (SettingsStore.loaded)
            seedDraft()
        else
            SettingsStore.refresh()
    }
    Connections {
        target: SettingsStore
        function onLoadedChanged() { if (SettingsStore.loaded) scr.seedDraft() }
    }

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

    // ── loading / error gate ──────────────────────────────────────────────────
    BusyIndicator {
        anchors.centerIn: parent
        running: true
        visible: SettingsStore.status === "loading" && !SettingsStore.loaded
    }
    ColumnLayout {
        anchors.centerIn: parent
        visible: SettingsStore.status === "error" && !SettingsStore.loaded
        spacing: 10
        Label {
            Layout.alignment: Qt.AlignHCenter
            text: qsTr("Не удалось загрузить настройки: %1").arg(SettingsStore.errorMessage)
            wrapMode: Text.WordWrap
        }
        Button {
            Layout.alignment: Qt.AlignHCenter
            text: qsTr("Повторить")
            onClicked: SettingsStore.refresh()
        }
    }

    // ── main content ──────────────────────────────────────────────────────────
    ColumnLayout {
        anchors.fill: parent
        visible: SettingsStore.loaded
        spacing: 0

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            // category sidebar
            Pane {
                Layout.preferredWidth: 200
                Layout.fillHeight: true
                padding: 0
                ListView {
                    id: catList
                    anchors.fill: parent
                    clip: true
                    currentIndex: 0
                    model: ListModel {
                        ListElement { label: qsTr("Транскрипция") }
                        ListElement { label: qsTr("LLM-провайдер") }
                        ListElement { label: qsTr("Шаблоны") }
                        ListElement { label: qsTr("Хранилище") }
                        ListElement { label: qsTr("Запись") }
                    }
                    delegate: ItemDelegate {
                        required property int index
                        required property string label
                        width: ListView.view.width
                        text: label
                        highlighted: catList.currentIndex === index
                        onClicked: catList.currentIndex = index
                    }
                }
            }

            ToolSeparator { Layout.fillHeight: true; padding: 0 }

            // panel stack
            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 0

                // restart-required banner
                Pane {
                    Layout.fillWidth: true
                    visible: scr.restartNeeded
                    background: Rectangle { color: scr.palette.highlight; opacity: 0.15 }
                    Label {
                        width: parent.width
                        wrapMode: Text.WordWrap
                        text: qsTr("Изменение пути к базе данных или каталога встреч " +
                                   "вступит в силу после перезапуска приложения.")
                    }
                }
                // insecure-secrets banner (keyring unavailable → plaintext fallback)
                Pane {
                    Layout.fillWidth: true
                    visible: SettingsStore.secretsFallback()
                    background: Rectangle { color: scr.palette.toolTipText; opacity: 0.12 }
                    Label {
                        width: parent.width
                        wrapMode: Text.WordWrap
                        text: qsTr("Системное хранилище ключей недоступно — API-ключи " +
                                   "сохраняются в файл без шифрования " +
                                   "(~/.config/meeting-assistant/secrets.json).")
                    }
                }

                StackLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    currentIndex: catList.currentIndex

                    WhisperPanel   { scr: scr }
                    LlmPanel       { scr: scr }
                    TemplatesPanel { scr: scr }
                    StoragePanel   { scr: scr }
                    RecordingPanel { scr: scr }
                }
            }
        }

        // ── footer: save / reset ────────────────────────────────────────────
        MenuSeparator { Layout.fillWidth: true }
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 12
            Label {
                Layout.fillWidth: true
                opacity: 0.7
                font.pixelSize: 12
                text: SettingsStore.status === "saving" ? qsTr("Сохранение…") : ""
            }
            Button {
                text: qsTr("Сбросить изменения")
                enabled: SettingsStore.status !== "saving"
                onClicked: scr.seedDraft()
            }
            Button {
                text: qsTr("Сохранить")
                highlighted: true
                enabled: SettingsStore.status !== "saving"
                onClicked: scr.save()
            }
        }
    }

    // lightweight toast
    Popup {
        id: toast
        property string message: ""
        function show(m) { message = m; open(); hideTimer.restart() }
        modal: false
        focus: false
        closePolicy: Popup.NoAutoClose
        x: (scr.width - width) / 2
        y: scr.height - height - 24
        padding: 12
        Label { text: toast.message }
        Timer { id: hideTimer; interval: 2500; onTriggered: toast.close() }
    }
}
