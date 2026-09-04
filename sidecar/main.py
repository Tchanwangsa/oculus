import asyncio
import atexit
import json
import os
import threading
import time
import urllib.request
from collections import deque
from contextlib import contextmanager
from pathlib import Path
from urllib.parse import unquote

import uvicorn
from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import StreamingResponse
from pydantic import BaseModel

from parser import PARSER_VERSION, parse_fast_isolated, parse_mode as _md_state, parse_quality
from memory_governor import MIN_CAP_MB
from model_workers import EMBED_WORKER, MEMORY_GOVERNOR, QUALITY_WORKER, shutdown_workers
from parse_settings import BACKENDS, PARSE_SETTINGS


def _notify_tauri(ipc_port: int, payload: dict) -> None:
    """POST a parse-status update back to the Tauri IPC server (fire-and-forget)."""
    if not ipc_port:
        return
    try:
        data = json.dumps(payload).encode("utf-8")
        req = urllib.request.Request(
            f"http://127.0.0.1:{ipc_port}/parse-status",
            data=data,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        urllib.request.urlopen(req, timeout=3).read()
    except Exception as e:
        print(f"[notify] failed: {e}")

app = FastAPI(title="Oculus Sidecar")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["*"],
    allow_headers=["*"],
)

# One sidecar-wide gate for operations with multi-gigabyte transient memory.
# MinerU quality parsing, one-shot fast parsing, page-image embedding, and a
# first-on-demand embedding-model load must never overlap. Their individual
# concurrency was bounded before, but the separate bounds still allowed their
# transient peaks to add up and exhaust system RAM.
_heavy_condition = threading.Condition()
_heavy_queue: deque[tuple[object, str]] = deque()
_heavy_current: str | None = None


@contextmanager
def _heavy_operation(kind: str, name: str):
    """Run one memory-heavy operation, releasing the global slot on failure."""
    _acquire_heavy_operation(kind, name)
    try:
        yield
    finally:
        _release_heavy_operation()


def _acquire_heavy_operation(kind: str, name: str) -> None:
    """Wait in FIFO order for the process-wide heavy-work slot."""
    global _heavy_current
    label = f"{kind}: {name}"
    ticket = object()
    with _heavy_condition:
        _heavy_queue.append((ticket, label))
        while _heavy_current is not None or _heavy_queue[0][0] is not ticket:
            _heavy_condition.wait()
        _heavy_queue.popleft()
        _heavy_current = label


def _release_heavy_operation() -> None:
    """Release the process-wide heavy-work slot."""
    global _heavy_current
    with _heavy_condition:
        _heavy_current = None
        _heavy_condition.notify_all()


# pdf_paths waiting for the quality slot, in arrival order. Positions are
# pushed to the app so a queued file reads "#3 in line" instead of looking
# stuck while the single slot works through the backlog.
_quality_waiting: list[str] = []
_waiting_lock = threading.Lock()

# Filename currently in the quality slot, surfaced by /health.
_quality_current: str | None = None
_cloud_current: set[str] = set()
_cloud_current_lock = threading.Lock()

# Seconds-per-page EMA across finished documents. MinerU only reports real
# progress once per bounded page window, so
# between windows the UI's percentage is *estimated* from this rate by the
# heartbeat below and corrected whenever a real window completes.
_quality_rate = [1.3]

# pdf_path -> progress dict
_progress: dict[str, dict] = {}
_parse_inflight: set[str] = set()
_parse_inflight_lock = threading.Lock()

# pdf_path -> quality start time, for the elapsed/ETA readout. Formula
# enrichment makes per-page cost vary by two orders of magnitude between decks,
# so a static estimate would be useless — measure this document as it goes.
_quality_started: dict[str, float] = {}


def _status_str(state: dict) -> str:
    if state.get("done"):
        return "done"
    if state.get("error"):
        return f"error: {state['error']}"
    if state.get("queued"):
        return "queued"
    return "running"


def _broadcast_queue_positions() -> None:
    """Re-send positions to everyone still waiting after the queue moves."""
    with _waiting_lock:
        snapshot = list(_quality_waiting)
    for i, p in enumerate(snapshot):
        meta = _progress.get(p) or {}
        _notify_tauri(meta.get("ipc_port", 0), {
            "relative_path": meta.get("relative_path", ""),
            "subject_id": meta.get("subject_id", 0),
            "status": "queued",
            "position": i + 1,
        })


def _run_quality(
    pdf_path: str,
    meta: dict,
    backend_setting: str,
    mineru_token: str | None,
) -> None:
    ipc_port = meta.get("ipc_port", 0)
    rel = meta.get("relative_path", "")
    sid = meta.get("subject_id", 0)
    name = Path(pdf_path).name

    def notify(status: str, extra: dict | None = None):
        _notify_tauri(ipc_port, {
            "relative_path": rel,
            "subject_id": sid,
            "status": status,
            **(extra or {}),
        })

    _progress[pdf_path] = {"queued": True, "done": False, **meta}

    def on_progress(state: dict):
        _progress[pdf_path] = {**_progress.get(pdf_path, {}), **state, **meta}
        started = _quality_started.get(pdf_path, time.time())
        done, total = state.get("pages_done", 0), state.get("total_pages", 0)
        rate = (time.time() - started) / max(1, done)
        eta = rate * (total - done)
        chunk = (
            f"chunk {state['chunk']}/{state['total_chunks']}  "
            if "chunk" in state else ""
        )
        print(f"  [quality] {name}  {chunk}pages {done}/{total}  "
              f"{time.time() - started:.0f}s elapsed, ~{eta:.0f}s left")
        notify("running", {
            "pages_done": done,
            "total_pages": total,
            **({"backend": state["backend"]} if state.get("backend") else {}),
        })

    global _quality_current
    stop_hb = threading.Event()

    def heartbeat(total: int, started: float):
        # Estimated movement between MinerU's per-window reports, from the
        # measured rate of previous documents. Never claims completion (capped
        # at 95%) and never moves backwards past a real report.
        while not stop_hb.wait(2.0):
            if stop_hb.is_set():
                break
            real = _progress.get(pdf_path, {}).get("pages_done", 0) or 0
            est = int(min((time.time() - started) / _quality_rate[0], total * 0.95))
            done = min(max(real, est), total - 1)
            notify("running", {"pages_done": done, "total_pages": total, "estimated": True})

    try:
        import fitz
        with fitz.open(pdf_path) as document:
            total_pages = document.page_count
    except Exception:
        total_pages = 0

    def start(backend: str) -> float:
        started = time.time()
        _quality_started[pdf_path] = started
        _progress[pdf_path] = {
            "pages_done": 0,
            "total_pages": total_pages,
            "done": False,
            "queued": False,
            "backend": backend,
            **meta,
        }
        print(f"[quality] START  {name}  ({backend}, {total_pages} pages)")
        notify("running", {
            "pages_done": 0,
            "total_pages": total_pages,
            "backend": backend,
        })
        return started

    def run_local() -> tuple[dict, float]:
        global _quality_current
        with _waiting_lock:
            _quality_waiting.append(pdf_path)
            position = len(_quality_waiting)
        print(f"[quality] QUEUE  {name}  (local #{position})")
        notify("queued", {"position": position, "backend": "mineru-local"})
        _acquire_heavy_operation("quality", name)
        stop_hb.clear()
        try:
            with _waiting_lock:
                if pdf_path in _quality_waiting:
                    _quality_waiting.remove(pdf_path)
            _broadcast_queue_positions()
            _quality_current = name
            started = start("mineru-local")
            if total_pages > 1:
                threading.Thread(
                    target=heartbeat, args=(total_pages, started), daemon=True
                ).start()
            result = parse_quality(
                pdf_path,
                on_progress=on_progress,
                backend="mineru-local",
            )
            stop_hb.set()
            if total_pages:
                duration = time.time() - started
                _quality_rate[0] = 0.5 * _quality_rate[0] + 0.5 * max(
                    0.2, min(30.0, duration / total_pages)
                )
            return result, started
        finally:
            stop_hb.set()
            _quality_current = None
            _release_heavy_operation()

    def run_cloud() -> tuple[dict, float]:
        started = start("mineru-cloud")
        with _cloud_current_lock:
            _cloud_current.add(name)
        try:
            return parse_quality(
                pdf_path,
                on_progress=on_progress,
                backend="mineru-cloud",
                mineru_token=mineru_token,
            ), started
        finally:
            with _cloud_current_lock:
                _cloud_current.discard(name)

    try:
        from mineru_cloud import CloudAuthError, CloudError, CloudQuotaExhausted
        from quality_client import LocalQualityMemoryError
        from quality_router import (
            may_fallback_to_cloud,
            note_cloud_failure,
            note_token_rejected,
            route_quality,
        )

        initial = route_quality(pdf_path, backend_setting, mineru_token)
        if initial == "cloud":
            try:
                result, started = run_cloud()
            except CloudQuotaExhausted:
                result, started = run_local()
            except CloudAuthError as error:
                # Settings reads this off /health; the file still gets parsed.
                note_token_rejected()
                print(f"[quality] MinerU token rejected ({error}); using local")
                result, started = run_local()
            except CloudError as error:
                note_cloud_failure()
                print(f"[quality] cloud fallback for {name}: {error}")
                result, started = run_local()
        else:
            try:
                result, started = run_local()
            except LocalQualityMemoryError:
                if not may_fallback_to_cloud(
                    pdf_path, backend_setting, mineru_token
                ):
                    raise
                try:
                    result, started = run_cloud()
                except CloudAuthError:
                    # Local already failed, so there is nothing left to try:
                    # latch the token and let the local error stand.
                    note_token_rejected()
                    raise

        duration = time.time() - started
        prev = _progress.get(pdf_path, {})
        _progress[pdf_path] = {
            **prev,
            "done": True,
            "queued": False,
            "backend": result.get("backend"),
        }
        print(f"[quality] DONE   {name}  ({result.get('backend')}, {duration:.1f}s)")
        notify("quality", {"backend": result.get("backend")})
    except Exception as e:
        _progress[pdf_path] = {"done": False, "error": str(e), "queued": False, **meta}
        print(f"[quality] ERROR  {name}  {e}")
        notify("error", {"error": str(e)})
    finally:
        stop_hb.set()
        with _parse_inflight_lock:
            _parse_inflight.discard(pdf_path)


class ParseRequest(BaseModel):
    pdf_path: str
    subject_code: str
    relative_path: str = ""
    subject_id: int = 0
    ipc_port: int = 0
    backend: str | None = None
    mineru_token: str | None = None


@app.post("/parse-pdf")
def parse_pdf_endpoint(req: ParseRequest):
    if not req.pdf_path:
        raise HTTPException(status_code=400, detail="pdf_path required")
    req = req.model_copy(update={"pdf_path": str(Path(req.pdf_path).resolve())})
    with _parse_inflight_lock:
        if req.pdf_path in _parse_inflight:
            state = _progress.get(req.pdf_path, {"queued": True})
            return {"mode": "skip", "quality_status": _status_str(state)}
        _parse_inflight.add(req.pdf_path)
    try:
        result = _parse_pdf(req)
    except BaseException:
        with _parse_inflight_lock:
            _parse_inflight.discard(req.pdf_path)
        raise
    if result["quality_status"] != "queued":
        with _parse_inflight_lock:
            _parse_inflight.discard(req.pdf_path)
    return result


def _parse_pdf(req: ParseRequest):
    if not req.pdf_path:
        raise HTTPException(status_code=400, detail="pdf_path required")
    backend = req.backend or PARSE_SETTINGS.get().backend
    if backend not in BACKENDS:
        raise HTTPException(status_code=400, detail="invalid parse backend")
    name = Path(req.pdf_path).name
    meta = {
        "relative_path": req.relative_path,
        "subject_id": req.subject_id,
        "ipc_port": req.ipc_port,
    }

    existing = _md_state(req.pdf_path)

    if existing == "quality":
        # Already fully parsed — nothing to do.
        print(f"[skip]    {name}  (quality already done)")
        _notify_tauri(req.ipc_port, {
            "relative_path": req.relative_path,
            "subject_id": req.subject_id,
            "status": "quality",
        })
        return {"mode": "skip", "quality_status": "done"}

    if existing == "none":
        # No .md yet — run fast parse so user has something immediately.
        try:
            _notify_tauri(req.ipc_port, {
                "relative_path": req.relative_path,
                "subject_id": req.subject_id,
                "status": "parsing",
            })
            with _heavy_operation("fast", name):
                import fitz
                with fitz.open(req.pdf_path) as document:
                    MEMORY_GOVERNOR.admit("fast", document.page_count)
                t = time.time()
                print(f"[fast]    START  {name}")
                parse_fast_isolated(req.pdf_path)
                print(f"[fast]    DONE   {name}  ({time.time()-t:.1f}s)")
            _notify_tauri(req.ipc_port, {
                "relative_path": req.relative_path,
                "subject_id": req.subject_id,
                "status": "fast",
            })
        except FileNotFoundError:
            raise HTTPException(status_code=404, detail=f"PDF not found: {req.pdf_path}")
        except Exception as e:
            print(f"[fast]    ERROR  {name}  {e}")
            raise HTTPException(status_code=500, detail=str(e))
    else:
        # existing == "fast": .md exists but no quality yet — skip fast, go straight to quality.
        print(f"[skip]    {name}  (fast already done, queuing quality)")

    threading.Thread(
        target=_run_quality,
        args=(req.pdf_path, meta, backend, req.mineru_token),
        daemon=True,
    ).start()
    return {"mode": "fast" if existing == "none" else "skip", "quality_status": "queued"}


@app.get("/parse-progress")
async def parse_progress_sse(pdf_path: str):
    pdf_path = str(Path(pdf_path).resolve())
    async def generator():
        for _ in range(10):
            if pdf_path in _progress:
                break
            await asyncio.sleep(0.5)

        while True:
            state = _progress.get(pdf_path)
            if state is None:
                yield f"data: {json.dumps({'status': 'unknown'})}\n\n"
                await asyncio.sleep(0.5)
                continue
            yield f"data: {json.dumps(state)}\n\n"
            if state.get("done") or state.get("error"):
                break
            await asyncio.sleep(0.5)

    return StreamingResponse(
        generator(),
        media_type="text/event-stream",
        headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
    )


@app.get("/parse-status")
def parse_status(pdf_path: str):
    state = _progress.get(str(Path(pdf_path).resolve()))
    if state is None:
        return {"pdf_path": pdf_path, "quality_status": "unknown"}
    return {"pdf_path": pdf_path, "quality_status": _status_str(state)}


@app.get("/parse-status-batch")
def parse_status_batch(paths: str):
    """Comma-separated URL-encoded pdf paths."""
    result = {}
    for raw in paths.split(","):
        p = unquote(raw.strip())
        if not p:
            continue
        state = _progress.get(str(Path(p).resolve()))
        result[p] = _status_str(state) if state else "unknown"
    return result


# ── Embeddings ───────────────────────────────────────────────────────────────
#
# Retrieval indexes the rendered page image, not scraped text — see embedder.py
# for why. These endpoints are synchronous: a 37-page deck takes ~20s and the
# caller is a Rust command already running off the UI thread.


class EmbedRequest(BaseModel):
    pdf_path: str
    force: bool = False
    # Set by the app so per-page embed progress can flow back through the same
    # IPC channel the parse statuses use. The CLI leaves them at their defaults.
    relative_path: str = ""
    ipc_port: int = 0


class QueryRequest(BaseModel):
    text: str


@app.post("/embed-pdf")
def embed_pdf_endpoint(req: EmbedRequest):
    import embedding_client as embedder

    if not req.pdf_path:
        raise HTTPException(status_code=400, detail="pdf_path required")
    if not Path(req.pdf_path).exists():
        raise HTTPException(status_code=404, detail=f"PDF not found: {req.pdf_path}")

    name = Path(req.pdf_path).name

    def notify(status: str, extra: dict | None = None):
        _notify_tauri(req.ipc_port, {
            "relative_path": req.relative_path,
            "subject_id": 0,
            "status": status,
            **(extra or {}),
        })

    if not req.force and embedder.is_embedded(req.pdf_path):
        print(f"[embed]   SKIP   {name}  (already embedded)")
        notify("embedded")
        return {
            "status": "skip",
            "embeddings_path": str(embedder.embeddings_path(req.pdf_path)),
        }

    def on_progress(state: dict):
        if state["pages_done"] % 10 == 0 or state["pages_done"] == state["total_pages"]:
            print(f"  [embed] {name}  {state['pages_done']}/{state['total_pages']}")
        notify("embedding", {"pages_done": state["pages_done"], "total_pages": state["total_pages"]})

    try:
        notify("embedding")
        with _heavy_operation("embed", name):
            print(f"[embed]   START  {name}")
            result = embedder.embed_pdf(req.pdf_path, on_progress=on_progress)
        notify("embedded")
        return {"status": "ok", **result}
    except Exception as e:
        print(f"[embed]   ERROR  {name}  {e}")
        notify("embed_error", {"error": str(e)})
        raise HTTPException(status_code=500, detail=str(e))


@app.post("/embed-query")
def embed_query_endpoint(req: QueryRequest):
    """Embed a search query. Carries the retrieval instruction; pages do not."""
    import embedding_client as embedder

    if not req.text.strip():
        raise HTTPException(status_code=400, detail="text required")
    try:
        # Even a previously warm worker can be evicted by the governor before
        # this call reaches it. Join the gate so an unnoticed reload cannot
        # overlap a local parse. Cloud work never holds this gate.
        with _heavy_operation("embed-query", "search query"):
            result = embedder.embed_query(req.text)
        return {
            "vector": result["vector"],
            "dim": embedder.EMBED_DIM,
            "dtype": "float16",
            "model": embedder.MODEL_REPO,
        }
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@app.get("/embed-info")
def embed_info():
    """Config the store must agree with — vectors from a different model or
    truncation are not comparable."""
    import embedding_client as embedder

    return {
        "model": embedder.MODEL_REPO,
        "dim": embedder.EMBED_DIM,
        "dtype": "float16",
        "max_tokens": embedder.EMBED_MAX_TOKENS,
        "loaded": embedder.is_loaded(),
    }


class LimitsRequest(BaseModel):
    memory_cap_mb: int | None = None
    backend: str | None = None


@app.post("/limits")
def update_limits(req: LimitsRequest):
    """Apply a memory cap live without interrupting an in-flight HTTP server."""
    if req.memory_cap_mb is not None and req.memory_cap_mb < MIN_CAP_MB:
        raise HTTPException(
            status_code=400,
            detail=f"memory_cap_mb must be at least {MIN_CAP_MB}",
        )
    if req.backend is not None and req.backend not in BACKENDS:
        raise HTTPException(status_code=400, detail="invalid parse backend")
    MEMORY_GOVERNOR.start()
    if req.memory_cap_mb is not None:
        MEMORY_GOVERNOR.update_cap(req.memory_cap_mb)
    try:
        settings = PARSE_SETTINGS.update(
            memory_cap_mb=req.memory_cap_mb,
            backend=req.backend,
        )
    except ValueError as error:
        raise HTTPException(status_code=400, detail=str(error)) from None
    return {
        "memory_cap_mb": settings.memory_cap_mb,
        "backend": settings.backend,
    }


@app.post("/mineru-token-reset")
def mineru_token_reset():
    """The stored MinerU token changed, so a past rejection no longer applies.

    Rust calls this after writing the keychain; without it a fresh token would
    stay locked out of cloud parsing until the sidecar restarted."""
    from quality_router import clear_token_rejected

    clear_token_rejected()
    return {"ok": True}


@app.get("/health")
def health():
    # The pid is here so "is the thing on this port the one I just started?"
    # has an answer. A sidecar that outlives its app keeps serving stale code
    # and looks perfectly healthy while doing it.
    with _heavy_condition:
        heavy_current = _heavy_current
        heavy_waiting = len(_heavy_queue)
    settings = PARSE_SETTINGS.get()
    try:
        from mineru_cloud import usage_status
        cloud_usage = usage_status()
    except Exception:
        cloud_usage = None
    from quality_router import token_rejected
    with _cloud_current_lock:
        cloud_current = sorted(_cloud_current)
    return {
        "status": "ok",
        "pid": os.getpid(),
        "parser_version": PARSER_VERSION,
        "quality_current": _quality_current,
        "quality_queued": len(_quality_waiting),
        "cloud_current": cloud_current,
        "heavy_current": heavy_current,
        "heavy_waiting": heavy_waiting,
        "memory": MEMORY_GOVERNOR.health(),
        "parse": {
            "backend": settings.backend,
            "cloud_usage": cloud_usage,
            "cloud_token_rejected": token_rejected(),
        },
        "workers": {
            "quality": {
                "pid": QUALITY_WORKER.pid,
                "active": QUALITY_WORKER.active,
            },
            "embed": {
                "pid": EMBED_WORKER.pid,
                "active": EMBED_WORKER.active,
            },
        },
    }


if __name__ == "__main__":
    # Do not eagerly load the ~4 GB embedding model. Keeping it absent until an
    # embed is requested leaves maximum headroom for a quality parse that is
    # first through the global heavy-work queue.
    MEMORY_GOVERNOR.start()
    atexit.register(shutdown_workers)
    uvicorn.run(app, host="127.0.0.1", port=9547)
