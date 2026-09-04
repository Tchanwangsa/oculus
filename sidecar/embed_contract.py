"""Lightweight embedding metadata and completed-output checks.

This module deliberately imports no torch/model code. The HTTP parent uses it
without materialising the Qwen runtime that belongs in ``embed_worker.py``.
"""

import json
from pathlib import Path


MODEL_REPO = "Qwen/Qwen3-VL-Embedding-2B"
EMBED_DIM = 512
EMBED_MAX_TOKENS = 640
QUERY_INSTRUCTION = (
    "Given a student's question, retrieve the lecture slide that answers it."
)


def embeddings_path(pdf_path) -> Path:
    path = Path(pdf_path)
    return path.parent / f"{path.stem}.emb.json"


def is_embedded(pdf_path: str) -> bool:
    path = embeddings_path(pdf_path)
    if not path.exists():
        return False
    try:
        metadata = json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return False
    return (
        metadata.get("model") == MODEL_REPO
        and metadata.get("dim") == EMBED_DIM
        and metadata.get("instruction") == QUERY_INSTRUCTION
    )
