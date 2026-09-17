"""Page-image embeddings for retrieval.

The retriever indexes what a page *looks like*, not what text we could scrape
off it. Measured on MULT20015 decks: on ordinary questions image and text
embeddings tie, but on formula slides, QUI screenshots and diagrams — where
text extraction returns garbage like `56 = 7(( 7() 7)(` — image retrieval
roughly doubles recall and text never recovers even at rank 3.

So: embed the rendered PNG, hydrate answers from the per-page markdown that
parser.py writes. The two meet on (file, page_no).
"""

import base64
import importlib.util
import json
import sys
import threading
import time
from pathlib import Path

import fitz
import numpy as np
import torch
from PIL import Image

from embed_contract import (
    EMBED_DIM,
    EMBED_MAX_TOKENS,
    MODEL_REPO,
    QUERY_INSTRUCTION,
    embeddings_path,
    is_embedded,
)
from modellock import MODEL_INIT_LOCK

# Pages are rendered above the model's own pixel cap and let the processor
# downscale, so RENDER_DPI only needs to be high enough not to be the
# bottleneck. EMBED_MAX_TOKENS is the real speed/accuracy knob — batching does
# nothing on MPS (identical s/page at batch 1 through 8), token count is all
# that moves the needle.
#
# Swept against the retrieval eval; accuracy is flat until it falls off a cliff:
#   1800 tok  1.47 s/page   visual R@1 6/8   <- the model default
#   1280 tok  1.17 s/page   visual R@1 7/8
#    900 tok  0.79 s/page   visual R@1 7/8
#    640 tok  0.54 s/page   visual R@1 7/8   <- knee
#    448 tok  0.36 s/page   visual R@1 6/8
#    256 tok  0.23 s/page   visual R@1 5/8
# 640 is 2.7x faster than the default and no worse. Semantic queries were 15/15
# at every setting — only figure-heavy pages care about resolution.
RENDER_DPI = 200
_PIXELS_PER_TOKEN = 32 * 32  # patch 16 with 2x2 spatial merge

# A ceiling on the rendered pixmap, because a page is not always page-sized. A
# spreadsheet exported with SinglePageSheets is one page however many rows it
# has: a 300-row marks sheet lands at 3418x3853pt, which at 200 DPI is 101
# megapixels — 305 MB of RGB for one page, in a process tree with a hard
# memory cap. The cap costs nothing real: the processor downscales to
# EMBED_MAX_TOKENS (640 tokens ~ 0.66 MP) anyway, so 8 MP is still an order of
# magnitude more detail than the model can see. A4 at 200 DPI is 3.9 MP and so
# is never clamped — no existing vector changes.
MAX_RENDER_PIXELS = 8_000_000

# Asymmetric retrieval: documents are embedded plainly, queries carry a task
# instruction. Changing either invalidates every stored vector.
_embedder = None
_load_lock = threading.Lock()

# pdf_path -> progress dict
_progress: dict[str, dict] = {}


def _load_model():
    """Load the reference implementation from the model snapshot.

    Qwen ships the embedding wrapper as a loose script rather than wiring it
    into config.json's auto_map, so there is no trust_remote_code path to it —
    we import the file. Its __init__ also hardcodes `cuda if available else
    cpu`, which on a Mac silently lands on CPU, so we subclass to place it.
    """
    from huggingface_hub import snapshot_download

    path = snapshot_download(MODEL_REPO)
    script = Path(path) / "scripts" / "qwen3_vl_embedding.py"
    spec = importlib.util.spec_from_file_location("qwen3_vl_embedding", script)
    mod = importlib.util.module_from_spec(spec)
    sys.modules["qwen3_vl_embedding"] = mod
    spec.loader.exec_module(mod)

    if torch.backends.mps.is_available():
        device = "mps"
    elif torch.cuda.is_available():
        device = "cuda"
    else:
        device = "cpu"

    class _Embedder(mod.Qwen3VLEmbedder):
        def __init__(self, model_path: str):
            self.max_length = mod.MAX_LENGTH
            self.min_pixels = mod.MIN_PIXELS
            self.max_pixels = EMBED_MAX_TOKENS * _PIXELS_PER_TOKEN
            self.total_pixels = mod.MAX_TOTAL_PIXELS
            self.fps = mod.FPS
            self.num_frames = mod.MAX_FRAMES
            self.max_frames = mod.MAX_FRAMES
            self.default_instruction = "Represent the user's input."
            self.model = mod.Qwen3VLForEmbedding.from_pretrained(
                model_path, dtype=torch.bfloat16
            ).to(device)
            self.processor = mod.Qwen3VLProcessor.from_pretrained(
                model_path, padding_side="right"
            )
            self.model.eval()

    print(f"[embed] loading {MODEL_REPO} on {device}…", flush=True)
    t = time.time()
    emb = _Embedder(path)
    print(f"[embed] ready ({time.time() - t:.1f}s)", flush=True)
    return emb


def get_embedder():
    """Load once, on whichever thread asks first.

    Takes the process-wide model-init lock as well as its own: MinerU loads
    its weights on another thread and two concurrent `from_pretrained` calls
    fail with `Cannot copy out of meta tensor`. See modellock.py.
    """
    global _embedder
    if _embedder is None:
        with _load_lock:
            if _embedder is None:
                with MODEL_INIT_LOCK:
                    _embedder = _load_model()
    return _embedder


def _to_stored(vec: torch.Tensor) -> np.ndarray:
    """Truncate to EMBED_DIM, re-normalise, return float16.

    Stored unit-length so cosine similarity is a plain dot product downstream.
    """
    v = vec.float()[..., :EMBED_DIM]
    v = torch.nn.functional.normalize(v, p=2, dim=-1)
    return v.cpu().numpy().astype(np.float16)


def _b64(arr: np.ndarray) -> str:
    return base64.b64encode(arr.tobytes()).decode("ascii")


def _page_zoom(page, zoom: float) -> tuple[float, float]:
    """`zoom`, lowered if this page would render past MAX_RENDER_PIXELS."""
    rect = page.rect
    area = max(1.0, rect.width * rect.height)
    capped = (MAX_RENDER_PIXELS / area) ** 0.5
    z = min(zoom, capped)
    return z, z


def render_pages(pdf_path: str, dpi: int = RENDER_DPI):
    doc = fitz.open(pdf_path)
    zoom = dpi / 72
    for i in range(doc.page_count):
        page = doc[i]
        pix = page.get_pixmap(matrix=fitz.Matrix(*_page_zoom(page, zoom)))
        yield i + 1, Image.frombytes("RGB", (pix.width, pix.height), pix.samples)


def embed_query(text: str) -> np.ndarray:
    emb = get_embedder()
    vec = emb.process([{"text": text, "instruction": QUERY_INSTRUCTION}])
    return _to_stored(vec)[0]


def embed_pdf(pdf_path: str, on_progress=None) -> dict:
    """Embed every page image and write the {stem}.emb.json sidecar."""
    path = Path(pdf_path)
    if not path.exists():
        raise FileNotFoundError(pdf_path)

    emb = get_embedder()
    doc = fitz.open(str(path))
    total = doc.page_count
    doc.close()

    pages = []
    t0 = time.time()
    for page_no, img in render_pages(str(path)):
        vec = _to_stored(emb.process([{"image": img}]))[0]
        pages.append({"page_no": page_no, "vector": _b64(vec)})
        if on_progress:
            on_progress({
                "pages_done": page_no,
                "total_pages": total,
                "done": False,
            })

    payload = {
        "pdf": path.name,
        "model": MODEL_REPO,
        "dim": EMBED_DIM,
        "dtype": "float16",
        "instruction": QUERY_INSTRUCTION,
        "page_count": len(pages),
        "pages": pages,
    }
    out = embeddings_path(path)
    out.write_text(json.dumps(payload), encoding="utf-8")

    elapsed = time.time() - t0
    print(f"[embed] {path.name}: {len(pages)} pages in {elapsed:.1f}s "
          f"({elapsed / max(1, len(pages)):.2f}s/page)", flush=True)

    return {
        "embeddings_path": str(out),
        "page_count": len(pages),
        "dim": EMBED_DIM,
        "model": MODEL_REPO,
        "seconds": round(elapsed, 1),
    }
