"""Parent-side local quality parsing with memory-aware kill-and-retry."""

from __future__ import annotations

import shutil
import tempfile
from pathlib import Path

import fitz

from memory_governor import MemoryAdmissionError
from mineru_local import CHUNK_PAGES, MFR_BATCH, PROCESSING_WINDOW_PAGES
from model_workers import MEMORY_GOVERNOR, QUALITY_WORKER
from worker_client import WorkerDied


# Formula batch first, then page batch: MinerU's memory peaks are driven by
# how many formulas decode together, not by how many pages are resident.
LOCAL_RETRY_LADDER = (
    {
        "chunk_pages": CHUNK_PAGES,
        "window_pages": PROCESSING_WINDOW_PAGES,
        "mfr_batch": MFR_BATCH,
    },
    {
        "chunk_pages": min(16, CHUNK_PAGES),
        "window_pages": min(4, PROCESSING_WINDOW_PAGES),
        "mfr_batch": 1,
    },
)


class LocalQualityMemoryError(RuntimeError):
    pass


def parse_local(
    pdf_path: str,
    images_dir: Path,
    images_rel: str,
    on_progress=None,
) -> tuple[list[dict], int]:
    with fitz.open(pdf_path) as document:
        total_pages = document.page_count

    failures: list[str] = []
    for attempt, limits in enumerate(LOCAL_RETRY_LADDER, start=1):
        # Each attempt gets a clean staging directory. A killed first attempt
        # must not strand crops that the successful retry never references.
        shutil.rmtree(images_dir, ignore_errors=True)
        try:
            MEMORY_GOVERNOR.admit(
                "quality",
                total_pages,
                worker=QUALITY_WORKER,
                window_pages=limits["window_pages"],
            )
            # The parent owns extraction scratch space so SIGKILL cannot
            # strand a worker's intermediate PDFs/crops on disk.
            with tempfile.TemporaryDirectory(prefix="oculus-mineru-worker-") as workspace:
                result = QUALITY_WORKER.request(
                    "parse",
                    {
                        "pdf_path": pdf_path,
                        "images_dir": str(images_dir),
                        "images_rel": images_rel,
                        "workspace": workspace,
                        **limits,
                    },
                    on_progress=on_progress,
                )
            return result["pages"], int(result["image_count"])
        except (MemoryAdmissionError, WorkerDied) as error:
            failures.append(f"attempt {attempt}: {error}")
            continue

    raise LocalQualityMemoryError("; ".join(failures))
