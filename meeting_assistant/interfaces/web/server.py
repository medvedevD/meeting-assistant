import asyncio
import logging
import os
import threading
import webbrowser
from contextlib import asynccontextmanager
from pathlib import Path

from fastapi import FastAPI
from fastapi.responses import FileResponse
from fastapi.staticfiles import StaticFiles

from ...infrastructure.container import build_container
from ...infrastructure.logging_setup import configure_logging
from ...application.watchdog import run_watchdog
from .state import WebState
from .routes import config, meetings, recording, processing, templates, search, upload, pipeline

configure_logging()
_log = logging.getLogger(__name__)

STATIC_DIR = Path(__file__).parent / "static"


@asynccontextmanager
async def lifespan(app: FastAPI):
    port = int(os.environ.get("SERVER_PORT", "7860"))
    url = f"http://127.0.0.1:{port}"
    _log.info("Meeting Assistant → %s", url)
    threading.Timer(0.4, lambda: webbrowser.open(url)).start()

    # Reconcile orphaned RQ jobs (re-queue anything that was running before restart)
    coordinator = getattr(app.state.container, "pipeline_coordinator", None)
    if coordinator is not None:
        try:
            requeued = coordinator.reconcile()
            if requeued:
                _log.info("Reconciled %d orphaned job(s) back to queued", requeued)
        except Exception:
            _log.exception("Reconcile failed — continuing startup")

    # Start watchdog: stale-job detection + recording-limit enforcement
    watchdog_task = asyncio.create_task(
        run_watchdog(app.state.container, web_state=app.state.web_state),
        name="watchdog",
    )

    yield

    watchdog_task.cancel()
    try:
        await watchdog_task
    except asyncio.CancelledError:
        pass


def create_app() -> FastAPI:
    application = FastAPI(title="Meeting Assistant", lifespan=lifespan)

    application.state.container = build_container()
    application.state.web_state = WebState()

    for router in [
        config.router,
        meetings.router,
        recording.router,
        processing.router,
        pipeline.router,
        templates.router,
        search.router,
        upload.router,
    ]:
        application.include_router(router, prefix="/api")

    application.mount("/static", StaticFiles(directory=str(STATIC_DIR)), name="static")

    @application.get("/", include_in_schema=False)
    async def index():
        return FileResponse(STATIC_DIR / "index.html")

    return application


app = create_app()
