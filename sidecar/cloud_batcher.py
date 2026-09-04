"""Short-window cloud accumulation queue, independent of local heavy work."""

from __future__ import annotations

import threading
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from pathlib import Path

from mineru_cloud import CloudDocument, MinerUCloudClient


ACCUMULATION_SECONDS = 5.0
ACCUMULATION_FILES = 20
CLOUD_BATCH_CONCURRENCY = 8


@dataclass
class _Queued:
    token: str
    document: CloudDocument
    done: threading.Event = field(default_factory=threading.Event)
    result: tuple[list[dict], int] | None = None
    error: BaseException | None = None


class CloudBatcher:
    def __init__(self):
        self._condition = threading.Condition()
        self._waiting: list[_Queued] = []
        self._started = False
        self._executor = ThreadPoolExecutor(
            max_workers=CLOUD_BATCH_CONCURRENCY,
            thread_name_prefix="mineru-cloud",
        )

    def submit(
        self,
        token: str,
        pdf_path: str,
        images_dir: Path,
        images_rel: str,
        on_progress=None,
    ) -> tuple[list[dict], int]:
        queued = _Queued(
            token=token,
            document=CloudDocument(
                pdf_path=pdf_path,
                images_dir=images_dir,
                images_rel=images_rel,
                on_progress=on_progress,
            ),
        )
        with self._condition:
            self._waiting.append(queued)
            if not self._started:
                self._started = True
                threading.Thread(
                    target=self._dispatch,
                    daemon=True,
                    name="mineru-cloud-batcher",
                ).start()
            self._condition.notify_all()
        queued.done.wait()
        if queued.error:
            raise queued.error
        assert queued.result is not None
        return queued.result

    def _dispatch(self) -> None:
        while True:
            with self._condition:
                while not self._waiting:
                    self._condition.wait()
                first_at = time.monotonic()
                token = self._waiting[0].token
                while True:
                    compatible = [job for job in self._waiting if job.token == token]
                    remaining = ACCUMULATION_SECONDS - (time.monotonic() - first_at)
                    if len(compatible) >= ACCUMULATION_FILES or remaining <= 0:
                        break
                    self._condition.wait(timeout=remaining)
                batch = [
                    job for job in self._waiting
                    if job.token == token
                ][:ACCUMULATION_FILES]
                for job in batch:
                    self._waiting.remove(job)
            self._executor.submit(self._run, batch)

    @staticmethod
    def _run(batch: list[_Queued]) -> None:
        try:
            client = MinerUCloudClient(batch[0].token)
            results = client.extract_documents([job.document for job in batch])
            for job in batch:
                job.result = results[job.document.pdf_path]
        except BaseException as error:
            for job in batch:
                job.error = error
        finally:
            for job in batch:
                job.done.set()


CLOUD_BATCHER = CloudBatcher()
