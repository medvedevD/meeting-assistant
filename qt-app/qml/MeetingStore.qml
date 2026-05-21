// MeetingListViewModel reimplemented client-side (audit: thin client, no core
// change). Owns the meeting-list state machine + an in-session protocol cache
// that stands in for the missing `protocolLoad` route (audit: scoped
// reimplement — cross-restart persisted-protocol view is a flagged future
// core/API route).
import QtQuick

QtObject {
    id: store

    // "loading" | "empty" | "success" | "error"  (the 4 list data-states)
    property string status: "loading"
    property var meetings: []
    property string errorMessage: ""

    // meetingId -> markdown, populated by GenerateProtocolScreen this session.
    property var protocolCache: ({})

    function protocolFor(id) {
        return protocolCache.hasOwnProperty(id) ? protocolCache[id] : ""
    }

    function cacheProtocol(id, markdown) {
        var c = protocolCache
        c[id] = markdown
        protocolCache = c // reassign so bindings re-evaluate
    }

    function meetingById(id) {
        for (var i = 0; i < meetings.length; ++i)
            if (meetings[i].id === id)
                return meetings[i]
        return null
    }

    function refresh() {
        status = "loading"
        errorMessage = ""
        _list.get("/api/v1/meetings")
    }

    // ── Import / scan ("Из папки") ───────────────────────────────────────────
    // importFile copies (default) or registers-in-place an audio file and
    // optionally queues transcription (POST /api/v1/meetings/import). On a
    // duplicate path the server returns 409 → importConflict(existingId).
    signal importDone(string meetingId, string jobId)
    signal importFailed(string message)
    signal importConflict(string existingId)
    signal scanDone(var candidates)
    signal scanFailed(string message)

    function importFile(path, copy, transcribe) {
        _import.post("/api/v1/meetings/import", {
            "path": path,
            "copy": copy === undefined ? true : copy,
            "transcribe": transcribe === undefined ? true : transcribe
        })
    }

    function scanFolder(dir) {
        if (dir && dir.length)
            _scan.get("/api/v1/meetings/scan?dir=" + encodeURIComponent(dir))
        else
            _scan.get("/api/v1/meetings/scan")
    }

    property Request _import: Request {
        onOk: function (j) {
            store.refresh()
            store.importDone(j.id, j.job_id || "")
        }
        onFail: function (s, e) {
            // ApiClient surfaces the response body in the error string; pull the
            // existing meeting id out of the 409 conflict payload if present.
            if (s === 409) {
                var m = /"existing_id"\s*:\s*"([^"]+)"/.exec(e)
                if (m) { store.importConflict(m[1]); return }
            }
            store.importFailed(s > 0 ? qsTr("HTTP %1: %2").arg(s).arg(e) : e)
        }
    }

    property Request _scan: Request {
        onOk: function (j) { store.scanDone(j ? j : []) }
        onFail: function (s, e) {
            store.scanFailed(s > 0 ? qsTr("HTTP %1: %2").arg(s).arg(e) : e)
        }
    }

    property Request _list: Request {
        onOk: function (json) {
            var arr = json ? json : []
            store.meetings = arr
            store.status = arr.length === 0 ? "empty" : "success"
        }
        onFail: function (httpStatus, error) {
            store.status = "error"
            store.errorMessage = httpStatus > 0
                ? qsTr("HTTP %1: %2").arg(httpStatus).arg(error)
                : error
        }
    }
}
