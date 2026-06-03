import asyncio
import json
import threading
import time
import urllib.request
import uvicorn
from pathlib import Path
from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import StreamingResponse
from pydantic import BaseModel
from urllib.parse import unquote

from parser import parse_fast, parse_quality


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

_quality_lock = threading.Semaphore(1)

# pdf_path -> progress dict
_progress: dict[str, dict] = {}


def _status_str(state: dict) -> str:
    if state.get("done"):
        return "done"
    if state.get("error"):
        return f"error: {state['error']}"
    if state.get("queued"):
        return "queued"
    return "running"


def _run_quality(pdf_path: str, meta: dict) -> None:
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
    print(f"[quality] QUEUE  {name}")
    notify("queued")
    _quality_lock.acquire()

    def on_progress(state: dict):
        _progress[pdf_path] = {**state, **meta}
        print(f"  [quality] {name}  chunk {state['chunk']}/{state['total_chunks']}  pages {state['pages_done']}/{state['total_pages']}")
        notify("running", {"pages_done": state["pages_done"], "total_pages": state["total_pages"]})

    try:
        _progress[pdf_path] = {"chunk": 0, "total_chunks": 0, "pages_done": 0, "total_pages": 0, "done": False, **meta}
        t = time.time()
        print(f"[quality] START  {name}")
        notify("running")
        parse_quality(pdf_path, on_progress=on_progress)
        prev = _progress.get(pdf_path, {})
        _progress[pdf_path] = {**prev, "done": True, "queued": False}
        print(f"[quality] DONE   {name}  ({time.time()-t:.1f}s)")
        notify("quality")
    except Exception as e:
        _progress[pdf_path] = {"done": False, "error": str(e), "queued": False, **meta}
        print(f"[quality] ERROR  {name}  {e}")
        notify("error", {"error": str(e)})
    finally:
        _quality_lock.release()


class ParseRequest(BaseModel):
    pdf_path: str
    subject_code: str
    relative_path: str = ""
    subject_id: int = 0
    ipc_port: int = 0


def _md_state(pdf_path: str) -> str:
    """Return 'quality' | 'fast' | 'none' based on disk siblings."""
    p = Path(pdf_path)
    md = p.with_suffix(".md")
    if not md.exists():
        return "none"
    images_dir = p.parent / f"{p.stem}_images"
    return "quality" if images_dir.is_dir() else "fast"


@app.post("/parse-pdf")
def parse_pdf_endpoint(req: ParseRequest):
    if not req.pdf_path:
        raise HTTPException(status_code=400, detail="pdf_path required")
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
            t = time.time()
            print(f"[fast]    START  {name}")
            parse_fast(req.pdf_path)
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

    threading.Thread(target=_run_quality, args=(req.pdf_path, meta), daemon=True).start()
    return {"mode": "fast" if existing == "none" else "skip", "quality_status": "queued"}


@app.get("/parse-progress")
async def parse_progress_sse(pdf_path: str):
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
    state = _progress.get(pdf_path)
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
        state = _progress.get(p)
        result[p] = _status_str(state) if state else "unknown"
    return result


@app.get("/health")
def health():
    return {"status": "ok"}


if __name__ == "__main__":
    uvicorn.run(app, host="127.0.0.1", port=9547)
