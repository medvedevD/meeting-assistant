// What a later launch has to tell the user, shown above the home state: a
// recording recovered after a crash, and setup that used to work and broke.
// Nothing here nags about setup the user never did or put off: that reminder
// lives on the recording screen only.
//
// Reading all of this raises no system dialog: the setup status is silent by
// contract, and MicrophonePermission.status never asks.
import QtQuick
import QtQuick.Layouts
import MeetingAssistant

ColumnLayout {
    id: notices

    // Meetings as the sidebar has them, to open a recovered one.
    property var meetings: []
    // AppPreferences, or null in a context that has none.
    property var preferences: null

    signal openMeeting(var meeting)
    // 0 transcription, 1 LLM provider.
    signal openSettings(int category)

    spacing: 8
    visible: items.length > 0

    // Ids of notices closed during this run. A new launch shows them again if
    // the cause is still there; a recovered recording is reported only once,
    // by the launch that recovered it.
    property var dismissed: ({})

    readonly property var permission:
        typeof microphonePermission !== "undefined" ? microphonePermission : null
    readonly property bool onMac: Qt.platform.os === "osx"

    function meetingById(id) {
        for (var i = 0; i < meetings.length; ++i)
            if (meetings[i].id === id)
                return meetings[i]
        return null
    }

    readonly property var items: {
        var out = []
        if (!SetupStore.loaded)
            return out
        var gone = dismissed

        var recovered = SetupStore.snapshot.recovered_recordings || []
        for (var i = 0; i < recovered.length; ++i) {
            var r = recovered[i]
            out.push({
                id: "recovered-" + r.meeting_id,
                tone: "info",
                text: qsTr("Запись «%1» прервалась при сбое и восстановлена.").arg(r.name),
                action: qsTr("Открыть"),
                run: "meeting", meetingId: r.meeting_id })
        }

        // A microphone that was heard before and is now refused: the user
        // turned it off in the system settings, or an update dropped it.
        if (permission && permission.status === "denied"
                && preferences && preferences.microphoneTested === true) {
            out.push({
                id: "mic-denied", tone: "warn",
                text: qsTr("Доступ к микрофону запрещён: записать встречу не получится."),
                action: onMac ? qsTr("Открыть настройки macOS") : "",
                run: "mic" })
        }

        // A model that was chosen and whose file is gone. "Never chosen" is
        // left to the recording screen.
        if (SetupStore.transcriptionState === "missing"
                && SetupStore.transcriptionReason === "model_missing") {
            out.push({
                id: "model-missing", tone: "warn",
                text: qsTr("Файл модели распознавания пропал: новые записи не расшифруются."),
                action: qsTr("Выбрать модель"), run: "settings", category: 0 })
        }

        if (SetupStore.minutesState === "locked") {
            out.push({
                id: "keys-locked", tone: "warn",
                text: qsTr("Хранилище ключей заблокировано: протоколы не сгенерируются."),
                action: qsTr("Разблокировать"), run: "settings", category: 1 })
        }

        return out.filter(function (n) { return gone[n.id] !== true })
    }

    function act(n) {
        if (n.run === "meeting") {
            var m = meetingById(n.meetingId)
            if (m)
                openMeeting(m)
            dismiss(n.id)
        } else if (n.run === "mic") {
            Qt.openUrlExternally(
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
        } else if (n.run === "settings") {
            openSettings(n.category)
        }
    }

    function dismiss(id) {
        var next = Object.assign({}, dismissed)
        next[id] = true
        dismissed = next
    }

    // The user may have fixed the permission in System Settings meanwhile.
    Connections {
        target: Qt.application
        function onStateChanged() {
            if (Qt.application.state === Qt.ApplicationActive && notices.permission)
                notices.permission.refresh()
        }
    }

    Repeater {
        model: notices.items

        Rectangle {
            id: card
            required property var modelData
            objectName: "notice-" + modelData.id
            Layout.fillWidth: true
            implicitHeight: row.implicitHeight + 20
            radius: Theme.rMd
            color: Theme.paperSub
            border.width: 1
            border.color: Theme.rule

            RowLayout {
                id: row
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                anchors.leftMargin: 14
                anchors.rightMargin: 6
                spacing: 10

                Rectangle {
                    Layout.preferredWidth: 6
                    Layout.preferredHeight: 6
                    Layout.alignment: Qt.AlignVCenter
                    radius: 3
                    color: card.modelData.tone === "warn" ? Theme.warn : Theme.ok
                }
                Text {
                    Layout.fillWidth: true
                    text: card.modelData.text
                    wrapMode: Text.WordWrap
                    lineHeight: 1.35
                    font.family: Theme.fontUi
                    font.pixelSize: Theme.fsBody
                    color: Theme.ink2
                }
                MeetyButton {
                    objectName: "noticeAction"
                    Layout.alignment: Qt.AlignVCenter
                    visible: card.modelData.action.length > 0
                    variant: "ghost"
                    text: card.modelData.action
                    onClicked: notices.act(card.modelData)
                }
                MeetyIconButton {
                    objectName: "noticeClose"
                    Layout.alignment: Qt.AlignVCenter
                    iconName: "plus"
                    rotation: 45
                    Accessible.name: qsTr("Скрыть")
                    onClicked: notices.dismiss(card.modelData.id)
                }
            }
        }
    }
}
