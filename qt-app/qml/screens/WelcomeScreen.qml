// WelcomeScreen — first-run and empty-content state from the Meety design.
// The sidebar owns the meeting list; this content pane explains the first run
// when there are no meetings and falls back to a compact "choose or record"
// state when meetings exist but none is selected.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import MeetingAssistant

Page {
    id: scr
    property var shell
    property var store

    readonly property bool firstRun: ((typeof firstRunPreviewEnabled !== "undefined"
                                       && firstRunPreviewEnabled) === true)
                                     || (store !== undefined && store.status === "empty")

    font.family: Theme.fontUi
    font.pixelSize: Theme.fsBody
    background: Rectangle { color: Theme.paper }

    Loader {
        anchors.fill: parent
        sourceComponent: scr.firstRun ? firstRunComp : emptyComp
    }

    Component {
        id: emptyComp

        Item {
            anchors.fill: parent

            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(parent.width - 80, 430)
                spacing: 10

                MeetyWordmark {
                    size: 42
                    Layout.alignment: Qt.AlignHCenter
                }

                Text {
                    Layout.fillWidth: true
                    Layout.topMargin: 4
                    horizontalAlignment: Text.AlignHCenter
                    text: qsTr("Выберите встречу")
                    font.family: Theme.fontSerif
                    font.pixelSize: 28
                    font.weight: Theme.wMedium
                    font.letterSpacing: Theme.tracking(28, -0.015)
                    color: Theme.ink
                }

                Text {
                    Layout.fillWidth: true
                    Layout.maximumWidth: 380
                    Layout.alignment: Qt.AlignHCenter
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.WordWrap
                    lineHeight: 1.45
                    text: qsTr("Или начните новую запись — meety запишет, расшифрует и подготовит протокол.")
                    font.family: Theme.fontUi
                    font.pixelSize: Theme.fsBodyLg
                    color: Theme.ink3
                }

                RowLayout {
                    Layout.alignment: Qt.AlignHCenter
                    Layout.topMargin: 8
                    spacing: 8

                    MeetyButton {
                        text: qsTr("Новая запись")
                        iconName: "mic"
                        variant: "accent"
                        enabled: scr.shell !== undefined
                        onClicked: scr.shell.showNewRecording()
                    }
                    MeetyButton {
                        text: qsTr("Импорт")
                        iconName: "doc"
                        enabled: scr.shell !== undefined
                        onClicked: scr.shell.showNewRecording()
                    }
                }
            }
        }
    }

    Component {
        id: firstRunComp

        GridLayout {
            id: welcome
            anchors.fill: parent
            columns: scr.width >= 760 ? 2 : 1
            rowSpacing: 0
            columnSpacing: 0

            Rectangle {
                id: heroPane
                Layout.fillWidth: true
                Layout.fillHeight: scr.width >= 760
                Layout.preferredWidth: scr.width >= 760 ? Math.round(scr.width * 0.51) : scr.width
                Layout.preferredHeight: scr.width >= 760 ? scr.height : 220
                color: Theme.paperSub
                clip: true

                Rectangle {
                    width: Math.max(heroPane.width * 1.15, 560)
                    height: Math.max(heroPane.height * 0.58, 360)
                    radius: Math.min(width, height) / 2
                    x: -width * 0.23
                    y: heroPane.height - height * 0.78
                    color: Theme.accentTint
                    opacity: 0.62
                }

                Rectangle {
                    width: Math.max(heroPane.width * 0.82, 420)
                    height: Math.max(heroPane.height * 0.46, 300)
                    radius: Math.min(width, height) / 2
                    x: heroPane.width - width * 0.72
                    y: heroPane.height * 0.28
                    color: "#F0E2D2"
                    opacity: 0.36
                }

                Rectangle {
                    anchors.fill: parent
                    color: Qt.rgba(Theme.paperSub.r, Theme.paperSub.g, Theme.paperSub.b, 0.42)
                }

                Rectangle {
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    width: scr.width >= 760 ? 1 : 0
                    color: Theme.rule
                }
                Rectangle {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    height: scr.width >= 760 ? 0 : 1
                    color: Theme.rule
                }

                ProtocolMockup {
                    anchors.centerIn: parent
                    width: Math.min(parent.width - 80, 360)
                    scale: scr.width >= 760 ? 1.0 : 0.62
                    transformOrigin: Item.Center
                }
            }

            ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                contentWidth: availableWidth
                clip: true
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                ColumnLayout {
                    width: parent.width
                    spacing: scr.width >= 760 ? 16 : 12

                    Item { Layout.fillHeight: true; Layout.minimumHeight: scr.width >= 760 ? 36 : 18 }

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.leftMargin: scr.width >= 760 ? 44 : 32
                        Layout.rightMargin: scr.width >= 760 ? 44 : 32
                        spacing: 12

                        MeetyWordmark {
                            size: 36
                            Layout.alignment: Qt.AlignVCenter
                        }
                        Text {
                            text: "meety"
                            font.family: Theme.fontSerif
                            font.pixelSize: 26
                            font.weight: Theme.wMedium
                            font.letterSpacing: Theme.tracking(26, -0.02)
                            color: Theme.ink
                        }
                    }

                    Text {
                        Layout.fillWidth: true
                        Layout.leftMargin: scr.width >= 760 ? 44 : 32
                        Layout.rightMargin: scr.width >= 760 ? 44 : 32
                        text: qsTr("Тихий ассистент\nдля шумных встреч.")
                        wrapMode: Text.WordWrap
                        lineHeight: 1.05
                        font.family: Theme.fontSerif
                        font.pixelSize: scr.width >= 760 ? 32 : 26
                        font.weight: Theme.wMedium
                        font.letterSpacing: Theme.tracking(font.pixelSize, -0.02)
                        color: Theme.ink
                    }

                    Text {
                        Layout.fillWidth: true
                        Layout.leftMargin: scr.width >= 760 ? 44 : 32
                        Layout.rightMargin: scr.width >= 760 ? 44 : 32
                        Layout.bottomMargin: scr.width >= 760 ? 14 : 8
                        text: qsTr("Запиши, расшифруй, оформи в протокол.")
                        wrapMode: Text.WordWrap
                        lineHeight: 1.35
                        font.family: Theme.fontSerif
                        font.italic: true
                        font.pixelSize: scr.width >= 760 ? 16 : 14
                        color: Theme.ink2
                    }

                    WelcomeBullet {
                        Layout.fillWidth: true
                        Layout.leftMargin: scr.width >= 760 ? 44 : 32
                        Layout.rightMargin: scr.width >= 760 ? 44 : 32
                        number: "1"
                        title: qsTr("Разрешите «Запись экрана»")
                        desc: qsTr("для захвата системного звука. Никаких снимков экрана meety не делает.")
                    }
                    WelcomeBullet {
                        Layout.fillWidth: true
                        Layout.leftMargin: scr.width >= 760 ? 44 : 32
                        Layout.rightMargin: scr.width >= 760 ? 44 : 32
                        number: "2"
                        title: qsTr("Скачайте модель Whisper")
                        desc: qsTr("≈3 ГБ · large-v3. Транскрипция идет локально, без интернета.")
                    }
                    WelcomeBullet {
                        Layout.fillWidth: true
                        Layout.leftMargin: scr.width >= 760 ? 44 : 32
                        Layout.rightMargin: scr.width >= 760 ? 44 : 32
                        number: "3"
                        title: qsTr("Добавьте API-ключ Claude")
                        desc: qsTr("для генерации протоколов. Ключ хранится в системном Keychain.")
                    }

                    MeetyButton {
                        Layout.leftMargin: scr.width >= 760 ? 44 : 32
                        Layout.rightMargin: scr.width >= 760 ? 44 : 32
                        Layout.topMargin: 12
                        Layout.alignment: Qt.AlignLeft
                        text: qsTr("Начать первую запись")
                        iconName: "mic"
                        variant: "accent"
                        large: true
                        enabled: scr.shell !== undefined
                        onClicked: scr.shell.showNewRecording()
                    }

                    MeetyButton {
                        Layout.leftMargin: scr.width >= 760 ? 44 : 32
                        Layout.rightMargin: scr.width >= 760 ? 44 : 32
                        Layout.alignment: Qt.AlignLeft
                        text: qsTr("У меня уже есть запись — импортировать")
                        variant: "ghost"
                        enabled: scr.shell !== undefined
                        onClicked: scr.shell.showNewRecording()
                    }

                    Item { Layout.fillHeight: true; Layout.minimumHeight: scr.width >= 760 ? 36 : 26 }
                }
            }
        }
    }

    component WelcomeBullet: RowLayout {
        id: bullet
        property string number
        property string title
        property string desc

        spacing: 12

        Rectangle {
            Layout.preferredWidth: 22
            Layout.preferredHeight: 22
            Layout.alignment: Qt.AlignTop
            Layout.topMargin: 1
            radius: 11
            color: Theme.accentTint

            Text {
                anchors.centerIn: parent
                text: bullet.number
                font.family: Theme.fontUi
                font.pixelSize: Theme.fsMicro
                font.weight: Theme.wBold
                color: Theme.accent2
            }
        }

        Text {
            Layout.fillWidth: true
            text: "<b>" + bullet.title + "</b> — " + bullet.desc
            textFormat: Text.RichText
            wrapMode: Text.WordWrap
            lineHeight: 1.45
            font.family: Theme.fontUi
            font.pixelSize: scr.width >= 760 ? 14 : 13
            color: Theme.ink2
        }
    }

    component ProtocolMockup: Item {
        id: mock
        implicitWidth: 360
        implicitHeight: 386

        Rectangle {
            x: 8
            y: -8
            width: page.width
            height: page.height
            radius: Theme.rLg
            color: Theme.paperSub
            border.width: 1
            border.color: Theme.rule
            rotation: 1.4
        }

        Rectangle {
            id: page
            anchors.fill: parent
            radius: Theme.rLg
            color: Theme.paper
            border.width: 1
            border.color: Theme.rule
            rotation: -1.4

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 30
                spacing: 0

                Text {
                    Layout.fillWidth: true
                    Layout.bottomMargin: 10
                    text: qsTr("21 мая · Командная встреча").toUpperCase()
                    font.family: Theme.fontUi
                    font.pixelSize: 10
                    font.weight: Theme.wSemiBold
                    font.letterSpacing: Theme.tracking(10, 0.12)
                    color: Theme.ink3
                }

                Text {
                    Layout.fillWidth: true
                    text: qsTr("Платежный флоу — план релиза")
                    wrapMode: Text.WordWrap
                    lineHeight: 1.05
                    font.family: Theme.fontSerif
                    font.pixelSize: 22
                    font.weight: Theme.wMedium
                    font.letterSpacing: Theme.tracking(22, -0.02)
                    color: Theme.ink
                }

                Text {
                    Layout.fillWidth: true
                    Layout.topMargin: 4
                    Layout.bottomMargin: 18
                    text: qsTr("47 мин · Аня, Кирилл, Маша")
                    font.family: Theme.fontUi
                    font.pixelSize: Theme.fsMicro
                    color: Theme.ink3
                }

                MockSection { label: qsTr("Резюме") }

                Text {
                    Layout.fillWidth: true
                    Layout.topMargin: 8
                    Layout.bottomMargin: 6
                    text: qsTr("Команда обсудила интеграцию с банком и план выхода в прод следующей средой.")
                    wrapMode: Text.WordWrap
                    lineHeight: 1.4
                    font.family: Theme.fontSerif
                    font.pixelSize: Theme.fsBody
                    color: Theme.ink2
                }

                MockSection {
                    Layout.topMargin: 8
                    label: qsTr("Action items · 3")
                }

                MockRow {
                    who: qsTr("Маша · сегодня")
                    task: qsTr("Поднять доступ к staging")
                }
                MockRow {
                    who: qsTr("Кирилл · до пятницы")
                    task: qsTr("Закончить ретрай")
                }
                MockRow {
                    dividerVisible: false
                    who: qsTr("Аня · 26 мая")
                    task: qsTr("Согласовать релиз со SRE")
                }
            }
        }
    }

    component MockSection: Text {
        property string label
        Layout.fillWidth: true
        Layout.topMargin: 14
        Layout.bottomMargin: 2
        text: label.toUpperCase()
        font.family: Theme.fontUi
        font.pixelSize: 10
        font.weight: Theme.wSemiBold
        font.letterSpacing: Theme.tracking(10, 0.12)
        color: Theme.ink3

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 1
            color: Theme.rule
        }
    }

    component MockRow: Item {
        id: row
        property string task
        property string who
        property bool dividerVisible: true

        Layout.fillWidth: true
        implicitHeight: content.implicitHeight

        RowLayout {
            id: content
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            spacing: 8

            Text {
                Layout.fillWidth: true
                Layout.topMargin: 5
                Layout.bottomMargin: 5
                text: row.task
                elide: Text.ElideRight
                font.family: Theme.fontUi
                font.pixelSize: Theme.fsMicro
                color: Theme.ink2
            }
            Text {
                Layout.topMargin: 5
                Layout.bottomMargin: 5
                text: row.who
                font.family: Theme.fontUi
                font.pixelSize: Theme.fsMicro
                color: Theme.ink3
            }
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 1
            visible: row.dividerVisible
            color: Theme.rule
        }
    }
}
