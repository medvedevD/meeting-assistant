import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window

ApplicationWindow {
    id: root
    width: 900
    height: 620
    visible: true
    title: qsTr("Meeting Assistant")

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
        ColumnLayout {
            spacing: 16
            Item { Layout.fillHeight: true }
            BusyIndicator {
                running: true
                Layout.alignment: Qt.AlignHCenter
            }
            Label {
                text: sidecar.stateText
                font.pixelSize: 18
                Layout.alignment: Qt.AlignHCenter
            }
            Label {
                visible: sidecar.state === SidecarManager.Restarting
                text: qsTr("The core stopped unexpectedly and is being restarted…")
                opacity: 0.7
                Layout.alignment: Qt.AlignHCenter
            }
            Item { Layout.fillHeight: true }
        }

        // ── 1: ready (the real app — section-04 screens) ────────────────────
        AppShell {}

        // ── 2: terminal failure backdrop (dialog draws on top) ──────────────
        Item {}
    }

    // Version gate (Q9): blocking, non-dismissable. The user cannot proceed.
    Dialog {
        id: incompatibleDialog
        modal: true
        closePolicy: Popup.NoAutoClose
        anchors.centerIn: parent
        width: 460
        title: qsTr("Incompatible components")
        visible: sidecar.state === SidecarManager.Incompatible
        standardButtons: Dialog.NoButton

        ColumnLayout {
            anchors.fill: parent
            spacing: 12
            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: qsTr("Components are incompatible — please update.\n\n"
                           + "The interface speaks protocol %1, but the core "
                           + "only supports protocols %2–%3.")
                      .arg(sidecar.clientProtocol)
                      .arg(sidecar.serverMinProtocol)
                      .arg(sidecar.serverProtocol)
            }
            Button {
                text: qsTr("Quit")
                Layout.alignment: Qt.AlignRight
                onClicked: Qt.quit()
            }
        }
    }

    // Non-version fatal (e.g. another instance already running, restart budget
    // exhausted).
    Dialog {
        id: failedDialog
        modal: true
        closePolicy: Popup.NoAutoClose
        anchors.centerIn: parent
        width: 460
        title: qsTr("Cannot start")
        visible: sidecar.state === SidecarManager.Failed
        standardButtons: Dialog.NoButton

        ColumnLayout {
            anchors.fill: parent
            spacing: 12
            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: sidecar.lastError.length > 0
                      ? sidecar.lastError
                      : qsTr("The core failed to start.")
            }
            Button {
                text: qsTr("Quit")
                Layout.alignment: Qt.AlignRight
                onClicked: Qt.quit()
            }
        }
    }
}
