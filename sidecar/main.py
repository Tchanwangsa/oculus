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
        started = _quality_started.get(pdf_path, time.time())
        done, total = state["pages_done"], state["total_pages"]
        rate = (time.time() - started) / max(1, done)
        eta = rate * (total - done)
        print(f"  [quality] {name}  chunk {state['chunk']}/{state['total_chunks']}  "
              f"pages {done}/{total}  {time.time() - started:.0f}s elapsed, "
              f"~{eta:.0f}s left ({rate:.1f}s/page)")
        notify("running", {"pages_done": state["pages_done"], "total_pages": state["total_pages"]})

    try:
        _progress[pdf_path] = {"chunk": 0, "total_chunks": 0, "pages_done": 0, "total_pages": 0, "done": False, **meta}
        t = time.time()
        _quality_started[pdf_path] = t
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


# ── Embeddings ───────────────────────────────────────────────────────────────
#
# Retrieval indexes the rendered page image, not scraped text — see embedder.py
# for why. These endpoints are synchronous: a 37-page deck takes ~20s and the
# caller is a Rust command already running off the UI thread.


class EmbedRequest(BaseModel):
    pdf_path: str
    force: bool = False


class QueryRequest(BaseModel):
    text: str


@app.post("/embed-pdf")
def embed_pdf_endpoint(req: EmbedRequest):
    import embedder

    if not req.pdf_path:
        raise HTTPException(status_code=400, detail="pdf_path required")
    if not Path(req.pdf_path).exists():
        raise HTTPException(status_code=404, detail=f"PDF not found: {req.pdf_path}")

    name = Path(req.pdf_path).name
    if not req.force and embedder.is_embedded(req.pdf_path):
        print(f"[embed]   SKIP   {name}  (already embedded)")
        return {
            "status": "skip",
            "embeddings_path": str(embedder.embeddings_path(req.pdf_path)),
        }

    def on_progress(state: dict):
        if state["pages_done"] % 10 == 0 or state["pages_done"] == state["total_pages"]:
            print(f"  [embed] {name}  {state['pages_done']}/{state['total_pages']}")

    try:
        print(f"[embed]   START  {name}")
        result = embedder.embed_pdf(req.pdf_path, on_progress=on_progress)
        return {"status": "ok", **result}
    except Exception as e:
        print(f"[embed]   ERROR  {name}  {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.post("/embed-query")
def embed_query_endpoint(req: QueryRequest):
    """Embed a search query. Carries the retrieval instruction; pages do not."""
    import base64

    import embedder

    if not req.text.strip():
        raise HTTPException(status_code=400, detail="text required")
    try:
        vec = embedder.embed_query(req.text)
        return {
            "vector": base64.b64encode(vec.tobytes()).decode("ascii"),
            "dim": int(vec.shape[0]),
            "dtype": "float16",
            "model": embedder.MODEL_REPO,
        }
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@app.get("/embed-info")
def embed_info():
    """Config the store must agree with — vectors from a different model or
    truncation are not comparable."""
    import embedder

    return {
        "model": embedder.MODEL_REPO,
        "dim": embedder.EMBED_DIM,
        "dtype": "float16",
        "max_tokens": embedder.EMBED_MAX_TOKENS,
        "loaded": embedder._embedder is not None,
    }


def _warm_embedder():
    """Load the embedding model before any parse can start.

    The lock in modellock.py makes a concurrent load safe, but getting the
    embedder in early means the two never contend at all: by the time the first
    scraped PDF arrives, docling is the only thing still loading. Costs ~2s on a
    background thread and makes the first search instant instead of 6s.
    """
    def warm():
        try:
            import embedder
            embedder.get_embedder()
        except Exception as e:
            # Non-fatal: embedding retries lazily on first use.
            print(f"[embed] warmup failed: {e}", flush=True)

    threading.Thread(target=warm, daemon=True).start()


@app.get("/health")
def health():
    return {"status": "ok"}


# Must stay inside the __main__ guard, not at module scope. MinerU renders PDF
# pages in a ProcessPoolExecutor using the "spawn" start method, and spawn
# re-imports the parent's entry module (this file, as __mp_main__) in every
# worker. At module scope the warmup would therefore load a 4GB Qwen3-VL into
# each render worker. Under the guard, __name__ is "__mp_main__" there and it
# doesn't fire.
if __name__ == "__main__":
    # Started here rather than via an on_event hook: on_event is deprecated, and
    # there is nothing to wait for — the thread just needs to be running before
    # the first parse arrives.
    _warm_embedder()
    uvicorn.run(app, host="127.0.0.1", port=9547)
