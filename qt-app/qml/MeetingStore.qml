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
