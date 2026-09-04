"""Lightweight parent-side facade for the Qwen embedding worker."""

from __future__ import annotations

import fitz

from embed_contract import (
    EMBED_DIM,
    EMBED_MAX_TOKENS,
    MODEL_REPO,
    QUERY_INSTRUCTION,
    embeddings_path,
    is_embedded,
)
from model_workers import EMBED_WORKER, MEMORY_GOVERNOR


_loaded_generation = -1


def is_loaded() -> bool:
    return EMBED_WORKER.is_alive and EMBED_WORKER.generation == _loaded_generation


def embed_pdf(pdf_path: str, on_progress=None) -> dict:
    global _loaded_generation
    with fitz.open(pdf_path) as document:
        pages = document.page_count
    MEMORY_GOVERNOR.admit("embed", pages, worker=EMBED_WORKER)
    result = EMBED_WORKER.request(
        "embed_pdf",
        {"pdf_path": pdf_path},
        on_progress=on_progress,
    )
    _loaded_generation = EMBED_WORKER.generation
    return result


def embed_query(text: str) -> dict:
    global _loaded_generation
    MEMORY_GOVERNOR.admit("embed", 1, worker=EMBED_WORKER)
    result = EMBED_WORKER.request("embed_query", {"text": text})
    _loaded_generation = EMBED_WORKER.generation
    return result


__all__ = [
    "EMBED_DIM",
    "EMBED_MAX_TOKENS",
    "MODEL_REPO",
    "QUERY_INSTRUCTION",
    "embed_pdf",
    "embed_query",
    "embeddings_path",
    "is_embedded",
    "is_loaded",
]
