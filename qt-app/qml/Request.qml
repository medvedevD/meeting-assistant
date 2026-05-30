// One in-flight ApiClient call, request-id correlated.
//
// ApiClient is async and fans every reply through the SAME
// requestSucceeded/requestFailed signals, so each caller must filter by the
// id it was handed. This wraps that bookkeeping: `get()`/`post()` start a
// call; exactly one of `ok`/`fail` fires for it. `api` is the engine-wide
// context property set in main.cpp.
import QtQuick

QtObject {
    id: req

    property int _id: -1
    property bool busy: false

    signal ok(var json)
    signal fail(int status, string error)

    function get(path) {
        busy = true
        _id = api.get(path)
    }

    function post(path, body) {
        busy = true
        _id = api.post(path, body)
    }

    function put(path, body) {
        busy = true
        _id = api.put(path, body)
    }

    function del(path) {
        busy = true
        _id = api.del(path)
    }

    property Connections _conn: Connections {
        target: api
        function onRequestSucceeded(requestId, path, json) {
            if (requestId !== req._id)
                return
            req._id = -1
            req.busy = false
            req.ok(json)
        }
        function onRequestFailed(requestId, path, httpStatus, error) {
            if (requestId !== req._id)
                return
            req._id = -1
            req.busy = false
            req.fail(httpStatus, error)
        }
    }
}
