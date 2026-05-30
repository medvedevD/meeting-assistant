// Phase-1 checkpoint view: renders the Meety palette + typography from Theme.qml
// so the design tokens can be approved before screens are restyled. Reachable
// only when MEETY_THEME_GALLERY=1 (see Main.qml); not part of the real nav.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import MeetingAssistant

Rectangle {
    id: gallery
    color: Theme.paper

    ScrollView {
        anchors.fill: parent
        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            width: gallery.width
            spacing: 28
            // padding via margins on a wrapper
            anchors.left: parent.left
            anchors.right: parent.right

            Item { height: 8; Layout.fillWidth: true }

            // ── header ──────────────────────────────────────────────────────
            ColumnLayout {
                Layout.leftMargin: 40
                Layout.rightMargin: 40
                Layout.fillWidth: true
                spacing: 2
                Text {
                    text: "Meety"
                    font.family: Theme.fontSerif
                    font.pixelSize: 40
                    font.weight: Theme.wMedium
                    font.letterSpacing: Theme.tracking(40, -0.02)
                    color: Theme.ink
                }
                Text {
                    text: "Design tokens — light · sienna"
                    font.family: Theme.fontSerif
                    font.italic: true
                    font.pixelSize: 16
                    color: Theme.ink2
                }
            }

            // ── palette swatches ────────────────────────────────────────────
            ColumnLayout {
                Layout.leftMargin: 40
                Layout.rightMargin: 40
                Layout.fillWidth: true
                spacing: 10
                Text {
                    text: "PALETTE"
                    font.family: Theme.fontUi
                    font.pixelSize: Theme.fsMicro
                    font.weight: Theme.wSemiBold
                    font.letterSpacing: Theme.tracking(Theme.fsMicro, 0.12)
                    color: Theme.ink3
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 12
                    Repeater {
                        model: [
                            { n: "paper", c: Theme.paper }, { n: "paperSub", c: Theme.paperSub },
                            { n: "paper3", c: Theme.paper3 }, { n: "paper4", c: Theme.paper4 },
                            { n: "ink", c: Theme.ink }, { n: "ink2", c: Theme.ink2 },
                            { n: "ink3", c: Theme.ink3 }, { n: "ink4", c: Theme.ink4 },
                            { n: "rule", c: Theme.rule }, { n: "rule2", c: Theme.rule2 },
                            { n: "accent", c: Theme.accent }, { n: "accent2", c: Theme.accent2 },
                            { n: "accentTint", c: Theme.accentTint }, { n: "rec", c: Theme.rec },
                            { n: "ok", c: Theme.ok }, { n: "warn", c: Theme.warn }
                        ]
                        delegate: ColumnLayout {
                            required property var modelData
                            spacing: 4
                            Rectangle {
                                width: 84; height: 56
                                radius: Theme.rMd
                                color: modelData.c
                                border.width: 1
                                border.color: Theme.rule
                            }
                            Text {
                                text: modelData.n
                                font.family: Theme.fontMono
                                font.pixelSize: 11
                                color: Theme.ink3
                            }
                            Text {
                                text: modelData.c
                                font.family: Theme.fontMono
                                font.pixelSize: 11
                                color: Theme.ink4
                            }
                        }
                    }
                }
            }

            Rectangle { Layout.fillWidth: true; Layout.leftMargin: 40; Layout.rightMargin: 40; height: 1; color: Theme.rule }

            // ── typography ──────────────────────────────────────────────────
            ColumnLayout {
                Layout.leftMargin: 40
                Layout.rightMargin: 40
                Layout.fillWidth: true
                spacing: 14
                Text {
                    text: "TYPOGRAPHY"
                    font.family: Theme.fontUi
                    font.pixelSize: Theme.fsMicro
                    font.weight: Theme.wSemiBold
                    font.letterSpacing: Theme.tracking(Theme.fsMicro, 0.12)
                    color: Theme.ink3
                }
                Text {
                    text: "Протокол совещания"
                    font.family: Theme.fontSerif
                    font.pixelSize: Theme.fsDisplay
                    font.weight: Theme.wMedium
                    font.letterSpacing: Theme.tracking(Theme.fsDisplay, -0.025)
                    color: Theme.ink
                }
                Text {
                    text: "Newsreader serif · заголовок раздела H1 — 44px"
                    font.family: Theme.fontSerif
                    font.pixelSize: Theme.fsTitle
                    font.weight: Theme.wMedium
                    color: Theme.ink
                }
                Text {
                    text: "ИТОГИ ВСТРЕЧИ"
                    font.family: Theme.fontUi
                    font.pixelSize: Theme.fsMicro
                    font.weight: Theme.wSemiBold
                    font.letterSpacing: Theme.tracking(Theme.fsMicro, 0.12)
                    color: Theme.ink3
                }
                Text {
                    text: "Geist UI · основной текст 14px. The quick brown fox прыгает через ленивую собаку. Цифры: 0123456789."
                    font.family: Theme.fontUi
                    font.pixelSize: Theme.fsBodyLg
                    color: Theme.ink
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }
                Text {
                    text: "JetBrains Mono · 00:14:32 · transcript.json · 1280×820"
                    font.family: Theme.fontMono
                    font.pixelSize: Theme.fsBody
                    color: Theme.ink2
                }
                Text {
                    text: "Newsreader italic · «тихий редакторский тон»"
                    font.family: Theme.fontSerif
                    font.italic: true
                    font.pixelSize: 18
                    color: Theme.ink2
                }
            }

            Rectangle { Layout.fillWidth: true; Layout.leftMargin: 40; Layout.rightMargin: 40; height: 1; color: Theme.rule }

            // ── components (Phase 2 — the real reusable widgets) ─────────────
            ColumnLayout {
                Layout.leftMargin: 40
                Layout.rightMargin: 40
                Layout.fillWidth: true
                spacing: 16

                MeetySectionLabel { label: "Components" }

                RowLayout {
                    spacing: 10
                    MeetyButton { text: "Default" }
                    MeetyButton { text: "Primary"; variant: "primary" }
                    MeetyButton { text: "Accent"; variant: "accent" }
                    MeetyButton { text: "Ghost"; variant: "ghost" }
                    MeetyButton { text: "＋"; variant: "default"; iconOnly: true }
                    MeetyButton { text: "Disabled"; enabled: false }
                }

                RowLayout {
                    spacing: 24
                    MeetyTag { text: "Транскрипт" }
                    MeetyTag { text: "00:14:32"; mono: true }
                    MeetySwitch { checked: true }
                    MeetySwitch { checked: false }
                    MeetySegmented {
                        model: ["Compact", "Regular", "Spacious"]
                        currentIndex: 1
                    }
                }

                MeetyField {
                    Layout.preferredWidth: 360
                    placeholderText: "Название встречи…"
                }

                MeetyCard {
                    Layout.fillWidth: true
                    ColumnLayout {
                        spacing: 6
                        MeetySectionLabel { label: "Решения" }
                        Text {
                            text: "Карточка (.card) — paperSub, hairline, радиус 12."
                            font.family: Theme.fontUi
                            font.pixelSize: Theme.fsBodyLg
                            color: Theme.ink
                        }
                    }
                }
            }

            Item { height: 40; Layout.fillWidth: true }
        }
    }
}
