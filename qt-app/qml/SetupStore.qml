// What the app still needs before it can do its job: the sidecar's
// `GET /api/v1/setup-status` (see setup_status_service.rs). A singleton so the
// recording screen, and later the first-run steps and the launch notice, all
// read one answer.
//
// The request is safe to send at any moment: the sidecar answers without
// reading a key from the OS keychain or opening a capture device, so it never
// raises a system dialog.
pragma Singleton
import QtQuick

QtObject {
    id: store

    // False until the first answer arrives. Nothing is reported as missing
    // before that: an unknown state must not look like a broken one.
    property bool loaded: false
    property var snapshot: ({})

    // "ready" | "downloading" | "missing"
    readonly property string transcriptionState:
        loaded && snapshot.transcription ? snapshot.transcription.state : "ready"
    // "model_not_selected" (never set up) | "model_missing" (its file is gone) | ""
    readonly property string transcriptionReason:
        loaded && snapshot.transcription && snapshot.transcription.reason
        ? snapshot.transcription.reason : ""
    // Whole percent of the running model download, -1 when the size is unknown.
    readonly property int downloadPercent: {
        var job = loaded && snapshot.transcription ? snapshot.transcription.installation : null
        return job && job.percent !== null && job.percent !== undefined
            ? Math.floor(job.percent) : -1
    }
    // "ready" | "missing" | "skipped" | "locked"
    readonly property string minutesState:
        loaded && snapshot.minutes ? snapshot.minutes.state : "ready"
    readonly property bool cpuSupported:
        !loaded || snapshot.cpu_supported !== false

    // One request at a time. An ask that arrives while one is in flight is
    // kept and sent when that one answers: the answer on its way describes
    // the state before whatever prompted the new ask.
    function refresh() {
        if (_get.busy)
            _refreshAgain = true
        else
            _get.get("/api/v1/setup-status")
    }
    property bool _refreshAgain: false
    function _answered() {
        if (_refreshAgain) {
            _refreshAgain = false
            _get.get("/api/v1/setup-status")
        }
    }

    // ── model install, owned here so it outlives whichever screen started it ──
    // The catalogue entry to offer a user who has no model: { id, display_name,
    // approximate_size, ... } or null until the catalogue has loaded.
    property var recommendedModel: null
    // The id being downloaded by installModel(), "" when none.
    property string installingModelId: ""
    // Whole percent of that download, -1 while the size is unknown.
    property int installPercent: -1
    // Why the last installModel() failed, "" when it did not.
    property string installError: ""
    readonly property bool installing: installingModelId.length > 0

    function loadCatalog() {
        if (!_catalog.busy)
            _catalog.get("/api/v1/transcription-models")
    }

    // Download a catalogue model and, once it is on disk, make it the selected
    // one. Selecting is part of the job: a downloaded model nobody selected
    // still leaves the first transcription failing.
    function installModel(modelId) {
        if (installing || !modelId)
            return
        installError = ""
        installPercent = -1
        installingModelId = modelId
        _install.post("/api/v1/transcription-models/" + encodeURIComponent(modelId) + "/install", {})
    }

    // The user's "later" for minutes: a missing key then reads as skipped.
    function skipMinutes(onDone) {
        _withSettings(function () {
            var next = JSON.parse(JSON.stringify(SettingsStore.snapshot))
            next.minutes_setup_skipped = true
            SettingsStore.apply(next, onDone)
        })
    }

    // Make `provider` the one that writes minutes. `onDone(ok, error)`.
    function setActiveProvider(provider, onDone) {
        _withSettings(function () {
            if (SettingsStore.llm().active === provider) {
                onDone(true, "")
                return
            }
            var next = JSON.parse(JSON.stringify(SettingsStore.snapshot))
            if (!next.llm)
                next.llm = {}
            next.llm.active = provider
            SettingsStore.apply(next, function (ok, res) { onDone(ok, ok ? "" : String(res)) })
        })
    }

    property string _installJobId: ""
    property var _afterSettings: null

    // Run `fn` once the settings snapshot is there to be edited.
    function _withSettings(fn) {
        if (SettingsStore.loaded) {
            fn()
        } else {
            _afterSettings = fn
            SettingsStore.refresh()
        }
    }

    function _installFailed(message) {
        installError = message
        installingModelId = ""
        _installJobId = ""
        refresh()
    }

    function _selectInstalled(modelId) {
        _withSettings(function () {
            var next = JSON.parse(JSON.stringify(SettingsStore.snapshot))
            if (!next.transcriber)
                next.transcriber = {}
            next.transcriber.model_source = "managed"
            next.transcriber.model_id = modelId
            SettingsStore.apply(next, function (ok, res) {
                store.installingModelId = ""
                store._installJobId = ""
                if (!ok)
                    store.installError = String(res)
                store.refresh()
            })
        })
    }

    property Request _catalog: Request {
        onOk: function (json) {
            var models = (json && json.models) ? json.models : []
            var wanted = store.snapshot.transcription
                       ? store.snapshot.transcription.recommended_model_id : ""
            for (var i = 0; i < models.length; ++i) {
                if (models[i].id === wanted) {
                    store.recommendedModel = models[i]
                    return
                }
            }
        }
        onFail: function (s, e) {}
    }

    property Request _install: Request {
        onOk: function (json) {
            store._installJobId = (json && json.job_id) ? json.job_id : ""
            store.refresh()
        }
        onFail: function (s, e, body) {
            // 409: the file is already there, so only the selection is missing.
            if (s === 409)
                store._selectInstalled(store.installingModelId)
            else
                store._installFailed(s > 0 ? qsTr("HTTP %1: %2").arg(s).arg(e) : e)
        }
    }

    property Request _installation: Request {
        onOk: function (json) {
            if (!json || !store.installing)
                return
            store.installPercent = json.percent !== null && json.percent !== undefined
                                 ? Math.floor(json.percent) : -1
            if (json.status === "done") {
                // Stop polling first: selecting takes a round trip of its own.
                store._installJobId = ""
                store._selectInstalled(store.installingModelId)
            }
            else if (json.status === "failed")
                store._installFailed(json.error || qsTr("Не удалось скачать модель."))
        }
        // The sidecar restarted and forgot the job: the download is gone.
        onFail: function (s, e) {
            if (s === 404)
                store._installFailed(qsTr("Загрузка прервалась."))
        }
    }

    property Timer _installPoll: Timer {
        interval: 1000
        repeat: true
        running: store._installJobId.length > 0
        onTriggered: if (!store._installation.busy)
                         store._installation.get("/api/v1/transcription-models/installations/"
                                                 + store._installJobId)
    }

    property Request _get: Request {
        onOk: function (json) {
            store.snapshot = json || ({})
            store.loaded = true
            store._answered()
        }
        // A failed refresh keeps the last answer: a hiccup must not flash a
        // warning on or off.
        onFail: function (s, e) { store._answered() }
    }

    // Saving settings or a key is what changes the answer, so follow them.
    property Connections _settings: Connections {
        target: SettingsStore
        function onSnapshotChanged() {
            if (store.loaded)
                store.refresh()
        }
        function onLoadedChanged() {
            if (SettingsStore.loaded && store._afterSettings) {
                var fn = store._afterSettings
                store._afterSettings = null
                fn()
            }
        }
    }

    // A download changes the answer on its own, with no settings change.
    property Timer _poll: Timer {
        interval: 2000
        repeat: true
        running: store.transcriptionState === "downloading"
        onTriggered: store.refresh()
    }
}
