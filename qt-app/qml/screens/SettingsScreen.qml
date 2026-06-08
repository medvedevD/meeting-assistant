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
    font.family: Theme.fontUi
    font.pixelSize: Theme.fsBody

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

    ListModel {
        id: settingsNavModel
        Component.onCompleted: {
            append({ label: qsTr("Транскрипция"), iconName: "mic" })
            append({ label: qsTr("LLM-провайдер"), iconName: "sparkle" })
            append({ label: qsTr("Шаблоны"), iconName: "doc" })
            append({ label: qsTr("Хранилище"), iconName: "storage" })
            append({ label: qsTr("Запись"), iconName: "mic" })
        }
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

    background: Rectangle { color: Theme.paper }

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
        Text {
            Layout.alignment: Qt.AlignHCenter
            text: qsTr("Не удалось загрузить настройки: %1").arg(SettingsStore.errorMessage)
            wrapMode: Text.WordWrap
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBodyLg
            color: Theme.rec
        }
        MeetyButton {
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
            Layout.leftMargin: 24
            Layout.rightMargin: 24
            Layout.topMargin: 16
            Layout.bottomMargin: 16
            spacing: 12

            MeetyButton {
                variant: "ghost"
                iconName: "arrow-left"
                text: qsTr("Назад")
                onClicked: scr.shell.showList()
            }
            Text {
                Layout.fillWidth: true
                text: qsTr("Настройки")
                font.family: Theme.fontSerif
                font.pixelSize: Theme.fsTitle
                font.weight: Theme.wMedium
                font.letterSpacing: 0
                color: Theme.ink
                elide: Text.ElideRight
            }
            Text {
                visible: SettingsStore.status === "saving"
                text: qsTr("Сохранение…")
                font.family: Theme.fontMono
                font.pixelSize: Theme.fsSmall
                color: Theme.ink3
            }
            MeetyButton {
                variant: "ghost"
                text: qsTr("Сбросить")
                enabled: SettingsStore.status !== "saving"
                onClicked: scr.seedDraft()
            }
            MeetyButton {
                variant: "primary"
                text: qsTr("Сохранить")
                enabled: SettingsStore.status !== "saving"
                onClicked: scr.save()
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 1
            color: Theme.rule
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            // category sidebar
            Rectangle {
                Layout.preferredWidth: 200
                Layout.fillHeight: true
                color: Theme.paperSub
                border.width: 0
                ListView {
                    id: catList
                    anchors.fill: parent
                    anchors.margins: 8
                    anchors.topMargin: 16
                    anchors.bottomMargin: 16
                    clip: true
                    currentIndex: 0
                    model: settingsNavModel
                    delegate: Rectangle {
                        required property int index
                        required property string label
                        required property string iconName
                        width: ListView.view.width
                        height: 34
                        radius: Theme.rSm
                        color: catList.currentIndex === index ? Theme.paper4
                              : navMouse.containsMouse ? Theme.paper3 : "transparent"

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 12
                            spacing: 10
                            MeetyIcon {
                                name: iconName
                                size: 12
                                strokeWidth: 2
                                color: catList.currentIndex === index ? Theme.ink : Theme.ink2
                            }
                            Text {
                                Layout.fillWidth: true
                                text: label
                                font.family: Theme.fontUi
                                font.pixelSize: Theme.fsBody
                                font.weight: Theme.wMedium
                                color: catList.currentIndex === index ? Theme.ink : Theme.ink2
                                elide: Text.ElideRight
                            }
                        }
                        MouseArea {
                            id: navMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: catList.currentIndex = index
                        }
                    }
                }
            }

            Rectangle {
                Layout.fillHeight: true
                Layout.preferredWidth: 1
                color: Theme.rule
            }

            // panel stack
            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 0

                // restart-required banner
                Rectangle {
                    Layout.fillWidth: true
                    visible: scr.restartNeeded
                    implicitHeight: restartText.implicitHeight + 24
                    color: Theme.accentTint
                    Text {
                        id: restartText
                        anchors.fill: parent
                        anchors.margins: 12
                        wrapMode: Text.WordWrap
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsBody
                        color: Theme.ink2
                        text: qsTr("Изменение пути к базе данных или каталога встреч " +
                                   "вступит в силу после перезапуска приложения.")
                    }
                }
                // insecure-secrets banner (keyring unavailable → plaintext fallback).
                // Only for the plaintext backend: a passphrase vault is encrypted,
                // so the "без шифрования" copy below would not apply.
                Rectangle {
                    Layout.fillWidth: true
                    visible: SettingsStore.secretStorage().kind === "plaintext"
                    implicitHeight: secretText.implicitHeight + 24
                    color: Theme.paper3
                    Text {
                        id: secretText
                        anchors.fill: parent
                        anchors.margins: 12
                        wrapMode: Text.WordWrap
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsBody
                        color: Theme.ink2
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
        background: Rectangle {
            radius: Theme.rMd
            color: Theme.ink
        }
        Text {
            text: toast.message
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            color: Theme.paper
        }
        Timer { id: hideTimer; interval: 2500; onTriggered: toast.close() }
    }
}
