// Port of .btn / .btn-primary / .btn-accent / .btn-ghost / .btn-icon / .btn-lg.
// Variant + size via properties; visuals fully overridden (Fusion is enforced
// app-wide, but background/contentItem replace its delegates).
import QtQuick
import QtQuick.Controls
import MeetingAssistant

Button {
    id: control

    // "default" | "primary" | "accent" | "ghost"
    property string variant: "default"
    property bool iconOnly: false
    property bool large: false
    property string iconName: ""   // optional leading icon

    activeFocusOnTab: true
    readonly property bool _accent: variant === "accent"
    readonly property bool _primary: variant === "primary"
    readonly property bool _ghost: variant === "ghost"
    readonly property color _fg: _accent ? Theme.accentInk
                               : _primary ? Theme.paper
                               : _ghost ? (hovered ? Theme.ink : Theme.ink2)
                               : Theme.ink

    font.family: Theme.fontUi
    font.pixelSize: large ? Theme.fsBodyLg : Theme.fsBody
    font.weight: Theme.wMedium

    hoverEnabled: true
    padding: large ? 10 : 7
    leftPadding: iconOnly ? 0 : (large ? 18 : 13)
    rightPadding: leftPadding
    topPadding: padding
    bottomPadding: padding

    contentItem: Item {
        implicitWidth: rowLayout.implicitWidth
        implicitHeight: rowLayout.implicitHeight
        Row {
            id: rowLayout
            anchors.centerIn: parent
            spacing: 6
            MeetyIcon {
                visible: control.iconName.length > 0
                anchors.verticalCenter: parent.verticalCenter
                name: control.iconName
                size: 14
                color: control._fg
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: control.text
                font: control.font
                verticalAlignment: Text.AlignVCenter
                elide: Text.ElideRight
                color: control._fg
            }
        }
    }

    background: Rectangle {
        implicitWidth: control.iconOnly ? 32 : 64
        implicitHeight: control.iconOnly ? 32 : (control.large ? 40 : 34)
        radius: Theme.rMd
        // 0.5px down-shift on press → tiny y translate
        y: control.down ? 0.5 : 0
        // ghost's idle fill is paper3 with 0 alpha (NOT "transparent" = black/0,
        // which makes the ColorAnimation pass through muddy grey on hover-in).
        readonly property color _ghostIdle: Qt.rgba(Theme.paper3.r, Theme.paper3.g, Theme.paper3.b, 0)
        color: {
            if (control._accent) return control.hovered ? Theme.accent2 : Theme.accent
            if (control._primary) return control.hovered ? Theme.ink2 : Theme.ink
            if (control._ghost) return control.hovered ? Theme.paper3 : _ghostIdle
            return control.hovered ? Theme.paper3 : Theme.paper
        }
        border.width: control._ghost ? 0 : 1
        border.color: {
            if (control.activeFocus) return Theme.focus
            if (control._accent) return control.hovered ? Theme.accent2 : Theme.accent
            if (control._primary) return control.hovered ? Theme.ink2 : Theme.ink
            return Theme.rule
        }
        opacity: control.enabled ? 1.0 : 0.5
        Behavior on color { ColorAnimation { duration: Theme.durBase } }
        Behavior on border.color { ColorAnimation { duration: Theme.durBase } }
    }
}
