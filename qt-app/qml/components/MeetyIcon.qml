// Port of Icons.jsx — the editorial stroke-icon set. Rendered as inline SVG via
// Image (Qt's svg image reader handles rect/circle/path + fill/stroke
// faithfully), recoloured by replacing `currentColor`. viewBox is 24×24.
import QtQuick
import QtQuick.Window
import MeetingAssistant

Image {
    id: root
    property string name: ""
    property color color: Theme.ink2
    property int size: 16
    property real strokeWidth: 1.5

    width: size
    height: size
    fillMode: Image.PreserveAspectFit
    smooth: true
    // render the SVG at device pixels for crispness on hi-dpi
    sourceSize.width: size * Screen.devicePixelRatio
    sourceSize.height: size * Screen.devicePixelRatio

    readonly property var _bodies: ({
        "mic": '<rect x="9" y="3" width="6" height="12" rx="3"/><path d="M5 11a7 7 0 0 0 14 0"/><path d="M12 18v3"/>',
        "play": '<path d="M7 5l12 7-12 7z" fill="currentColor"/>',
        "pause": '<rect x="7" y="5" width="3.5" height="14" rx="1" fill="currentColor"/><rect x="13.5" y="5" width="3.5" height="14" rx="1" fill="currentColor"/>',
        "stop": '<rect x="6" y="6" width="12" height="12" rx="1.5" fill="currentColor"/>',
        "plus": '<path d="M12 5v14M5 12h14"/>',
        "refresh": '<path d="M3 12a9 9 0 0 1 15.5-6.2L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-15.5 6.2L3 16"/><path d="M3 21v-5h5"/>',
        "search": '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>',
        "gear": '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.9 2.9l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.9-2.9l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.9-2.9l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.9 2.9l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>',
        "sparkle": '<path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M5.6 18.4l2.1-2.1M16.3 7.7l2.1-2.1"/>',
        "doc": '<path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/><path d="M14 3v6h6"/><path d="M9 14h6M9 17h6M9 11h2"/>',
        "folder": '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
        "arrow-left": '<path d="M19 12H5M12 5l-7 7 7 7"/>',
        "arrow-right": '<path d="M5 12h14M12 5l7 7-7 7"/>',
        "chevron-down": '<path d="m6 9 6 6 6-6"/>',
        "chevron-up": '<path d="m18 15-6-6-6 6"/>',
        "check": '<path d="m5 13 4 4L19 7"/>',
        "more": '<circle cx="5" cy="12" r="1.5" fill="currentColor"/><circle cx="12" cy="12" r="1.5" fill="currentColor"/><circle cx="19" cy="12" r="1.5" fill="currentColor"/>',
        "waveform": '<path d="M4 10v4M8 7v10M12 4v16M16 7v10M20 10v4"/>',
        "download": '<path d="M12 3v12M7 10l5 5 5-5M5 21h14"/>',
        "copy": '<rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15V5a2 2 0 0 1 2-2h10"/>',
        "eye": '<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
        "eye-off": '<path d="M3 3l18 18"/><path d="M10.6 10.6A3 3 0 0 0 13.4 13.4"/><path d="M9.9 5.4A10.5 10.5 0 0 1 12 5c6.5 0 10 7 10 7a17.8 17.8 0 0 1-2.1 3.1"/><path d="M6.1 6.8C3.5 8.6 2 12 2 12s3.5 7 10 7a10.8 10.8 0 0 0 5.9-1.8"/>',
        "trash": '<path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2M6 6l1 14a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-14"/>',
        "edit": '<path d="M11 4H4v16h16v-7"/><path d="M18 2 22 6l-12 12H6v-4z"/>',
        "list": '<path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/>',
        "clock": '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
        "user": '<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>',
        "team": '<circle cx="9" cy="8" r="3.5"/><path d="M2.5 20a6.5 6.5 0 0 1 13 0"/><circle cx="17" cy="6" r="2.5"/><path d="M14.5 13.5A5 5 0 0 1 21.5 18"/>',
        "cpu": '<rect x="6" y="6" width="12" height="12" rx="1.5"/><rect x="9" y="9" width="6" height="6"/><path d="M10 2v3M14 2v3M10 19v3M14 19v3M2 10h3M2 14h3M19 10h3M19 14h3"/>',
        "storage": '<ellipse cx="12" cy="5" rx="8" ry="3"/><path d="M4 5v14a8 3 0 0 0 16 0V5"/><path d="M4 12a8 3 0 0 0 16 0"/>',
        "key": '<circle cx="8" cy="15" r="4"/><path d="m10.8 12.2 9.2-9.2M16 5l3 3M14 7l3 3"/>'
    })

    function _svgColor(c) {
        function h(x) { return ("0" + Math.round(x * 255).toString(16)).slice(-2) }
        return "#" + h(c.r) + h(c.g) + h(c.b)
    }

    source: {
        var body = _bodies[name] || ""
        if (body.length === 0) return ""
        var c = _svgColor(color)
        body = body.replace(/currentColor/g, c)
        var svg = "<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24' "
                + "viewBox='0 0 24 24' fill='none' stroke='" + c + "' stroke-width='"
                + strokeWidth + "' stroke-linecap='round' stroke-linejoin='round'>"
                + body + "</svg>"
        return "data:image/svg+xml;utf8," + encodeURIComponent(svg)
    }
}
