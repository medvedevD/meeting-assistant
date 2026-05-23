// DiagnosticsScreen — the log/health surface the section asks for, from the
// meta routes (audit: partial reimplement). Live /health + /version, sidecar
// URL, protocol range, enforced style. Device/path/ffmpeg/log enumeration has
// no sidecar route → stated honestly, flagged future core/API route.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import MeetingAssistant

Page {
    id: scr
    property var shell
    property string healthText: qsTr("проверка…")
    property string versionText: qsTr("проверка…")

    function probe() {
        healthText = qsTr("проверка…")
        versionText = qsTr("проверка…")
        healthReq.get("/health")
        versionReq.get("/version")
    }

    Request {
        id: healthReq
        onOk: function (j) {
            scr.healthText = j && j.status ? j.status : JSON.stringify(j)
        }
        onFail: function (s, e) { scr.healthText = qsTr("недоступен: %1").arg(e) }
    }
    Request {
        id: versionReq
        onOk: function (j) {
            scr.versionText = qsTr("build %1 · protocol [%2, %3]")
                .arg(j.build).arg(j.min_protocol).arg(j.protocol)
        }
        onFail: function (s, e) { scr.versionText = qsTr("недоступен: %1").arg(e) }
    }

    Component.onCompleted: probe()

    background: Rectangle { color: Theme.paper }

    component DiagnosticsRow: RowLayout {
        id: row
        property string label: ""
        property string value: ""
        property bool strong: false

        spacing: 18

        Text {
            Layout.preferredWidth: 150
            text: row.label
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            color: Theme.ink3
        }
        Text {
            Layout.fillWidth: true
            text: row.value
            wrapMode: Text.WrapAnywhere
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            font.weight: row.strong ? Theme.wSemiBold : Theme.wRegular
            color: row.strong ? Theme.ink : Theme.ink2
        }
    }

    header: Item {
        implicitHeight: 65

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 24
            anchors.rightMargin: 24
            spacing: 12

            MeetyButton {
                variant: "ghost"
                iconName: "arrow-left"
                text: qsTr("Назад")
                onClicked: scr.shell.showList()
            }

            Text {
                Layout.fillWidth: true
                text: qsTr("Диагностика")
                elide: Text.ElideRight
                font.family: Theme.fontSerif
                font.pixelSize: Theme.fsTitle
                font.weight: Theme.wMedium
                font.letterSpacing: Theme.tracking(Theme.fsTitle, -0.02)
                color: Theme.ink
            }

            MeetyIconButton {
                iconName: "refresh"
                onClicked: scr.probe()
            }
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 1
            color: Theme.rule
        }
    }

    ScrollView {
        anchors.fill: parent
        clip: true
        contentWidth: availableWidth

        ColumnLayout {
            width: Math.min(parent.width - 48, 760)
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 16

            MeetyCard {
                Layout.fillWidth: true
                Layout.topMargin: 24

                ColumnLayout {
                    anchors.fill: parent
                    spacing: 12

                    MeetySectionLabel { label: qsTr("Сайдкар") }
                    DiagnosticsRow {
                        label: qsTr("URL:")
                        value: sidecar.baseUrl
                        strong: true
                    }
                    DiagnosticsRow {
                        label: qsTr("/health:")
                        value: scr.healthText
                        strong: true
                    }
                    DiagnosticsRow {
                        label: qsTr("/version:")
                        value: scr.versionText
                    }
                    DiagnosticsRow {
                        label: qsTr("Протокол (клиент):")
                        value: sidecar.clientProtocol
                    }
                    DiagnosticsRow {
                        label: qsTr("Протокол (сервер):")
                        value: "[" + sidecar.serverMinProtocol + ", "
                               + sidecar.serverProtocol + "]"
                    }
                    DiagnosticsRow {
                        label: qsTr("Сборка сервера:")
                        value: sidecar.serverBuild
                    }
                    DiagnosticsRow {
                        label: qsTr("Стиль интерфейса:")
                        value: controlsStyle
                        strong: true
                    }
                }
            }

            MeetyCard {
                Layout.fillWidth: true
                Layout.bottomMargin: 24

                ColumnLayout {
                    anchors.fill: parent
                    spacing: 12

                    MeetySectionLabel { label: qsTr("Недоступно через сайдкар") }
                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        lineHeight: 1.35
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsBody
                        color: Theme.ink3
                        text: qsTr("Список аудиоустройств, пути и их статус, " +
                                   "проверка ffmpeg и журнал ядра не " +
                                   "экспонируются через 7 маршрутов сайдкара. " +
                                   "Появятся здесь после добавления " +
                                   "диагностического маршрута в ядро.")
                    }
                }
            }
        }
    }
}
