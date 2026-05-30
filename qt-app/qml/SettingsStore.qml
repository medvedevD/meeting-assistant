// Server-side settings cache (decision #12): the single source of truth is the
// sidecar's `GET/PUT /api/v1/settings`. A QML singleton so any screen can read
// the current snapshot (recording defaults, default_template, llm.active) and
// call apply() to persist — replacing the old per-screen QtCore.Settings.
//
// Shape mirrors the sidecar snapshot (see settings_service.rs): paths{},
// recording{}, default_template, transcriber{}, llm{active, <provider>{model,
// max_tokens, base_url, has_key}}, secrets_fallback. The snapshot's read-only
// extras (has_key, secrets_fallback) round-trip harmlessly through PUT — the
// Rust DTO ignores unknown fields.
pragma Singleton
import QtQuick

QtObject {
    id: store

    // "loading" | "ready" | "saving" | "error"
    property string status: "loading"
    property string errorMessage: ""

    // The full snapshot object. Empty until the first refresh resolves.
    property var snapshot: ({})
    property bool loaded: false

    // Convenience accessors with sane fallbacks (snapshot may be {} pre-load).
    function paths()        { return snapshot.paths || ({}) }
    function recording()    { return snapshot.recording || ({ source: "mic", echo_cancel: false }) }
    function transcriber()  { return snapshot.transcriber || ({}) }
    function llm()          { return snapshot.llm || ({}) }
    function defaultTemplate() { return snapshot.default_template || "" }
    function secretsFallback() { return snapshot.secrets_fallback === true }

    function providerCfg(kind) {
        var l = llm()
        return l[kind] || ({ model: "", max_tokens: 4096, base_url: null, has_key: false })
    }

    function refresh() {
        status = "loading"
        errorMessage = ""
        _get.get("/api/v1/settings")
    }

    // Persist a full settings object (PUT replaces the whole document). Pass the
    // edited snapshot; the server returns the new sanitized snapshot. `onDone`
    // is an optional callback(success, errorOrSnapshot).
    function apply(next, onDone) {
        status = "saving"
        errorMessage = ""
        store._applyCb = onDone || null
        _put.put("/api/v1/settings", next)
    }

    // Store or clear a provider's API key. value === null deletes it.
    // `onDone(success, error)` optional.
    function setSecret(provider, value, onDone) {
        store._secretCb = onDone || null
        _secret.put("/api/v1/settings/secret",
                    { "provider": provider, "value": value })
    }

    // Probe a provider's credentials. `onDone(success, error)`.
    function testProvider(provider, onDone) {
        store._testCb = onDone || null
        _test.post("/api/v1/settings/test", { "provider": provider })
    }

    property var _applyCb: null
    property var _secretCb: null
    property var _testCb: null

    property Request _get: Request {
        onOk: function (json) {
            store.snapshot = json || ({})
            store.loaded = true
            store.status = "ready"
        }
        onFail: function (s, e) {
            store.status = "error"
            store.errorMessage = s > 0 ? qsTr("HTTP %1: %2").arg(s).arg(e) : e
        }
    }

    property Request _put: Request {
        onOk: function (json) {
            store.snapshot = json || store.snapshot
            store.status = "ready"
            if (store._applyCb) { store._applyCb(true, store.snapshot); store._applyCb = null }
        }
        onFail: function (s, e) {
            store.status = "ready"
            store.errorMessage = s > 0 ? qsTr("HTTP %1: %2").arg(s).arg(e) : e
            if (store._applyCb) { store._applyCb(false, store.errorMessage); store._applyCb = null }
        }
    }

    property Request _secret: Request {
        onOk: function (json) {
            // Re-pull so has_key flips in the snapshot.
            store.refresh()
            if (store._secretCb) { store._secretCb(true, ""); store._secretCb = null }
        }
        onFail: function (s, e) {
            var msg = s > 0 ? qsTr("HTTP %1: %2").arg(s).arg(e) : e
            if (store._secretCb) { store._secretCb(false, msg); store._secretCb = null }
        }
    }

    property Request _test: Request {
        onOk: function (json) {
            if (store._testCb) { store._testCb(true, ""); store._testCb = null }
        }
        onFail: function (s, e) {
            var msg = s > 0 ? qsTr("HTTP %1: %2").arg(s).arg(e) : e
            if (store._testCb) { store._testCb(false, msg); store._testCb = null }
        }
    }
}
