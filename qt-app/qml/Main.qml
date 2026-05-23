import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import MeetingAssistant

ApplicationWindow {
    id: root
    width: 1120
    height: 820
    visible: true
    title: qsTr("Meeting Assistant")
    color: Theme.paper

    // qt-redesign Phase-1 sign-off: MEETY_THEME_GALLERY=1 shows the token
    // gallery instead of the app shell. Dev-only; never reached in production.
    Loader {
        anchors.fill: parent
        active: typeof themeGalleryEnabled !== "undefined" && themeGalleryEnabled
        sourceComponent: ThemeGallery {}
        z: 1000
    }

    // Skeleton shell (section-03). Real screens land in section-04; this view
    // exists to prove the boundary: Fusion enforced, sidecar lifecycle states,
    // version gate, authenticated ApiClient.
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
                    text: sidecar.stateText
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.WordWrap
                    font.family: Theme.fontSerif
                    font.pixelSize: Theme.fsTitle
                    font.weight: Theme.wMedium
                    color: Theme.ink
                }
                Text {
                    Layout.fillWidth: true
                    visible: sidecar.state === SidecarManager.Restarting
                    text: qsTr("Ядро остановилось неожиданно и перезапускается…")
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.WordWrap
                    lineHeight: 1.35
                    font.family: Theme.fontUi
                    font.pixelSize: Theme.fsBodyLg
                    color: Theme.ink3
                }
            }
        }

        // ── 1: ready (the real app — section-04 screens) ────────────────────
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
