// Capture-device catalogue cache: the sidecar's `GET /api/v1/audio/devices`.
// A QML singleton so the New Recording screen and the Settings recording panel
// share one device list (mic inputs + system-audio sources) and one refresh.
//
// Shape mirrors the sidecar (routes/audio.rs):
//   { input:  [{ id, label, is_default }],
//     output: [{ id, label, is_default }],
//     system_selectable: bool }
// `system_selectable` is false on macOS, where the system source is the
// aggregate mix with no per-output handle — the UI hides the system picker.
pragma Singleton
import QtQuick

QtObject {
    id: store

    // "idle" | "loading" | "ready" | "error"
    property string status: "idle"
    property string errorMessage: ""
    property bool loaded: false

    property var inputs: []          // [{ id, label, is_default }]
    property var outputs: []         // [{ id, label, is_default }]
    property bool systemSelectable: true

    function refresh() {
        store.status = "loading"
        store.errorMessage = ""
        _get.get("/api/v1/audio/devices")
    }

    // Ensure the list is loaded at least once (cheap no-op if already loading/ready).
    function ensureLoaded() {
        if (store.status === "idle")
            store.refresh()
    }

    // The label to display for a pinned `id`, or the localized "default" text
    // when `id` is empty/null or no longer present in `list`.
    function labelFor(list, id, defaultText) {
        if (id) {
            for (var i = 0; i < list.length; ++i)
                if (list[i].id === id)
                    return list[i].label
        }
        return defaultText
    }

    property Request _get: Request {
        onOk: function (json) {
            store.inputs = (json && json.input) || []
            store.outputs = (json && json.output) || []
            store.systemSelectable = !json || json.system_selectable !== false
            store.loaded = true
            store.status = "ready"
        }
        onFail: function (s, e) {
            store.status = "error"
            store.errorMessage = s > 0 ? qsTr("HTTP %1: %2").arg(s).arg(e) : e
        }
    }
}
