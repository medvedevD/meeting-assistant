// Port of .sidebar-section / .card-label / .gen-leak-label etc. — the uppercase,
// letter-spaced micro caption used as a section header throughout the design.
import QtQuick
import MeetingAssistant

Text {
    id: root
    property string label: ""
    property real trackingEm: 0.12

    text: label.toUpperCase()
    font.family: Theme.fontUi
    font.pixelSize: Theme.fsMicro
    font.weight: Theme.wSemiBold
    font.letterSpacing: Theme.tracking(Theme.fsMicro, trackingEm)
    color: Theme.ink3
}
