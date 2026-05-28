// Global active-jobs store (backlog/active-jobs-store.md). One QML singleton
// that owns the JobPoller for every in-flight reprocess/protocol job, keyed by
// meetingId, so progress survives any navigation that destroys a screen
// (AppShell pops the StackView on every tab switch).
//
// A screen reads entryFor(meetingId)/isActive(meetingId) and feeds the cached
// `job` snapshot into a PipelineProgress in `sourceJob` mode (no second poller).
// Enqueue + the transcribe -> protocol chain live here, not on the generation
// screen, so the chain keeps going even when that screen is gone.
pragma Singleton
import QtQuick
import MeetingAssistant

QtObject {
    id: store

    // meetingId -> entry:
    //   { jobId, kind, status, job, terminalAt, templateName, chainToProtocol,
    //     poller, sweepTimer }
    // terminalAt is 0 while live, Date.now() once done/failed (then swept).
    property var _jobs: ({})

    // Bumped on every mutation (including each poll) so bindings that read the
    // map through the functions below re-evaluate. Reactivity contract: read
    // `version` alongside entryFor/isActive in a binding to track changes.
    property int version: 0

    // Emitted once when a job (or a transcribe->protocol chain) reaches a
    // terminal state. `kind` is the terminal job's kind.
    signal jobFinished(string meetingId, string status, var job, string kind)
    // Emitted when the enqueue POST itself fails (before any polling).
    signal enqueueFailed(string meetingId, string error)

    // How long a terminal entry lingers before it is swept, so a screen the
    // user navigates back to can still show the outcome.
    readonly property int terminalTtlMs: 4000

    property Component _pollerComp: Component { JobPoller { intervalMs: 1000 } }
    property Component _reqComp: Component { Request {} }
    property Component _timerComp: Component { Timer { repeat: false } }

    // ── reads ────────────────────────────────────────────────────────────────
    function entryFor(meetingId) { return _jobs[meetingId] || null }
    function isActive(meetingId) {
        var e = _jobs[meetingId]
        return !!e && e.terminalAt === 0
    }
    function activeCount() {
        var n = 0
        for (var k in _jobs)
            if (_jobs[k] && _jobs[k].terminalAt === 0)
                n++
        return n
    }

    // ── seeding (resume after restart) ─────────────────────────────────────────
    // Register an already-running backend job without re-enqueuing it. Used to
    // re-seed the store from GET /api/v1/active-jobs after an app restart so a
    // job that was in flight when the app closed re-appears. A locally enqueued
    // entry is always at least as fresh as a seeded snapshot, so this is a no-op
    // when the meeting already has a live entry — that also makes repeated seeds
    // idempotent (never a second poller for the same meeting).
    function track(meetingId, jobId, kind) {
        if (!meetingId || !jobId || jobId.length === 0)
            return
        if (isActive(meetingId))
            return
        _onEnqueued(meetingId, jobId, kind || "", "", false)
    }

    // Fetch in-flight jobs and track each. Call once the API is configured
    // (app start / sidecar ready). Safe to call repeatedly — track() dedupes.
    function seedActive() {
        var req = _reqComp.createObject(store)
        if (req === null)
            return
        req.ok.connect(function (jobs) {
            if (jobs && jobs.length !== undefined)
                for (var i = 0; i < jobs.length; ++i) {
                    var j = jobs[i]
                    if (j && j.meeting_id && j.id)
                        store.track(j.meeting_id, j.id, j.kind || "")
                }
            req.destroy()
        })
        req.fail.connect(function (s, e) { req.destroy() })
        req.get("/api/v1/active-jobs")
    }

    // ── enqueue ──────────────────────────────────────────────────────────────
    // Single reprocess job, no chaining (menu "Перетранскрибировать" / "Перегенерировать").
    function reprocess(meetingId, kind, templateName) {
        _enqueue(meetingId, kind, templateName, false)
    }

    // Protocol generation: transcribe first when needed, then chain to protocol.
    function startGeneration(meetingId, hasTranscript, templateName) {
        if (hasTranscript === true)
            _enqueue(meetingId, "protocol", templateName, false)
        else
            _enqueue(meetingId, "transcribe", templateName, true)
    }

    function clear(meetingId) { _untrack(meetingId); _touch() }

    // ── internals ────────────────────────────────────────────────────────────
    function _touch() { version = version + 1 }

    function _enqueue(meetingId, kind, templateName, chainToProtocol) {
        if (!meetingId)
            return
        var req = _reqComp.createObject(store)
        if (req === null) {
            store.enqueueFailed(meetingId, qsTr("internal error"))
            return
        }
        req.ok.connect(function (j) {
            store._onEnqueued(meetingId, (j && j.job_id) ? j.job_id : "",
                              kind, templateName, chainToProtocol)
            req.destroy()
        })
        req.fail.connect(function (s, e) {
            store.enqueueFailed(meetingId, e)
            req.destroy()
        })
        var body = { "kind": kind }
        if (templateName && templateName.length > 0)
            body["template_name"] = templateName
        req.post("/api/v1/meetings/" + meetingId + "/reprocess", body)
    }

    function _onEnqueued(meetingId, jobId, kind, templateName, chainToProtocol) {
        if (!jobId || jobId.length === 0) {
            store.enqueueFailed(meetingId, qsTr("no job id"))
            return
        }
        _untrack(meetingId)
        var poller = _pollerComp.createObject(store, { "api": api, "jobId": jobId })
        if (poller === null) {
            store.enqueueFailed(meetingId, qsTr("internal error"))
            return
        }
        var entry = {
            "jobId": jobId, "kind": kind, "status": "pending", "job": ({}),
            "terminalAt": 0, "templateName": templateName || "",
            "chainToProtocol": chainToProtocol === true, "poller": poller,
            "sweepTimer": null
        }
        _jobs[meetingId] = entry
        poller.statusChanged.connect(function (s, j) { store._onUpdate(meetingId, s, j) })
        poller.jobUpdated.connect(function (s, j) { store._onUpdate(meetingId, s, j) })
        poller.failed.connect(function (e) { store._onPollerFailed(meetingId, e) })
        poller.start()
        _touch()
    }

    // Replace the entry with a shallow copy carrying `patch`. A fresh object
    // reference on every poll is what makes `var`-typed QML bindings on
    // entryFor()/isActive() actually re-fire (in-place mutation would be missed).
    function _patch(meetingId, patch) {
        var e = _jobs[meetingId]
        if (!e)
            return null
        var next = {}
        for (var k in e) next[k] = e[k]
        for (var p in patch) next[p] = patch[p]
        _jobs[meetingId] = next
        return next
    }

    function _onUpdate(meetingId, status, job) {
        var e = _jobs[meetingId]
        if (!e || e.terminalAt !== 0)
            return
        _patch(meetingId, { "job": job || ({}), "status": status })
        if (status === "done" || status === "failed")
            _onTerminal(meetingId, status, job || ({}))
        _touch()
    }

    function _onPollerFailed(meetingId, error) {
        var e = _jobs[meetingId]
        if (!e || e.terminalAt !== 0)
            return
        var job = { "status": "failed", "error_class": "unknown", "last_error": error }
        _patch(meetingId, { "job": job, "status": "failed" })
        _onTerminal(meetingId, "failed", job)
        _touch()
    }

    function _onTerminal(meetingId, status, job) {
        var e = _jobs[meetingId]
        if (!e || e.terminalAt !== 0)
            return
        // Chain transcribe -> protocol while a generation is still in flight.
        // Leave the entry live (terminalAt stays 0) so the UI never flashes a
        // "no protocol" state between the two jobs.
        if (status === "done" && e.chainToProtocol && e.kind === "transcribe") {
            if (e.poller)
                e.poller.stop()
            _enqueue(meetingId, "protocol", e.templateName, false)
            return
        }
        if (e.poller)
            e.poller.stop()
        _patch(meetingId, { "terminalAt": Date.now() })
        store.jobFinished(meetingId, status, job, e.kind)
        var t = _timerComp.createObject(store, { "interval": store.terminalTtlMs })
        if (t === null) {
            store.clear(meetingId)
            return
        }
        _patch(meetingId, { "sweepTimer": t })
        t.triggered.connect(function () { store.clear(meetingId) })
        t.start()
    }

    function _untrack(meetingId) {
        var e = _jobs[meetingId]
        if (!e)
            return
        if (e.poller) { e.poller.stop(); e.poller.destroy() }
        if (e.sweepTimer) { e.sweepTimer.stop(); e.sweepTimer.destroy() }
        delete _jobs[meetingId]
    }
}
