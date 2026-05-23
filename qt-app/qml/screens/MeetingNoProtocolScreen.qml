import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import MeetingAssistant

Page {
    id: scr

    property var shell
    property var store
    property string meetingId
    property string meetingName
    property string audioPath
    property bool hasTranscript: false
    property double createdAt: 0

    property string actionNote: ""

    background: Rectangle { color: Theme.paper }

    Request {
        id: deleteReq
        onOk: function (j) {
            scr.store.refresh()
            scr.shell.showList()
        }
        onFail: function (s, e) { scr.actionNote = qsTr("Ошибка удаления: %1").arg(e) }
    }
    Request {
        id: deleteAudioReq
        onOk: function (j) {
            scr.actionNote = qsTr("Аудио удалено, транскрипт сохранён.")
            scr.store.refresh()
        }
        onFail: function (s, e) { scr.actionNote = qsTr("Ошибка: %1").arg(e) }
    }

    MeetyMenu {
        id: actionMenu
        MeetyMenuItem {
            text: qsTr("Удалить аудио (оставить транскрипт)")
            iconName: "trash"
            danger: true
            enabled: scr.audioPath.length > 0
            onTriggered: deleteAudioReq.del("/api/v1/meetings/" + scr.meetingId + "?mode=audio")
        }
        MeetyMenuItem {
            text: qsTr("Удалить встречу")
            iconName: "trash"
            danger: true
            onTriggered: confirmDelete.open()
        }
    }

    Dialog {
        id: confirmDelete
        modal: true
        anchors.centerIn: Overlay.overlay
        title: qsTr("Удалить встречу?")
        standardButtons: Dialog.Yes | Dialog.No
        onAccepted: deleteReq.del("/api/v1/meetings/" + scr.meetingId + "?mode=full")
        Label {
            width: 360
            wrapMode: Text.WordWrap
            text: qsTr("Встреча, её транскрипт, протокол и аудиофайл будут удалены безвозвратно.")
        }
    }

    RowLayout {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.leftMargin: 24
        anchors.rightMargin: 24
        anchors.topMargin: 16
        spacing: 12
        z: 1

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 4
            Text {
                Layout.fillWidth: true
                text: scr.meetingName
                elide: Text.ElideRight
                font.family: Theme.fontSerif
                font.pixelSize: Theme.fsTitle
                font.weight: Theme.wMedium
                font.letterSpacing: Theme.tracking(Theme.fsTitle, -0.02)
                color: Theme.ink
            }
            RowLayout {
                spacing: 10
                Text {
                    visible: scr.createdAt > 0
                    text: Qt.formatDateTime(new Date(scr.createdAt * 1000),
                                            "d MMMM, HH:mm")
                    font.family: Theme.fontUi
                    font.pixelSize: Theme.fsSmall
                    color: Theme.ink3
                }
                Rectangle {
                    visible: scr.hasTranscript
                    width: 2.5; height: 2.5; radius: 1.25; color: Theme.ink4
                }
                Text {
                    visible: scr.hasTranscript
                    text: qsTr("транскрипт готов")
                    font.family: Theme.fontUi
                    font.pixelSize: Theme.fsSmall
                    color: Theme.ink3
                }
            }
        }

        MeetyIconButton {
            id: actionButton
            Layout.alignment: Qt.AlignTop
            iconName: "more"
            onClicked: actionMenu.popupFromButton(actionButton)
            MeetyToolTip { text: qsTr("Действия"); visible: parent.hovered }
        }
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.topMargin: 88
        height: 1
        color: Theme.rule
    }

    ColumnLayout {
        anchors.centerIn: parent
        width: Math.min(Math.max(parent.width - 64, 260), 460)
        spacing: 16

        MeetyIcon {
            Layout.alignment: Qt.AlignHCenter
            name: "doc"; size: 36; color: Theme.ink4
        }
        Text {
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            text: qsTr("Протокол ещё не сгенерирован")
            font.family: Theme.fontSerif
            font.pixelSize: 28
            font.weight: Theme.wMedium
            font.letterSpacing: Theme.tracking(28, -0.015)
            color: Theme.ink
        }
        Text {
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBodyLg
            color: Theme.ink3
            text: scr.audioPath.length > 0
                  ? qsTr("У встречи есть аудиозапись. Запустите транскрипцию и генерацию протокола — это займёт пару минут.")
                  : qsTr("Запишите встречу, чтобы сгенерировать протокол.")
        }
        MeetyButton {
            visible: scr.audioPath.length > 0
            Layout.alignment: Qt.AlignHCenter
            variant: "accent"
            large: true
            iconName: "sparkle"
            text: qsTr("Сгенерировать протокол")
            onClicked: scr.shell.showGenerate({
                "id": scr.meetingId, "name": scr.meetingName,
                "audio_path": scr.audioPath,
                "has_transcript": scr.hasTranscript,
                "created_at": scr.createdAt
            }, false)
        }
        Text {
            visible: scr.actionNote.length > 0
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            color: Theme.ink2
            text: scr.actionNote
        }
    }
}
