"""Web GUI для управления записью и обработкой встреч."""

import http.server
import shutil
import socketserver
import json
import re
import subprocess
import threading
import webbrowser
import socket
import os
import time
import queue
import signal
import sys
from pathlib import Path
from urllib.parse import urlparse, parse_qs

ROOT_DIR = Path(__file__).parent.parent
SCRIPTS_DIR = ROOT_DIR / "scripts"
MEETINGS_DIR = ROOT_DIR / "meetings"
VENV_PYTHON = ROOT_DIR / ".venv" / "bin" / "python3"

sys.path.insert(0, str(SCRIPTS_DIR))
from gui_config import (
    load as load_config,
    save as save_config,
    load_templates,
    load_models,
    save_template,
    delete_template,
)

_SENTINEL = object()


class State:
    rec_lock = threading.Lock()
    rec_proc: subprocess.Popen | None = None
    rec_start: float | None = None
    rec_folder: str | None = None

    proc_lock = threading.Lock()
    proc_queue: queue.Queue = queue.Queue()
    proc_running: bool = False


state = State()


def _json(handler, data, status=200):
    body = json.dumps(data, ensure_ascii=False).encode()
    handler.send_response(status)
    handler.send_header("Content-Type", "application/json; charset=utf-8")
    handler.send_header("Content-Length", str(len(body)))
    handler.end_headers()
    handler.wfile.write(body)


def _read_body(handler) -> dict:
    length = int(handler.headers.get("Content-Length", 0))
    raw = handler.rfile.read(length)
    return json.loads(raw) if raw else {}


def _pactl_sources() -> dict:
    try:
        src_out = subprocess.check_output(["pactl", "list", "sources", "short"], text=True)
        snk_out = subprocess.check_output(["pactl", "list", "sinks", "short"], text=True)
    except Exception:
        return {"mics": [], "sinks": []}
    mics = [
        p[1] for line in src_out.splitlines()
        if len(p := line.split("\t")) >= 2 and not p[1].endswith(".monitor")
    ]
    sinks = [
        p[1] + ".monitor" for line in snk_out.splitlines()
        if len(p := line.split("\t")) >= 2
    ]
    return {"mics": mics, "sinks": sinks}


def _list_recordings() -> list:
    """Meeting folders that have a recording.wav (not yet processed or already processed)."""
    MEETINGS_DIR.mkdir(exist_ok=True)
    result = []
    for d in sorted(MEETINGS_DIR.iterdir(), reverse=True):
        if d.is_dir() and (d / "recording.wav").exists():
            result.append(d.name)
    return result


def _list_meetings() -> list:
    """Meeting folders that have at least a transcript or protocol."""
    MEETINGS_DIR.mkdir(exist_ok=True)
    result = []
    for d in sorted(MEETINGS_DIR.iterdir(), reverse=True):
        if d.is_dir() and ((d / "transcript.md").exists() or (d / "protocol.md").exists()):
            result.append(d.name)
    return result


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        pass

    def do_GET(self):
        path = urlparse(self.path).path
        routes = {
            "/": self._html,
            "/api/config": self._get_config,
            "/api/sources": self._get_sources,
            "/api/recordings": self._get_recordings,
            "/api/meetings": self._get_meetings,
            "/api/meeting": self._get_meeting,
            "/api/templates": self._get_templates,
            "/api/models": self._get_models,
            "/api/record/status": self._get_rec_status,
            "/api/process/stream": self._get_proc_stream,
            "/api/ollama/status": self._get_ollama_status,
        }
        fn = routes.get(path)
        if fn:
            fn()
        else:
            self.send_error(404)

    def do_POST(self):
        path = urlparse(self.path).path
        routes = {
            "/api/config": self._post_config,
            "/api/template/save": self._post_template_save,
            "/api/template/delete": self._post_template_delete,
            "/api/record/start": self._post_rec_start,
            "/api/record/stop": self._post_rec_stop,
            "/api/process/start": self._post_proc_start,
            "/api/meeting/delete": self._post_meeting_delete,
        }
        fn = routes.get(path)
        if fn:
            fn()
        else:
            self.send_error(404)

    def _html(self):
        body = HTML.encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _get_config(self):
        cfg = load_config()
        if cfg["api"].get("anthropic_api_key"):
            cfg["api"]["anthropic_api_key"] = "***"
        if cfg["api"].get("gemini_api_key"):
            cfg["api"]["gemini_api_key"] = "***"
        if cfg["api"].get("mistral_api_key"):
            cfg["api"]["mistral_api_key"] = "***"
        _json(self, cfg)

    def _get_sources(self):
        _json(self, _pactl_sources())

    def _get_recordings(self):
        _json(self, {"folders": _list_recordings()})

    def _get_meetings(self):
        _json(self, {"folders": _list_meetings()})

    def _get_meeting(self):
        qs = parse_qs(urlparse(self.path).query)
        folder = qs.get("folder", [""])[0]
        if not folder or not re.match(r'^[\w\-\.\ ]+$', folder):
            _json(self, {"error": "invalid folder"}, 400)
            return
        meeting_dir = (MEETINGS_DIR / folder).resolve()
        if not str(meeting_dir).startswith(str(MEETINGS_DIR.resolve())):
            _json(self, {"error": "invalid path"}, 400)
            return
        protocol_path = meeting_dir / "protocol.md"
        transcript_path = meeting_dir / "transcript.md"
        _json(self, {
            "protocol": protocol_path.read_text(encoding="utf-8") if protocol_path.exists() else None,
            "transcript": transcript_path.read_text(encoding="utf-8") if transcript_path.exists() else None,
        })

    def _get_templates(self):
        cfg = load_config()
        _json(self, {
            "templates": load_templates(),
            "active": cfg.get("protocol", {}).get("active_template", ""),
        })

    def _get_models(self):
        _json(self, load_models())

    def _get_ollama_status(self):
        import urllib.request
        qs = parse_qs(urlparse(self.path).query)
        base_url = qs.get("base_url", ["http://localhost:11434/v1"])[0].rstrip("/")
        try:
            req = urllib.request.Request(f"{base_url}/models", headers={"Accept": "application/json"})
            with urllib.request.urlopen(req, timeout=3) as resp:
                data = json.loads(resp.read())
            models = [m["id"] for m in data.get("data", [])]
            _json(self, {"ok": True, "models": models})
        except Exception as exc:
            _json(self, {"ok": False, "error": str(exc)})

    def _get_rec_status(self):
        with state.rec_lock:
            running = state.rec_proc is not None and state.rec_proc.poll() is None
            elapsed = int(time.time() - state.rec_start) if running and state.rec_start else 0
            _json(self, {"running": running, "elapsed": elapsed, "folder": state.rec_folder})

    def _get_proc_stream(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "keep-alive")
        self.end_headers()
        try:
            while True:
                try:
                    item = state.proc_queue.get(timeout=20)
                    if item is _SENTINEL:
                        self.wfile.write(b"event: done\ndata: \n\n")
                        self.wfile.flush()
                        state.proc_queue.put(_SENTINEL)
                        break
                    line = item.replace("\r", "").strip()
                    if line:
                        data = json.dumps(line, ensure_ascii=False)
                        self.wfile.write(f"data: {data}\n\n".encode())
                        self.wfile.flush()
                except queue.Empty:
                    self.wfile.write(b": ping\n\n")
                    self.wfile.flush()
        except Exception:
            pass

    def _post_config(self):
        data = _read_body(self)
        cfg = load_config()
        for section in ("transcription", "recording", "protocol"):
            if section in data and isinstance(data[section], dict):
                cfg[section].update(data[section])
        if "api" in data:
            key = data["api"].get("anthropic_api_key", "***")
            if key and key != "***":
                cfg["api"]["anthropic_api_key"] = key
            gkey = data["api"].get("gemini_api_key", "***")
            if gkey and gkey != "***":
                cfg["api"]["gemini_api_key"] = gkey
            mkey = data["api"].get("mistral_api_key", "***")
            if mkey and mkey != "***":
                cfg["api"]["mistral_api_key"] = mkey
        save_config(cfg)
        _json(self, {"ok": True})

    def _post_template_save(self):
        data = _read_body(self)
        name = data.get("name", "").strip()
        prompt = data.get("prompt", "")
        active = data.get("active", "")
        if not name:
            _json(self, {"error": "name required"}, 400)
            return
        save_template(name, prompt)
        if active:
            cfg = load_config()
            cfg["protocol"]["active_template"] = active
            save_config(cfg)
        _json(self, {"ok": True})

    def _post_template_delete(self):
        data = _read_body(self)
        name = data.get("name", "").strip()
        if not name:
            _json(self, {"error": "name required"}, 400)
            return
        delete_template(name)
        _json(self, {"ok": True})

    def _post_meeting_delete(self):
        data = _read_body(self)
        folder = data.get("folder", "")
        what = data.get("what", "all")  # "all" | "transcript" | "protocol"
        if not folder or not re.match(r'^[\w\-\.\ ]+$', folder):
            _json(self, {"error": "invalid folder"}, 400)
            return
        meeting_dir = (MEETINGS_DIR / folder).resolve()
        if not str(meeting_dir).startswith(str(MEETINGS_DIR.resolve())):
            _json(self, {"error": "invalid path"}, 400)
            return
        if not meeting_dir.exists():
            _json(self, {"error": "not found"}, 404)
            return
        if what == "all":
            shutil.rmtree(meeting_dir)
        elif what == "transcript":
            (meeting_dir / "transcript.md").unlink(missing_ok=True)
        elif what == "protocol":
            (meeting_dir / "protocol.md").unlink(missing_ok=True)
        else:
            _json(self, {"error": "unknown what"}, 400)
            return
        _json(self, {"ok": True})

    def _post_rec_start(self):
        data = _read_body(self)
        raw = data.get("name", "").strip()
        prepend = data.get("prepend_date", True)
        if raw and prepend and not re.match(r'^\d{4}-\d{2}-\d{2}', raw):
            raw = time.strftime("%Y-%m-%d_%H-%M") + "_" + raw
        name = raw or time.strftime("%Y-%m-%d_%H-%M")
        with state.rec_lock:
            if state.rec_proc and state.rec_proc.poll() is None:
                _json(self, {"error": "already recording"}, 400)
                return
            state.rec_folder = name
            state.rec_proc = subprocess.Popen(
                [str(SCRIPTS_DIR / "record-meeting.sh"), name],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            state.rec_start = time.time()
        _json(self, {"ok": True, "folder": name})

    def _post_rec_stop(self):
        data = _read_body(self)
        new_name = re.sub(r"[^\w\-]", "_", data.get("name", "").strip())
        with state.rec_lock:
            if state.rec_proc and state.rec_proc.poll() is None:
                state.rec_proc.send_signal(signal.SIGTERM)
                try:
                    state.rec_proc.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    state.rec_proc.kill()
            old_folder = state.rec_folder or ""
            if new_name and new_name != old_folder:
                old_path = MEETINGS_DIR / old_folder
                new_path = MEETINGS_DIR / new_name
                if old_path.exists() and not new_path.exists():
                    old_path.rename(new_path)
                    state.rec_folder = new_name
        _json(self, {"ok": True, "folder": state.rec_folder})

    def _post_proc_start(self):
        data = _read_body(self)
        folder = data.get("folder", "")
        name = data.get("name", "").strip()
        model = data.get("model", "")
        no_protocol = data.get("no_protocol", False)
        from_transcript = data.get("from_transcript", False)

        if not folder or not re.match(r'^[\w\-\.\ ]+$', folder):
            _json(self, {"error": "folder required"}, 400)
            return
        meeting_dir = (MEETINGS_DIR / folder).resolve()
        if not str(meeting_dir).startswith(str(MEETINGS_DIR.resolve())):
            _json(self, {"error": "invalid path"}, 400)
            return

        with state.proc_lock:
            if state.proc_running:
                _json(self, {"error": "already processing"}, 400)
                return
            state.proc_running = True

        while not state.proc_queue.empty():
            try:
                state.proc_queue.get_nowait()
            except queue.Empty:
                break

        def run():
            audio_path = str(MEETINGS_DIR / folder / "recording.wav")
            cmd = [str(VENV_PYTHON), "-u", str(SCRIPTS_DIR / "process_meeting.py"), audio_path]
            if name:
                cmd.append(name)
            if model:
                cmd += ["--model", model]
            if no_protocol:
                cmd.append("--no-protocol")
            if from_transcript:
                cmd.append("--from-transcript")

            env = os.environ.copy()
            api_cfg = load_config()["api"]
            if not env.get("ANTHROPIC_API_KEY"):
                key = api_cfg.get("anthropic_api_key", "")
                if key and key != "***":
                    env["ANTHROPIC_API_KEY"] = key
            if not env.get("GEMINI_API_KEY"):
                gkey = api_cfg.get("gemini_api_key", "")
                if gkey and gkey != "***":
                    env["GEMINI_API_KEY"] = gkey
            if not env.get("MISTRAL_API_KEY"):
                mkey = api_cfg.get("mistral_api_key", "")
                if mkey and mkey != "***":
                    env["MISTRAL_API_KEY"] = mkey

            try:
                proc = subprocess.Popen(
                    cmd,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    text=True,
                    bufsize=1,
                    env=env,
                )
                for line in proc.stdout:
                    state.proc_queue.put(line)
                proc.wait()
            except Exception as e:
                state.proc_queue.put(f"Ошибка: {e}\n")
            finally:
                state.proc_queue.put(_SENTINEL)
                with state.proc_lock:
                    state.proc_running = False

        threading.Thread(target=run, daemon=True).start()
        _json(self, {"ok": True})


class Server(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True


HTML = """<!DOCTYPE html>
<html lang="ru">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Meeting Assistant</title>
<style>
*{box-sizing:border-box;margin:0;padding:0}
body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;background:#0f172a;color:#e2e8f0;min-height:100vh}
header{background:#1e293b;padding:14px 24px;border-bottom:1px solid #334155;display:flex;align-items:center;gap:10px}
header h1{font-size:1.1rem;font-weight:600;color:#f1f5f9}
.badge{background:#3b82f6;color:#fff;font-size:.7rem;padding:2px 8px;border-radius:999px}
.container{max-width:700px;margin:0 auto;padding:20px 16px;display:flex;flex-direction:column;gap:14px}
.card{background:#1e293b;border:1px solid #334155;border-radius:12px;overflow:hidden}
.card-head{padding:13px 18px;display:flex;align-items:center;justify-content:space-between;cursor:pointer;user-select:none;border-bottom:1px solid transparent}
.card-head.open{border-bottom-color:#334155}
.card-head:hover{background:#263548}
.card-head h2{font-size:.8rem;font-weight:600;text-transform:uppercase;letter-spacing:.06em;color:#94a3b8}
.arrow{color:#475569;transition:transform .2s;font-size:.8rem}
.card-head.open .arrow{transform:rotate(180deg)}
.card-body{padding:18px;display:none;flex-direction:column;gap:12px}
.card-body.open{display:flex}
.field{display:flex;flex-direction:column;gap:5px}
label{font-size:.8rem;color:#94a3b8;font-weight:500}
input,select,textarea{background:#0f172a;border:1px solid #334155;color:#e2e8f0;border-radius:8px;padding:9px 11px;font-size:.88rem;width:100%;outline:none;transition:border-color .15s;font-family:inherit}
textarea{resize:vertical;min-height:140px;font-family:'Courier New',monospace;font-size:.82rem}
input:focus,select:focus,textarea:focus{border-color:#3b82f6}
.row{display:flex;gap:10px}
.row .field{flex:1}
.chk-row{display:flex;align-items:center;gap:8px}
.chk-row input[type=checkbox]{width:16px;height:16px;accent-color:#3b82f6;cursor:pointer;flex-shrink:0}
.chk-row label{font-size:.88rem;color:#e2e8f0;cursor:pointer}
.btn{padding:9px 16px;border-radius:8px;border:none;font-size:.88rem;font-weight:500;cursor:pointer;transition:all .15s;white-space:nowrap}
.btn-blue{background:#3b82f6;color:#fff}
.btn-blue:hover{background:#2563eb}
.btn-red{background:#ef4444;color:#fff}
.btn-red:hover{background:#dc2626}
.btn-ghost{background:#1e293b;color:#94a3b8;border:1px solid #334155}
.btn-ghost:hover{background:#263548;color:#e2e8f0}
.btn:disabled{opacity:.4;cursor:not-allowed}
.card-locked{opacity:.45;pointer-events:none;user-select:none}
.btn-del{color:#f87171!important;margin-left:auto}
.btn-row{display:flex;gap:8px;align-items:center;flex-wrap:wrap}
.status{font-size:.83rem;color:#64748b}
.status.on{color:#22c55e}
.status.err{color:#ef4444}
.dot{display:inline-block;width:7px;height:7px;border-radius:50%;background:#ef4444;animation:pulse 1s infinite;margin-right:5px}
@keyframes pulse{0%,100%{opacity:1}50%{opacity:.3}}
.terminal{background:#020617;border:1px solid #1e293b;border-radius:8px;padding:12px;font-family:'Courier New',monospace;font-size:.8rem;color:#94a3b8;min-height:60px;max-height:280px;overflow-y:auto;white-space:pre-wrap;word-break:break-all;margin-top:4px}
.ok{color:#22c55e}
.er{color:#ef4444}
.save-ok{font-size:.82rem;color:#22c55e;display:none}
.key-row{display:flex;gap:8px;align-items:center}
.key-row input{flex:1}
.show-btn{background:none;border:1px solid #334155;color:#64748b;padding:4px 8px;border-radius:6px;font-size:.75rem;cursor:pointer;white-space:nowrap}
.show-btn:hover{color:#94a3b8}
.hint{font-size:.75rem;color:#475569;margin-top:2px}
.item-list{display:flex;flex-direction:column;gap:4px;max-height:200px;overflow-y:auto}
.list-item{padding:8px 12px;border-radius:8px;cursor:pointer;font-size:.85rem;color:#94a3b8;border:1px solid transparent;transition:all .15s;display:flex;align-items:center;justify-content:space-between}
.list-item .del-btn{opacity:0;background:none;border:none;color:#f87171;font-size:1rem;cursor:pointer;padding:0 2px;line-height:1;flex-shrink:0}
.list-item:hover .del-btn,.list-item.active .del-btn{opacity:.6}
.list-item .del-btn:hover{opacity:1!important}
.list-item:hover{background:#263548;color:#e2e8f0;border-color:#334155}
.list-item.active{background:#1e3a5f;color:#93c5fd;border-color:#3b82f6}
.list-item.processed{color:#64748b}
.list-item.processed::after{content:' ✓';color:#22c55e;font-size:.75rem}
.no-items{font-size:.85rem;color:#475569;padding:8px 0}
.meeting-viewer{background:#020617;border:1px solid #1e293b;border-radius:8px;padding:14px;font-family:'Courier New',monospace;font-size:.8rem;color:#94a3b8;max-height:350px;overflow-y:auto;white-space:pre-wrap;word-break:break-word;display:none}
.viewer-actions{display:flex;justify-content:flex-end;gap:8px}
.tmpl-head{display:flex;gap:8px;align-items:center}
.tmpl-head select{flex:1}
.kind-tabs{display:flex;gap:0;border:1px solid #334155;border-radius:8px;overflow:hidden;margin-bottom:4px}
.kind-tab{flex:1;padding:8px 14px;font-size:.83rem;font-weight:600;border:none;cursor:pointer;transition:background .15s,color .15s;background:#0f172a;color:#64748b;text-align:center}
.kind-tab.active{background:#1e3a5f;color:#93c5fd}
.kind-tab:first-child{border-right:1px solid #334155}
.ollama-status{display:flex;align-items:center;gap:8px;font-size:.82rem;color:#64748b;padding:6px 0}
.ollama-status .dot-ok{display:inline-block;width:7px;height:7px;border-radius:50%;background:#22c55e;flex-shrink:0}
.ollama-status .dot-err{display:inline-block;width:7px;height:7px;border-radius:50%;background:#ef4444;flex-shrink:0}
.privacy-badge{display:inline-flex;align-items:center;gap:5px;font-size:.75rem;color:#22c55e;background:#052e16;border:1px solid #166534;border-radius:6px;padding:3px 8px;margin-bottom:4px}
</style>
</head>
<body>
<header>
  <h1>Meeting Assistant</h1>
  <span class="badge">Local</span>
</header>
<div class="container">

  <!-- ЗАПИСЬ -->
  <div class="card" id="card-rec">
    <div class="card-head open" onclick="toggle(this)">
      <h2>Запись</h2><span class="arrow">▾</span>
    </div>
    <div class="card-body open">
      <div class="row">
        <div class="field">
          <label>Микрофон</label>
          <select id="mic-src"><option value="">— по умолчанию —</option></select>
        </div>
        <div class="field">
          <label>Системный звук</label>
          <select id="sys-src"><option value="">— по умолчанию —</option></select>
        </div>
      </div>
      <div class="field">
        <label>Имя встречи</label>
        <input type="text" id="rec-name" placeholder="daily  (пусто = дата/время)">
        <span class="hint">Создаёт папку meetings/&lt;имя&gt;/recording.wav</span>
      </div>
      <div class="chk-row">
        <input type="checkbox" id="prepend-date" checked>
        <label for="prepend-date">Добавлять дату в начало имени</label>
      </div>
      <div class="btn-row">
        <button class="btn btn-blue" id="btn-rec-start" onclick="startRec()">● Начать запись</button>
        <button class="btn btn-red"  id="btn-rec-stop"  onclick="stopRec()" disabled>■ Остановить</button>
        <span class="status" id="rec-st">Готово к записи</span>
      </div>
    </div>
  </div>

  <!-- ОБРАБОТКА -->
  <div class="card" id="card-proc">
    <div class="card-head open" onclick="toggle(this)">
      <h2>Обработка</h2><span class="arrow">▾</span>
    </div>
    <div class="card-body open">
      <div class="row">
        <div class="field">
          <label>Встреча</label>
          <select id="proc-folder"><option value="">— выбери встречу —</option></select>
        </div>
        <div class="field">
          <label>Имя для протокола (необязательно)</label>
          <input type="text" id="proc-name" placeholder="например, 1-на-1 с коллегой">
        </div>
      </div>
      <div class="chk-row">
        <input type="checkbox" id="only-transcript">
        <label for="only-transcript">Только транскрипция (без протокола)</label>
      </div>
      <div class="btn-row">
        <button class="btn btn-blue" id="btn-proc" onclick="startProc()">▶ Запустить</button>
        <span class="status" id="proc-st"></span>
      </div>
      <div class="terminal" id="proc-out" style="display:none"></div>
    </div>
  </div>

  <!-- ВСТРЕЧИ -->
  <div class="card">
    <div class="card-head open" onclick="toggle(this)">
      <h2>Встречи</h2><span class="arrow">▾</span>
    </div>
    <div class="card-body open">
      <div class="item-list" id="meetings-list">
        <span class="no-items">Загрузка...</span>
      </div>
      <div class="viewer-actions" id="viewer-actions" style="display:none">
        <button class="btn btn-ghost" id="btn-view-transcript" onclick="showTab('transcript')">Транскрипция</button>
        <button class="btn btn-ghost" id="btn-view-protocol" onclick="showTab('protocol')">Протокол</button>
        <button class="btn btn-ghost" onclick="copyViewer()">Копировать</button>
        <button class="btn btn-ghost" id="btn-regen" onclick="regenProtocol()" style="display:none">↻ Регенерировать</button>
        <button class="btn btn-ghost btn-del" id="btn-del-file" onclick="deleteFile()">✕ файл</button>
      </div>
      <div class="meeting-viewer" id="meeting-viewer"></div>
    </div>
  </div>

  <!-- НАСТРОЙКИ ТРАНСКРИПЦИИ -->
  <div class="card" id="card-cfg">
    <div class="card-head" onclick="toggle(this)">
      <h2>Настройки транскрипции</h2><span class="arrow">▾</span>
    </div>
    <div class="card-body">
      <div class="row">
        <div class="field">
          <label>Модель Whisper</label>
          <select id="cfg-model">
            <option value="tiny">tiny — быстро, хуже</option>
            <option value="small">small</option>
            <option value="medium">medium (рекомендуется)</option>
            <option value="large-v3">large-v3 — медленно, лучше</option>
          </select>
        </div>
        <div class="field">
          <label>Язык</label>
          <input type="text" id="cfg-lang" placeholder="ru">
        </div>
      </div>
      <div class="row">
        <div class="field">
          <label>Устройство</label>
          <select id="cfg-device" onchange="updateComputeTypes()">
            <option value="cpu">CPU</option>
            <option value="cuda">GPU (CUDA)</option>
          </select>
        </div>
        <div class="field">
          <label>Тип вычислений</label>
          <select id="cfg-compute"></select>
        </div>
        <div class="field">
          <label>Beam size</label>
          <input type="number" id="cfg-beam" min="1" max="10" step="1">
        </div>
      </div>
      <div class="chk-row">
        <input type="checkbox" id="cfg-vad" onchange="toggleVad()">
        <label for="cfg-vad">Фильтр тишины (VAD)</label>
      </div>
      <div class="row" id="vad-params">
        <div class="field">
          <label>Минимальная тишина (мс)</label>
          <input type="number" id="cfg-silence" min="100" max="5000" step="100">
        </div>
        <div class="field">
          <label>Отступ вокруг речи (мс)</label>
          <input type="number" id="cfg-pad" min="0" max="1000" step="50">
        </div>
      </div>
      <div class="btn-row">
        <button class="btn btn-blue" onclick="saveConfig()">Сохранить</button>
        <span class="save-ok" id="save-ok-t">✓ Сохранено</span>
      </div>
    </div>
  </div>

  <!-- ШАБЛОНЫ ПРОМПТОВ -->
  <div class="card">
    <div class="card-head" onclick="toggle(this)">
      <h2>Шаблоны промптов</h2><span class="arrow">▾</span>
    </div>
    <div class="card-body">
      <div class="field">
        <label>Активный шаблон</label>
        <div class="tmpl-head">
          <select id="tmpl-select" onchange="onTmplSelect()"></select>
          <button class="btn btn-ghost" onclick="addTemplate()">+ Новый</button>
          <button class="btn btn-ghost" onclick="deleteTemplate()">Удалить</button>
        </div>
        <span class="hint">Файлы в папке prompts/ — каждый файл один шаблон</span>
      </div>
      <div class="field">
        <label>Промпт (используй {meeting_name} и {transcript})</label>
        <textarea id="tmpl-prompt" rows="10"></textarea>
      </div>
      <div class="btn-row">
        <button class="btn btn-blue" onclick="saveTemplate()">Сохранить шаблон</button>
        <span class="save-ok" id="save-ok-tmpl">✓ Сохранено</span>
      </div>
    </div>
  </div>

  <!-- ПРОТОКОЛ И API -->
  <div class="card">
    <div class="card-head" onclick="toggle(this)">
      <h2>Протокол и API</h2><span class="arrow">▾</span>
    </div>
    <div class="card-body">

      <!-- Вкладки Облачные / Локальные -->
      <div class="kind-tabs">
        <button class="kind-tab active" id="tab-cloud" onclick="switchKind('cloud')">☁ Облачные</button>
        <button class="kind-tab" id="tab-local" onclick="switchKind('local')">⬡ Локальные</button>
      </div>

      <!-- ОБЛАЧНЫЕ -->
      <div id="kind-cloud">
        <div class="row">
          <div class="field">
            <label>Провайдер</label>
            <select id="cfg-provider" onchange="toggleProviderFields()">
              <option value="claude">Claude (Anthropic)</option>
              <option value="gemini">Gemini (Google)</option>
              <option value="mistral">Mistral AI</option>
            </select>
          </div>
          <div class="field">
            <label>Max tokens</label>
            <select id="cfg-tokens">
              <option value="2048">2 048 — краткий протокол</option>
              <option value="4096">4 096 — стандартная встреча (~60 мин)</option>
              <option value="8192">8 192 — подробный / длинная встреча</option>
              <option value="16384">16 384 — максимум (модели с расш. выводом)</option>
            </select>
          </div>
        </div>
        <div id="claude-fields">
          <div class="row">
            <div class="field">
              <label>Модель Claude</label>
              <select id="cfg-claude"></select>
            </div>
          </div>
          <div class="field">
            <label>ANTHROPIC_API_KEY</label>
            <div class="key-row">
              <input type="password" id="cfg-key" placeholder="sk-ant-...">
              <button class="show-btn" onclick="toggleKey('cfg-key')">показать</button>
            </div>
            <span class="hint">Оставь пустым, если задан в переменной окружения</span>
          </div>
        </div>
        <div id="gemini-fields" style="display:none">
          <div class="row">
            <div class="field">
              <label>Модель Gemini</label>
              <select id="cfg-gemini"></select>
            </div>
          </div>
          <div class="field">
            <label>GEMINI_API_KEY</label>
            <div class="key-row">
              <input type="password" id="cfg-gemini-key" placeholder="AIza...">
              <button class="show-btn" onclick="toggleKey('cfg-gemini-key')">показать</button>
            </div>
            <span class="hint">Оставь пустым, если задан в переменной окружения</span>
          </div>
        </div>
        <div id="mistral-fields" style="display:none">
          <div class="row">
            <div class="field">
              <label>Модель Mistral</label>
              <select id="cfg-mistral"></select>
            </div>
          </div>
          <div class="field">
            <label>MISTRAL_API_KEY</label>
            <div class="key-row">
              <input type="password" id="cfg-mistral-key" placeholder="...">
              <button class="show-btn" onclick="toggleKey('cfg-mistral-key')">показать</button>
            </div>
            <span class="hint">Оставь пустым, если задан в переменной окружения</span>
          </div>
        </div>
      </div>

      <!-- ЛОКАЛЬНЫЕ -->
      <div id="kind-local" style="display:none">
        <span class="privacy-badge">🔒 Данные не покидают компьютер</span>
        <div class="field">
          <label>Модель Ollama</label>
          <input list="ollama-datalist" id="cfg-ollama-model" placeholder="qwen2.5:14b-instruct-q4_K_M">
          <datalist id="ollama-datalist"></datalist>
          <span class="hint">Введи имя любой установленной модели или выбери из списка</span>
        </div>
        <div class="field">
          <label>Ollama Base URL</label>
          <input type="text" id="cfg-ollama-url" placeholder="http://localhost:11434/v1">
        </div>
        <div class="ollama-status" id="ollama-status-row">
          <span id="ollama-dot" class="dot-err"></span>
          <span id="ollama-status-text">Статус неизвестен</span>
          <button class="show-btn" onclick="checkOllamaStatus()">Проверить</button>
        </div>
        <span class="hint">Установка: <code>curl -fsSL https://ollama.com/install.sh | sh</code> &nbsp;·&nbsp; Затем: <code>ollama pull qwen2.5:14b-instruct-q4_K_M</code></span>
      </div>

      <div class="btn-row" style="margin-top:8px">
        <button class="btn btn-blue" onclick="saveProtocolConfig()">Сохранить</button>
        <span class="save-ok" id="save-ok-p">✓ Сохранено</span>
      </div>
    </div>
  </div>

</div>
<script>
// --- UI helpers ---
function toggle(h){
  h.classList.toggle('open');
  h.nextElementSibling.classList.toggle('open');
}
function toggleVad(){
  document.getElementById('vad-params').style.display =
    document.getElementById('cfg-vad').checked ? '' : 'none';
}
function toggleKey(id){
  const el=document.getElementById(id||'cfg-key');
  el.type=el.type==='password'?'text':'password';
}
function toggleProviderFields(){
  const p=document.getElementById('cfg-provider').value;
  document.getElementById('claude-fields').style.display=p==='claude'?'':'none';
  document.getElementById('gemini-fields').style.display=p==='gemini'?'':'none';
  document.getElementById('mistral-fields').style.display=p==='mistral'?'':'none';
}
function switchKind(kind){
  document.getElementById('kind-cloud').style.display=kind==='cloud'?'':'none';
  document.getElementById('kind-local').style.display=kind==='local'?'':'none';
  document.getElementById('tab-cloud').classList.toggle('active',kind==='cloud');
  document.getElementById('tab-local').classList.toggle('active',kind==='local');
  if(kind==='local') checkOllamaStatus();
}
async function checkOllamaStatus(){
  const base=document.getElementById('cfg-ollama-url').value||'http://localhost:11434/v1';
  document.getElementById('ollama-status-text').textContent='Проверяю...';
  const r=await fetch('/api/ollama/status?base_url='+encodeURIComponent(base));
  const d=await r.json();
  const dot=document.getElementById('ollama-dot');
  const txt=document.getElementById('ollama-status-text');
  if(d.ok){
    dot.className='dot-ok';
    txt.textContent=`Ollama запущена · моделей установлено: ${d.models.length}`;
    const dl=document.getElementById('ollama-datalist');
    dl.innerHTML=d.models.map(m=>`<option value="${m}">`).join('');
  } else {
    dot.className='dot-err';
    txt.textContent='Ollama не отвечает — запусти: ollama serve';
  }
}
function flashOk(id){
  const el=document.getElementById(id);
  el.style.display='inline';
  setTimeout(()=>el.style.display='none',2000);
}

// --- Compute types per device ---
const COMPUTE_TYPES = {
  cpu:  [['int8','int8 (быстро)'],['float32','float32']],
  cuda: [['float16','float16 (рекомендуется)'],['int8_float16','int8_float16'],['float32','float32']],
};
function updateComputeTypes(){
  const device=document.getElementById('cfg-device').value;
  const ct=document.getElementById('cfg-compute');
  const prev=ct.value;
  ct.innerHTML='';
  COMPUTE_TYPES[device].forEach(([v,l])=>ct.add(new Option(l,v)));
  if(COMPUTE_TYPES[device].find(([v])=>v===prev)) ct.value=prev;
}

// --- Model select helpers ---
function populateSelect(id,items){
  const el=document.getElementById(id);
  el.innerHTML=items.map(m=>`<option value="${m}">${m}</option>`).join('');
}
function setSelectValue(id,val){
  const el=document.getElementById(id);
  if(val) el.value=val;
}

// --- Load config ---
async function fetchConfig(){
  const [mr,cr]=await Promise.all([fetch('/api/models'),fetch('/api/config')]);
  const models=await mr.json();
  const c=await cr.json();
  const t=c.transcription||{},p=c.protocol||{},a=c.api||{},rec=c.recording||{};
  document.getElementById('cfg-model').value=t.model||'medium';
  document.getElementById('cfg-lang').value=t.language||'ru';
  document.getElementById('cfg-device').value=t.device||'cpu';
  updateComputeTypes();
  document.getElementById('cfg-compute').value=t.compute_type||'int8';
  document.getElementById('cfg-beam').value=t.beam_size||5;
  document.getElementById('cfg-vad').checked=t.vad_filter!==false;
  document.getElementById('cfg-silence').value=t.min_silence_duration_ms||500;
  document.getElementById('cfg-pad').value=t.speech_pad_ms||200;
  // cloud models
  const cloud=models.cloud||models;  // backwards-compat if models is flat
  const local=models.local||{};
  populateSelect('cfg-claude',cloud.claude||[]);
  populateSelect('cfg-gemini',cloud.gemini||[]);
  populateSelect('cfg-mistral',cloud.mistral||[]);
  document.getElementById('cfg-provider').value=p.provider||'claude';
  setSelectValue('cfg-claude',p.claude_model);
  setSelectValue('cfg-gemini',p.gemini_model);
  setSelectValue('cfg-mistral',p.mistral_model);
  const tokensEl=document.getElementById('cfg-tokens');
  tokensEl.value=p.max_tokens||8192;
  if(!tokensEl.value)tokensEl.value=8192;
  document.getElementById('cfg-key').value=(a.anthropic_api_key==='***')?'':(a.anthropic_api_key||'');
  document.getElementById('cfg-gemini-key').value=(a.gemini_api_key==='***')?'':(a.gemini_api_key||'');
  document.getElementById('cfg-mistral-key').value=(a.mistral_api_key==='***')?'':(a.mistral_api_key||'');
  // local / ollama
  const ollamaModels=local.ollama||[];
  const dl=document.getElementById('ollama-datalist');
  dl.innerHTML=ollamaModels.map(m=>`<option value="${m}">`).join('');
  document.getElementById('cfg-ollama-model').value=p.ollama_model||ollamaModels[0]||'';
  document.getElementById('cfg-ollama-url').value=p.ollama_base_url||'http://localhost:11434/v1';
  // kind tabs
  const kind=p.kind||'cloud';
  switchKind(kind);
  toggleProviderFields();
  toggleVad();
  window._savedMic=rec.mic_source||'';
  window._savedSys=rec.system_source||'';
  document.getElementById('prepend-date').checked=rec.prepend_date!==false;
}

// --- Audio sources ---
async function fetchSources(){
  const r=await fetch('/api/sources');
  const d=await r.json();
  const ms=document.getElementById('mic-src');
  const ss=document.getElementById('sys-src');
  d.mics.forEach(s=>ms.add(new Option(s,s)));
  d.sinks.forEach(s=>ss.add(new Option(s,s)));
  if(window._savedMic) ms.value=window._savedMic;
  if(window._savedSys) ss.value=window._savedSys;
}

// --- Recordings list (for processing) ---
async function fetchRecordings(){
  const r=await fetch('/api/recordings');
  const d=await r.json();
  const sel=document.getElementById('proc-folder');
  sel.innerHTML='<option value="">— выбери встречу —</option>';
  d.folders.forEach(f=>sel.add(new Option(f,f)));
}

// --- Meetings list ---
let _meetingData={protocol:null,transcript:null};
let _activeTab='protocol';
let _activeFolder='';
async function fetchMeetings(){
  const r=await fetch('/api/meetings');
  const d=await r.json();
  const list=document.getElementById('meetings-list');
  if(!d.folders||!d.folders.length){
    list.innerHTML='<span class="no-items">Встреч пока нет</span>';
    return;
  }
  list.innerHTML='';
  d.folders.forEach(f=>{
    const item=document.createElement('div');
    item.className='list-item';
    item.dataset.folder=f;
    item.onclick=()=>viewMeeting(f,item);
    const label=document.createElement('span');
    label.textContent=f.replace(/_/g,' ');
    const del=document.createElement('button');
    del.className='del-btn';
    del.textContent='✕';
    del.title='Удалить встречу';
    del.onclick=e=>{e.stopPropagation();deleteMeeting(f);};
    item.appendChild(label);
    item.appendChild(del);
    list.appendChild(item);
  });
}
async function viewMeeting(folder,el){
  document.querySelectorAll('#meetings-list .list-item').forEach(i=>i.classList.remove('active'));
  el.classList.add('active');
  _activeFolder=folder;
  const r=await fetch('/api/meeting?folder='+encodeURIComponent(folder));
  _meetingData=await r.json();
  document.getElementById('viewer-actions').style.display='flex';
  document.getElementById('btn-view-protocol').style.display=_meetingData.protocol?'':'none';
  document.getElementById('btn-view-transcript').style.display=_meetingData.transcript?'':'none';
  document.getElementById('btn-regen').style.display=_meetingData.transcript?'':'none';
  const tab=_meetingData.protocol?'protocol':(_meetingData.transcript?'transcript':'protocol');
  showTab(tab);
}
function showTab(tab){
  _activeTab=tab;
  const content=_meetingData[tab]||'(нет данных)';
  const viewer=document.getElementById('meeting-viewer');
  viewer.style.display='block';
  viewer.textContent=content;
  document.getElementById('btn-view-protocol').classList.toggle('btn-blue',tab==='protocol');
  document.getElementById('btn-view-transcript').classList.toggle('btn-blue',tab==='transcript');
  document.getElementById('btn-view-protocol').classList.toggle('btn-ghost',tab!=='protocol');
  document.getElementById('btn-view-transcript').classList.toggle('btn-ghost',tab!=='transcript');
}
function copyViewer(){
  const content=_meetingData[_activeTab];
  if(content) navigator.clipboard.writeText(content);
}
async function deleteFile(){
  if(!_activeFolder||!_meetingData[_activeTab]) return;
  const label=_activeTab==='transcript'?'транскрипцию':'протокол';
  if(!confirm(`Удалить ${label} встречи «${_activeFolder}»?`)) return;
  await fetch('/api/meeting/delete',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({folder:_activeFolder,what:_activeTab})});
  _meetingData[_activeTab]=null;
  document.getElementById(_activeTab==='transcript'?'btn-view-transcript':'btn-view-protocol').style.display='none';
  const fallback=_activeTab==='transcript'?'protocol':'transcript';
  if(_meetingData[fallback]) showTab(fallback);
  else {
    document.getElementById('meeting-viewer').style.display='none';
    document.getElementById('viewer-actions').style.display='none';
    _activeFolder='';
    fetchMeetings();
  }
}
async function deleteMeeting(folder){
  folder=folder||_activeFolder;
  if(!folder) return;
  if(!confirm(`Удалить встречу «${folder.replace(/_/g,' ')}» целиком (запись, транскрипцию, протокол)?`)) return;
  await fetch('/api/meeting/delete',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({folder,what:'all'})});
  if(folder===_activeFolder){
    _activeFolder='';
    _meetingData={protocol:null,transcript:null};
    document.getElementById('meeting-viewer').style.display='none';
    document.getElementById('viewer-actions').style.display='none';
  }
  fetchMeetings();
}

// --- Regenerate protocol from existing transcript ---
async function regenProtocol(){
  if(!_activeFolder)return;
  if(document.getElementById('btn-proc').disabled){alert('Уже идёт обработка');return;}
  if(!confirm('Перегенерировать протокол из существующей транскрипции?'))return;
  const procCard=document.getElementById('card-proc');
  const procBody=procCard.querySelector('.card-body');
  const procHead=procCard.querySelector('.card-head');
  if(!procBody.classList.contains('open')){procBody.classList.add('open');procHead.classList.add('open');}
  procCard.scrollIntoView({behavior:'smooth'});
  const out=document.getElementById('proc-out');
  out.style.display='block';out.innerHTML='';
  document.getElementById('btn-proc').disabled=true;
  document.getElementById('btn-regen').disabled=true;
  setProcActive(true);
  const st=document.getElementById('proc-st');
  st.textContent='Регенерация протокола...';st.className='status on';
  if(evtSrc)evtSrc.close();
  evtSrc=new EventSource('/api/process/stream');
  evtSrc.onmessage=e=>{
    const line=JSON.parse(e.data);
    const div=document.createElement('div');
    if(line.startsWith('✓'))div.className='ok';
    else if(line.startsWith('Ошибка')||line.startsWith('✗'))div.className='er';
    else if(line.startsWith('⚠'))div.className='er';
    div.textContent=line;
    out.appendChild(div);out.scrollTop=out.scrollHeight;
  };
  evtSrc.addEventListener('done',()=>{
    evtSrc.close();
    document.getElementById('btn-proc').disabled=false;
    document.getElementById('btn-regen').disabled=false;
    setProcActive(false);
    st.textContent='Готово';st.className='status';
    fetchMeetings().then(()=>{
      const item=document.querySelector(`#meetings-list .list-item[data-folder="${_activeFolder}"]`);
      if(item)viewMeeting(_activeFolder,item);
    });
  });
  evtSrc.onerror=()=>{
    st.textContent='Ошибка соединения';st.className='status err';
    document.getElementById('btn-proc').disabled=false;
    document.getElementById('btn-regen').disabled=false;
    setProcActive(false);
  };
  await fetch('/api/process/start',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({folder:_activeFolder,from_transcript:true})});
}

// --- Templates ---
let _templates=[], _activeTmpl='';
async function fetchTemplates(){
  const r=await fetch('/api/templates');
  const d=await r.json();
  _templates=d.templates||[];
  _activeTmpl=d.active||'';
  renderTmplSelect();
  loadTmpl(_activeTmpl);
}
function renderTmplSelect(){
  const sel=document.getElementById('tmpl-select');
  sel.innerHTML='';
  sel.add(new Option('— без шаблона —','','',_activeTmpl===''));
  _templates.forEach(t=>sel.add(new Option(t.name,t.name,t.name===_activeTmpl,t.name===_activeTmpl)));
}
function loadTmpl(name){
  const t=_templates.find(t=>t.name===name);
  const area=document.getElementById('tmpl-prompt');
  area.value=t?t.prompt:'';
  area.disabled=!name;
  _activeTmpl=name;
}
function onTmplSelect(){
  loadTmpl(document.getElementById('tmpl-select').value);
}
async function saveTemplate(){
  const name=document.getElementById('tmpl-select').value;
  const prompt=document.getElementById('tmpl-prompt').value;
  const idx=_templates.findIndex(t=>t.name===name);
  if(idx>=0) _templates[idx].prompt=prompt;
  await fetch('/api/template/save',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({name,prompt,active:_activeTmpl})});
  flashOk('save-ok-tmpl');
}
async function addTemplate(){
  const name=prompt('Название нового шаблона:');
  if(!name||!name.trim()) return;
  const base=_templates[0]?.prompt||'';
  _templates.push({name:name.trim(),prompt:base});
  _activeTmpl=name.trim();
  await fetch('/api/template/save',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({name:name.trim(),prompt:base,active:_activeTmpl})});
  renderTmplSelect();
  document.getElementById('tmpl-select').value=_activeTmpl;
  loadTmpl(_activeTmpl);
}
async function deleteTemplate(){
  const name=document.getElementById('tmpl-select').value;
  if(!name){alert('Нечего удалять');return;}
  await fetch('/api/template/delete',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({name})});
  _templates=_templates.filter(t=>t.name!==name);
  _activeTmpl=_templates[0]?.name||'';
  renderTmplSelect();
  loadTmpl(_activeTmpl);
}

// --- Lock helpers ---
function setRecActive(on){
  document.getElementById('card-proc').classList.toggle('card-locked',on);
  document.getElementById('card-cfg').classList.toggle('card-locked',on);
}
function setProcActive(on){
  document.getElementById('card-rec').classList.toggle('card-locked',on);
  document.getElementById('card-cfg').classList.toggle('card-locked',on);
}

// --- Recording ---
let recTimer=null;
let _recFolder=null;
async function startRec(){
  const name=document.getElementById('rec-name').value.trim();
  const prependDate=document.getElementById('prepend-date').checked;
  await fetch('/api/config',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({recording:{
      mic_source:document.getElementById('mic-src').value,
      system_source:document.getElementById('sys-src').value,
      prepend_date:prependDate,
    }})});
  const r=await fetch('/api/record/start',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({name,prepend_date:prependDate})});
  const d=await r.json();
  if(d.error){alert(d.error);return;}
  _recFolder=d.folder;
  document.getElementById('btn-rec-start').disabled=true;
  document.getElementById('btn-rec-stop').disabled=false;
  recTimer=setInterval(pollRec,1000);
}
async function stopRec(){
  await fetch('/api/record/stop',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({name:_recFolder||''})});
  clearInterval(recTimer);
  setTimeout(()=>{pollRec();fetchRecordings();},1200);
}
async function pollRec(){
  const r=await fetch('/api/record/status');
  const d=await r.json();
  const st=document.getElementById('rec-st');
  if(d.running){
    const m=String(Math.floor(d.elapsed/60)).padStart(2,'0');
    const s=String(d.elapsed%60).padStart(2,'0');
    st.innerHTML=`<span class="dot"></span><span class="on">Запись идёт ${m}:${s}</span>`;
    document.getElementById('btn-rec-start').disabled=true;
    document.getElementById('btn-rec-stop').disabled=false;
    setRecActive(true);
  } else {
    st.textContent=d.folder?`Записано: ${d.folder}`:'Готово к записи';
    st.className='status';
    document.getElementById('btn-rec-start').disabled=false;
    document.getElementById('btn-rec-stop').disabled=true;
    if(recTimer){clearInterval(recTimer);recTimer=null;}
    setRecActive(false);
  }
}

// --- Processing ---
let evtSrc=null;
async function startProc(){
  const folder=document.getElementById('proc-folder').value;
  if(!folder){alert('Выбери встречу для обработки');return;}
  const name=document.getElementById('proc-name').value.trim();
  const model=document.getElementById('cfg-model').value;
  const out=document.getElementById('proc-out');
  out.style.display='block';out.innerHTML='';
  document.getElementById('btn-proc').disabled=true;
  setProcActive(true);
  const st=document.getElementById('proc-st');
  st.textContent='Обработка...';st.className='status on';
  if(evtSrc)evtSrc.close();
  evtSrc=new EventSource('/api/process/stream');
  evtSrc.onmessage=e=>{
    const line=JSON.parse(e.data);
    const div=document.createElement('div');
    if(line.startsWith('✓'))div.className='ok';
    else if(line.startsWith('Ошибка')||line.startsWith('Error'))div.className='er';
    div.textContent=line;
    out.appendChild(div);out.scrollTop=out.scrollHeight;
  };
  evtSrc.addEventListener('done',()=>{
    evtSrc.close();
    document.getElementById('btn-proc').disabled=false;
    setProcActive(false);
    st.textContent='Готово';st.className='status';
    fetchMeetings();
  });
  evtSrc.onerror=()=>{
    st.textContent='Ошибка соединения';st.className='status err';
    document.getElementById('btn-proc').disabled=false;
    setProcActive(false);
  };
  const noProtocol=document.getElementById('only-transcript').checked;
  await fetch('/api/process/start',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({folder,name,model,no_protocol:noProtocol})});
}

// --- Save transcription settings ---
async function saveConfig(){
  await fetch('/api/config',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({
      transcription:{
        model:document.getElementById('cfg-model').value,
        language:document.getElementById('cfg-lang').value,
        device:document.getElementById('cfg-device').value,
        compute_type:document.getElementById('cfg-compute').value,
        beam_size:+document.getElementById('cfg-beam').value,
        vad_filter:document.getElementById('cfg-vad').checked,
        min_silence_duration_ms:+document.getElementById('cfg-silence').value,
        speech_pad_ms:+document.getElementById('cfg-pad').value,
      },
      recording:{
        mic_source:document.getElementById('mic-src').value,
        system_source:document.getElementById('sys-src').value,
      },
    })});
  flashOk('save-ok-t');
}

// --- Save protocol & API ---
async function saveProtocolConfig(){
  const kind=document.getElementById('tab-cloud').classList.contains('active')?'cloud':'local';
  const key=document.getElementById('cfg-key').value;
  const gkey=document.getElementById('cfg-gemini-key').value;
  await fetch('/api/config',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({
      protocol:{
        kind,
        provider:document.getElementById('cfg-provider').value,
        claude_model:document.getElementById('cfg-claude').value,
        gemini_model:document.getElementById('cfg-gemini').value,
        mistral_model:document.getElementById('cfg-mistral').value,
        ollama_model:document.getElementById('cfg-ollama-model').value,
        ollama_base_url:document.getElementById('cfg-ollama-url').value||'http://localhost:11434/v1',
        max_tokens:+document.getElementById('cfg-tokens').value,
        active_template:_activeTmpl,
      },
      api:{
        anthropic_api_key:key||'***',
        gemini_api_key:gkey||'***',
        mistral_api_key:document.getElementById('cfg-mistral-key').value||'***',
      },
    })});
  flashOk('save-ok-p');
}

// --- Init ---
fetchConfig().then(fetchSources);
fetchRecordings();
fetchMeetings();
fetchTemplates();
</script>
</body>
</html>"""


def main():
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("", 0))
        port = s.getsockname()[1]

    server = Server(("127.0.0.1", port), Handler)
    url = f"http://127.0.0.1:{port}"
    print(f"Meeting Assistant → {url}")
    print("Ctrl+C для выхода")

    threading.Timer(0.4, lambda: webbrowser.open(url)).start()
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nЗавершение...")


if __name__ == "__main__":
    main()
