// Port of .menu popover container. Use with MeetyMenuItem for rows.
import QtQuick
import QtQuick.Controls
import MeetingAssistant

Menu {
    id: control
    property int minimumWidth: 320
    property int maximumWidth: Overlay.overlay
                               ? Math.max(160, Overlay.overlay.width - 16)
                               : minimumWidth

    padding: 4
    width: Math.min(minimumWidth, maximumWidth)

    function popupFromButton(anchorItem, rightInset, belowOffset) {
        const inset = rightInset === undefined ? 32 : rightInset
        const offset = belowOffset === undefined ? 8 : belowOffset
        open()
        Qt.callLater(function () {
            const host = Overlay.overlay ? Overlay.overlay : parent
            if (anchorItem && host) {
                const p = anchorItem.mapToItem(host, 0, 0)
                x = Math.max(8, host.width - width - inset)
                y = p.y + anchorItem.height + offset
            }
        })
    }

    background: Rectangle {
        radius: Theme.rMd
        color: Theme.paper
        border.width: 1
        border.color: Theme.rule
        // shadow-2 approximation
        layer.enabled: true
    }
}
