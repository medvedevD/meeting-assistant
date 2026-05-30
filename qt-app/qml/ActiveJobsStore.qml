// Global active-jobs store (backlog/active-jobs-store.md). One QML singleton
// that owns the JobPoller for every in-flight reprocess/protocol job, keyed by
// meetingId, so progress survives any navigation that destroys a screen
// (AppShell pops the StackView on every tab switch).
//
// A screen reads entryFor(meetingId)/isActive(meetingId) and feeds the cached
// `job` snapshot into a PipelineProgress in `sourceJob` mode (no second poller).
//
// The transcribe -> protocol chain is owned by the BACKEND: the generation flow
// posts the transcribe job with `then_protocol`, and the worker enqueues the
// protocol job on success. That keeps the chain alive even if the app restarts
// mid-transcription. The client's only job is to FOLLOW the chain: when a
// transcription job finishes, it adopts whatever follow-up job the backend
// enqueued for that meeting (via /active-jobs) so the visible progress moves
// straight on to the protocol step without flashing a "no protocol" state.
pragma Singleton
import QtQuick
import MeetingAssistant

QtObject {
    id: store

    // meetingId -> entry:
    //   { jobId, kind, status, job, terminalAt, poller, sweepTimer }
    // `kind` is a client label: "transcribe" or "protocol" (backend kinds are
    // normalized via _clientKind). terminalAt is 0 while live, Date.now() once
    // done/failed (then swept).
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
        _onEnqueued(meetingId, jobId, _clientKind(kind || ""))
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

    // Protocol generation. With a transcript, enqueue the protocol job directly.
    // Without one, enqueue a transcribe job that asks the backend to chain into
    // the protocol step on success (then_protocol); the client adopts that
    // backend-enqueued protocol job when the transcription finishes.
    function startGeneration(meetingId, hasTranscript, templateName) {
        if (hasTranscript === true)
            _enqueue(meetingId, "protocol", templateName, false)
        else
            _enqueue(meetingId, "transcribe", templateName, true)
    }

    function clear(meetingId) { _untrack(meetingId); _touch() }

    // ── internals ────────────────────────────────────────────────────────────
    function _touch() { version = version + 1 }

    // Normalize a backend JobKind ("transcribe"|"reprocess_transcribe"|
    // "regenerate_protocol") to the client label the UI reasons about.
    function _clientKind(k) {
        return _isTranscribeKind(k) ? "transcribe" : "protocol"
    }
    function _isTranscribeKind(k) {
        return k === "transcribe" || k === "reprocess_transcribe"
    }

    function _enqueue(meetingId, kind, templateName, thenProtocol) {
        if (!meetingId)
            return
        var req = _reqComp.createObject(store)
        if (req === null) {
            store.enqueueFailed(meetingId, qsTr("internal error"))
            return
        }
        req.ok.connect(function (j) {
            store._onEnqueued(meetingId, (j && j.job_id) ? j.job_id : "", kind)
            req.destroy()
        })
        req.fail.connect(function (s, e) {
            store.enqueueFailed(meetingId, e)
            req.destroy()
        })
        var body = { "kind": kind }
        if (templateName && templateName.length > 0)
            body["template_name"] = templateName
        if (thenProtocol === true)
            body["then_protocol"] = true
        req.post("/api/v1/meetings/" + meetingId + "/reprocess", body)
    }

    function _onEnqueued(meetingId, jobId, kind) {
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
            "terminalAt": 0, "poller": poller, "sweepTimer": null
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
        // Stop the finished job's poller first: the adoption lookup below is
        // async, and a still-running poller would re-emit "done" every tick and
        // fire it repeatedly.
        if (e.poller)
            e.poller.stop()
        // A successful transcription may have a backend-chained protocol job
        // (then_protocol). Adopt it so the entry follows the chain without
        // flashing a "no protocol" state. Self-correcting: if there is no
        // follow-up (plain re-transcribe), adoption finalizes the entry.
        if (status === "done" && _isTranscribeKind(e.kind)) {
            _adoptFollowUp(meetingId, job)
            return
        }
        _finalize(meetingId, status, job)
    }

    // Look up the meeting's next in-flight job (the protocol job the worker
    // enqueued) and re-point this entry's poller at it; finalize if there's none.
    function _adoptFollowUp(meetingId, transcribeJob) {
        var req = _reqComp.createObject(store)
        if (req === null) {
            _finalize(meetingId, "done", transcribeJob)
            return
        }
        req.ok.connect(function (jobs) {
            var found = null
            if (jobs && jobs.length !== undefined)
                for (var i = 0; i < jobs.length; ++i)
                    if (jobs[i] && jobs[i].meeting_id === meetingId) {
                        found = jobs[i]
                        break
                    }
            if (found)
                store._repoint(meetingId, found.id, found.kind || "")
            else
                store._finalize(meetingId, "done", transcribeJob)
            req.destroy()
        })
        req.fail.connect(function (s, e) {
            store._finalize(meetingId, "done", transcribeJob)
            req.destroy()
        })
        req.get("/api/v1/active-jobs")
    }

    // Re-point a live entry at a new job id (the chained protocol job), keeping
    // terminalAt at 0 so the screen stays in its "running" state.
    function _repoint(meetingId, jobId, backendKind) {
        var e = _jobs[meetingId]
        if (!e || e.terminalAt !== 0)
            return
        if (e.poller) { e.poller.stop(); e.poller.destroy() }
        var poller = _pollerComp.createObject(store, { "api": api, "jobId": jobId })
        if (poller === null) {
            _finalize(meetingId, "done", e.job)
            return
        }
        _patch(meetingId, {
            "jobId": jobId, "kind": _clientKind(backendKind),
            "status": "pending", "job": ({}), "poller": poller
        })
        poller.statusChanged.connect(function (s, j) { store._onUpdate(meetingId, s, j) })
        poller.jobUpdated.connect(function (s, j) { store._onUpdate(meetingId, s, j) })
        poller.failed.connect(function (er) { store._onPollerFailed(meetingId, er) })
        poller.start()
        _touch()
    }

    function _finalize(meetingId, status, job) {
        var e = _jobs[meetingId]
        if (!e || e.terminalAt !== 0)
            return
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
        _touch()
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
