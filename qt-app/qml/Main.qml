import QtCore
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import MeetingAssistant

ApplicationWindow {
    id: root
    width: 1120
    height: 820
    // Shown by restoreGeometry(), once it is where the user left it.
    visible: false
    title: qsTr("Meeting Assistant")
    color: Theme.paper

    // ── window size and place across launches ──────────────────────────────
    Settings {
        id: savedWindow
        category: "window"
        property int left: 0
        property int top: 0
        property int width: 0       // 0: nothing saved yet
        property int height: 0
        property bool maximized: false
    }

    function restoreGeometry() {
        // A monitor may be gone since: use the saved place only if the title
        // bar lands on a screen that is still there.
        var place = null
        if (savedWindow.width >= 640 && savedWindow.height >= 480) {
            var screens = Qt.application.screens
            for (var i = 0; i < screens.length; ++i) {
                var sc = screens[i]
                if (savedWindow.left >= sc.virtualX
                        && savedWindow.top >= sc.virtualY
                        && savedWindow.left + 120 <= sc.virtualX + sc.width
                        && savedWindow.top + 40 <= sc.virtualY + sc.height) {
                    place = sc
                    break
                }
            }
        }
        if (place) {
            root.width = Math.min(savedWindow.width, place.width)
            root.height = Math.min(savedWindow.height, place.height)
        }

        if (savedWindow.maximized) {
            root.showMaximized()
            return
        }
        root.show()
        // Placed after show(): a position set on a hidden window counts the
        // title bar on macOS, while x and y read back without it, so the
        // window would creep up on every launch.
        if (place) {
            root.x = savedWindow.left
            root.y = savedWindow.top
        } else {
            // First launch, or its screen is gone: centre on the current one,
            // so the place is known instead of left to the system.
            var s = root.screen
            root.width = Math.min(root.width, s.width)
            root.height = Math.min(root.height, s.height)
            root.x = s.virtualX + Math.round((s.width - root.width) / 2)
            root.y = s.virtualY + Math.round((s.height - root.height) / 2)
        }
    }

    function saveGeometry() {
        savedWindow.maximized = root.visibility === Window.Maximized
        // A maximized or full-screen window keeps the size it returns to.
        if (root.visibility === Window.Windowed) {
            savedWindow.left = root.x
            savedWindow.top = root.y
            savedWindow.width = root.width
            savedWindow.height = root.height
        }
        savedWindow.sync()
    }

    Component.onCompleted: restoreGeometry()
    onClosing: saveGeometry()
    // Cmd+Q on macOS quits without closing the window first.
    Connections {
        target: Qt.application
        function onAboutToQuit() { root.saveGeometry() }
    }

    // MEETY_THEME_GALLERY=1 shows the token
    // gallery instead of the app shell. Dev-only; never reached in production.
    Loader {
        anchors.fill: parent
        active: typeof themeGalleryEnabled !== "undefined" && themeGalleryEnabled
        sourceComponent: ThemeGallery {}
        z: 1000
    }

    // One page per sidecar lifecycle state: starting, ready (the app itself),
    // and a backdrop for a failed or incompatible sidecar.
    StackLayout {
        anchors.fill: parent
        currentIndex: {
            switch (sidecar.state) {
            case SidecarManager.Ready: return 1
            case SidecarManager.Incompatible:
            case SidecarManager.Failed: return 2
            default: return 0 // Idle / Starting / Restarting
            }
        }

        // ── 0: starting / restarting ─────────────────────────────────────────
        Rectangle {
            color: Theme.paper

            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(parent.width - 64, 420)
                spacing: 16

                MeetyWordmark {
                    size: 40
                    Layout.alignment: Qt.AlignHCenter
                }
                BusyIndicator {
                    running: true
                    Layout.alignment: Qt.AlignHCenter
                }
                Text {
                    Layout.fillWidth: true
                    text: sidecar.state === SidecarManager.Restarting
                          ? qsTr("Перезапускаем Meeting Assistant…")
                          : qsTr("Запускаем Meeting Assistant…")
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.WordWrap
                    font.family: Theme.serifFor(text)
                    renderType: Theme.renderTypeFor(font.family)
                    font.pixelSize: Theme.fsTitle
                    font.weight: Theme.wMedium
                    color: Theme.ink
                }
                Text {
                    Layout.fillWidth: true
                    visible: sidecar.state === SidecarManager.Restarting
                    text: qsTr("Фоновая часть приложения неожиданно остановилась и запускается заново.")
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.WordWrap
                    lineHeight: 1.35
                    font.family: Theme.fontUi
                    font.pixelSize: Theme.fsBodyLg
                    color: Theme.ink3
                }
            }
        }

        // ── 1: ready (the real app) ─────────────────────────────────────────
        AppShell {}

        // ── 2: terminal failure backdrop (dialog draws on top) ──────────────
        Item {}
    }

    // Version gate (Q9): blocking, non-dismissable. The user cannot proceed.
    MeetyDialog {
        id: incompatibleDialog
        closePolicy: Popup.NoAutoClose
        preferredWidth: 460
        title: qsTr("Несовместимые компоненты")
        visible: sidecar.state === SidecarManager.Incompatible

        Text {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            lineHeight: 1.35
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBodyLg
            color: Theme.ink2
            text: qsTr("Компоненты несовместимы — обновите приложение.\n\n"
                       + "Интерфейс использует протокол %1, а ядро "
                       + "поддерживает только %2–%3.")
                  .arg(sidecar.clientProtocol)
                  .arg(sidecar.serverMinProtocol)
                  .arg(sidecar.serverProtocol)
        }

        footer: MeetyDialogActions {
            dialog: incompatibleDialog
            showCancel: false
            confirmText: qsTr("Закрыть")
            confirmVariant: "primary"
            onAccepted: Qt.quit()
        }
    }

    // Non-version fatal (e.g. another instance already running, restart budget
    // exhausted).
    MeetyDialog {
        id: failedDialog
        closePolicy: Popup.NoAutoClose
        preferredWidth: 460
        title: qsTr("Не удалось запустить")
        visible: sidecar.state === SidecarManager.Failed

        Text {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            lineHeight: 1.35
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBodyLg
            color: Theme.ink2
            text: sidecar.lastError.length > 0
                  ? sidecar.lastError
                  : qsTr("Ядро не запустилось.")
        }

        footer: MeetyDialogActions {
            dialog: failedDialog
            showCancel: false
            confirmText: qsTr("Закрыть")
            confirmVariant: "primary"
            onAccepted: Qt.quit()
        }
    }
}
