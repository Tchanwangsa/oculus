"""Shared model-worker instances and high-level request helpers."""

from __future__ import annotations

from memory_governor import MemoryGovernor
from worker_client import WorkerDied, WorkerProcess


QUALITY_WORKER = WorkerProcess("quality_worker.py", "quality-worker")
EMBED_WORKER = WorkerProcess("embed_worker.py", "embed-worker")
MEMORY_GOVERNOR = MemoryGovernor({
    "quality": QUALITY_WORKER,
    "embed": EMBED_WORKER,
})


def shutdown_workers() -> None:
    MEMORY_GOVERNOR.stop()
    QUALITY_WORKER.shutdown()
    EMBED_WORKER.shutdown()


__all__ = [
    "EMBED_WORKER",
    "MEMORY_GOVERNOR",
    "QUALITY_WORKER",
    "WorkerDied",
    "shutdown_workers",
]
