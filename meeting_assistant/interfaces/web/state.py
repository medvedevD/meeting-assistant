import queue
import subprocess
import threading
from dataclasses import dataclass, field


@dataclass
class WebState:
    rec_lock: threading.Lock = field(default_factory=threading.Lock)
    rec_proc: subprocess.Popen | None = None
    rec_start: float | None = None
    rec_folder: str | None = None

    proc_lock: threading.Lock = field(default_factory=threading.Lock)
    proc_queue: queue.Queue = field(default_factory=queue.Queue)
    proc_running: bool = False
    proc_proc: subprocess.Popen | None = None
    rq_current_slug: str | None = None
