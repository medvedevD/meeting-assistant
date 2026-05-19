// DiagnosticsScreen — the log/health surface the section asks for, from the
// meta routes (audit: partial reimplement). Live /health + /version, sidecar
// URL, protocol range, enforced style. Device/path/ffmpeg/log enumeration has
// no sidecar route → stated honestly, flagged future core/API route.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

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

    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            ToolButton { text: qsTr("‹ Назад"); onClicked: scr.shell.showList() }
            Label {
                text: qsTr("Диагностика")
                font.bold: true
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
            }
            ToolButton { text: qsTr("⟳"); onClicked: scr.probe() }
        }
    }

    ScrollView {
        anchors.fill: parent
        clip: true
        ColumnLayout {
            width: scr.width
            spacing: 16

            GroupBox {
                title: qsTr("Сайдкар")
                Layout.fillWidth: true
                Layout.margins: 24
                GridLayout {
                    columns: 2
                    columnSpacing: 24
                    rowSpacing: 8
                    Label { text: qsTr("URL:") }
                    Label { text: sidecar.baseUrl; font.bold: true }
                    Label { text: qsTr("/health:") }
                    Label { text: scr.healthText; font.bold: true }
                    Label { text: qsTr("/version:") }
                    Label { text: scr.versionText }
                    Label { text: qsTr("Протокол (клиент):") }
                    Label { text: sidecar.clientProtocol }
                    Label { text: qsTr("Протокол (сервер):") }
                    Label {
                        text: "[" + sidecar.serverMinProtocol + ", "
                              + sidecar.serverProtocol + "]"
                    }
                    Label { text: qsTr("Сборка сервера:") }
                    Label { text: sidecar.serverBuild }
                    Label { text: qsTr("Стиль интерфейса:") }
                    Label { text: controlsStyle; font.bold: true }
                }
            }

            GroupBox {
                title: qsTr("Недоступно через сайдкар")
                Layout.fillWidth: true
                Layout.leftMargin: 24
                Layout.rightMargin: 24
                Layout.bottomMargin: 24
                Label {
                    width: parent.width
                    wrapMode: Text.WordWrap
                    opacity: 0.7
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
