"""MinerU cloud batch-upload client and persistent quota accounting."""

from __future__ import annotations

import http.client
import json
import os
import shutil
import tempfile
import threading
import time
import urllib.error
import urllib.request
import uuid
import zipfile
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Callable
from urllib.parse import urlsplit

import fitz

from mineru_render import render


API_ROOT = "https://mineru.net/api/v4"

# One source of truth. File/page/batch and priority limits are from the live
# docs (2026-09-03). Per-minute/file-day budgets remain conservative Oculus
# policy where that page does not publish an account limit. Server errors win.
LIMITS = {
    "max_file_bytes": 200 * 1024 * 1024,
    "max_pages_per_task": 200,
    "max_files_per_batch": 50,
    "submit_requests_per_minute": 50,
    "poll_requests_per_minute": 1000,
    "daily_files": 5_000,
    "daily_html_files": 100,
    "daily_priority_pages": 1_000,
}


class CloudError(RuntimeError):
    pass


class CloudQuotaExhausted(CloudError):
    pass


class CloudAuthError(CloudError):
    """MinerU rejected the token itself, not the document.

    A0202 is a token it never accepted; A0211 is one that has since expired.
    Either way the fix is a new token, so callers latch this instead of
    retrying every remaining file into the same 401.
    """

    def __init__(self, message: str, code: str | None = None, expired: bool = False):
        super().__init__(message)
        self.code = code
        self.expired = expired


def _auth_error(body: bytes) -> CloudAuthError:
    code = None
    try:
        payload = json.loads(body.decode("utf-8"))
        if isinstance(payload, dict):
            raw = payload.get("msgCode") or payload.get("code")
            if isinstance(raw, str):
                code = raw
    except (ValueError, UnicodeDecodeError):
        pass
    expired = code == "A0211"
    return CloudAuthError(
        "MinerU token has expired" if expired else "MinerU rejected the token",
        code=code,
        expired=expired,
    )


class TokenBucket:
    def __init__(self, per_minute: int):
        self.capacity = float(per_minute)
        self.tokens = float(per_minute)
        self.rate = float(per_minute) / 60.0
        self.updated = time.monotonic()
        self.lock = threading.Condition()

    def acquire(self) -> None:
        with self.lock:
            while True:
                now = time.monotonic()
                self.tokens = min(
                    self.capacity,
                    self.tokens + (now - self.updated) * self.rate,
                )
                self.updated = now
                if self.tokens >= 1:
                    self.tokens -= 1
                    return
                self.lock.wait((1 - self.tokens) / self.rate)


SUBMIT_BUCKET = TokenBucket(LIMITS["submit_requests_per_minute"])
POLL_BUCKET = TokenBucket(LIMITS["poll_requests_per_minute"])


def _beijing_day() -> str:
    return datetime.now(timezone(timedelta(hours=8))).date().isoformat()


class UsageLedger:
    """Crash-safe daily counters, conservatively reset on Beijing midnight."""

    def __init__(self, path: Path | None = None):
        data_dir = Path(os.environ.get("OCULUS_DATA_DIR", Path(__file__).parent / "data"))
        self.path = path or data_dir / "mineru-usage.json"
        self.lock = threading.Lock()

    @staticmethod
    def _empty() -> dict:
        return {
            "date": _beijing_day(),
            "files": 0,
            "html_files": 0,
            "pages": 0,
            "quota_exhausted": False,
        }

    def _read_unlocked(self) -> dict:
        try:
            value = json.loads(self.path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            value = self._empty()
        if value.get("date") != _beijing_day():
            value = self._empty()
        return {**self._empty(), **value}

    def _write_unlocked(self, value: dict) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        temporary = self.path.with_suffix(f".tmp-{os.getpid()}-{threading.get_ident()}")
        temporary.write_text(json.dumps(value, indent=2), encoding="utf-8")
        temporary.replace(self.path)

    def snapshot(self) -> dict:
        with self.lock:
            return self._read_unlocked()

    def ensure_available(self, files: int) -> None:
        value = self.snapshot()
        if value["quota_exhausted"] or value["files"] + files > LIMITS["daily_files"]:
            raise CloudQuotaExhausted("MinerU daily file quota is exhausted")

    def record(self, files: int, pages: int) -> None:
        with self.lock:
            value = self._read_unlocked()
            if value["quota_exhausted"] or value["files"] + files > LIMITS["daily_files"]:
                raise CloudQuotaExhausted("MinerU daily file quota is exhausted")
            value["files"] += files
            value["pages"] += pages
            self._write_unlocked(value)

    def latch_exhausted(self) -> None:
        with self.lock:
            value = self._read_unlocked()
            value["quota_exhausted"] = True
            self._write_unlocked(value)


USAGE_LEDGER = UsageLedger()


@dataclass
class CloudDocument:
    pdf_path: str
    images_dir: Path
    images_rel: str
    on_progress: Callable[[dict], None] | None = None
    total_pages: int = 0
    content: list[dict] = field(default_factory=list)
    source_images: Path | None = None
    task_progress: dict[str, int] = field(default_factory=dict)


@dataclass
class _Task:
    document: CloudDocument
    data_id: str
    upload_name: str
    source_path: Path
    page_offset: int
    page_count: int
    page_ranges: str | None

    def api_entry(self) -> dict:
        entry = {"name": self.upload_name, "data_id": self.data_id, "is_ocr": False}
        if self.page_ranges:
            entry["page_ranges"] = self.page_ranges
        return entry


def cloud_quota_available() -> bool:
    value = USAGE_LEDGER.snapshot()
    return not value["quota_exhausted"] and value["files"] < LIMITS["daily_files"]


def usage_status() -> dict:
    value = USAGE_LEDGER.snapshot()
    return {
        **value,
        "daily_file_limit": LIMITS["daily_files"],
        "daily_priority_page_limit": LIMITS["daily_priority_pages"],
        "priority_degraded": value["pages"] >= LIMITS["daily_priority_pages"],
    }


class MinerUCloudClient:
    def __init__(self, token: str):
        token = token.strip()
        if not token:
            raise CloudError("MinerU token is missing")
        self._token = token

    def _api_json(
        self,
        method: str,
        path: str,
        body: dict | None = None,
        *,
        bucket: TokenBucket,
        attempts: int = 4,
    ) -> dict:
        delay = 1.0
        attempt = 0
        while attempt < attempts:
            bucket.acquire()
            data = json.dumps(body).encode("utf-8") if body is not None else None
            request = urllib.request.Request(
                f"{API_ROOT}{path}",
                data=data,
                method=method,
                headers={
                    "Authorization": f"Bearer {self._token}",
                    "Accept": "application/json",
                    **({"Content-Type": "application/json"} if data is not None else {}),
                },
            )
            try:
                with urllib.request.urlopen(request, timeout=30) as response:
                    payload = json.loads(response.read().decode("utf-8"))
            except urllib.error.HTTPError as error:
                if error.code == 429:
                    # Per-minute pressure is a wait, never a local fallback.
                    try:
                        wait = float(error.headers.get("Retry-After", "60"))
                    except (TypeError, ValueError):
                        wait = 60
                    time.sleep(max(1, min(wait, 60)))
                    continue
                if error.code in (401, 403):
                    # A bad token cannot be retried into working.
                    raise _auth_error(error.read()) from None
                if error.code >= 500 and attempt + 1 < attempts:
                    attempt += 1
                    time.sleep(delay)
                    delay = min(delay * 2, 10)
                    continue
                raise CloudError(f"MinerU returned HTTP {error.code}") from None
            except (OSError, ValueError) as error:
                if attempt + 1 < attempts:
                    attempt += 1
                    time.sleep(delay)
                    delay = min(delay * 2, 10)
                    continue
                raise CloudError(f"MinerU is unreachable: {error}") from None

            if not isinstance(payload, dict):
                raise CloudError("MinerU returned an invalid API response")
            code = payload.get("code")
            if code == 0:
                return payload.get("data") or {}
            if code in ("A0202", "A0211"):
                raise _auth_error(json.dumps(payload).encode("utf-8"))
            if code == -60018:
                USAGE_LEDGER.latch_exhausted()
                raise CloudQuotaExhausted("MinerU daily file quota is exhausted")
            if code in (-60009, -10001, -60007) and attempt + 1 < attempts:
                attempt += 1
                time.sleep(delay)
                delay = min(delay * 2, 10)
                continue
            # Do not echo arbitrary server text: it can include signed URLs.
            raise CloudError(f"MinerU error {code}")
        raise CloudError("MinerU request failed")

    @staticmethod
    def _put_file(url: str, path: Path) -> None:
        parsed = urlsplit(url)
        if parsed.scheme != "https" or not parsed.hostname:
            raise CloudError("MinerU returned a non-HTTPS upload URL")
        connection_type = (
            http.client.HTTPSConnection if parsed.scheme == "https"
            else http.client.HTTPConnection
        )
        connection = connection_type(parsed.hostname, parsed.port, timeout=120)
        target = parsed.path + (f"?{parsed.query}" if parsed.query else "")
        try:
            connection.putrequest("PUT", target)
            connection.putheader("Content-Length", str(path.stat().st_size))
            # MinerU explicitly requires that Content-Type be omitted.
            connection.endheaders()
            with path.open("rb") as source:
                while chunk := source.read(1024 * 1024):
                    connection.send(chunk)
            response = connection.getresponse()
            response.read()
            if not 200 <= response.status < 300:
                raise CloudError(f"MinerU upload returned HTTP {response.status}")
        except (OSError, http.client.HTTPException) as error:
            raise CloudError(f"MinerU upload failed: {error}") from None
        finally:
            connection.close()

    @staticmethod
    def _safe_extract(zip_path: Path, destination: Path) -> None:
        destination.mkdir(parents=True, exist_ok=True)
        root = destination.resolve()
        with zipfile.ZipFile(zip_path) as archive:
            for member in archive.infolist():
                target = (destination / member.filename).resolve()
                if target != root and root not in target.parents:
                    raise CloudError("MinerU result zip contains an unsafe path")
                archive.extract(member, destination)

    @staticmethod
    def _download_zip(url: str, destination: Path) -> None:
        if urlsplit(url).scheme != "https":
            raise CloudError("MinerU returned a non-HTTPS result URL")
        for attempt in range(3):
            try:
                with urllib.request.urlopen(url, timeout=60) as response, destination.open("wb") as out:
                    shutil.copyfileobj(response, out, length=1024 * 1024)
                return
            except OSError as error:
                if attempt == 2:
                    raise CloudError("MinerU result download failed") from None
                time.sleep(2 ** attempt)

    @staticmethod
    def _write_pdf_slice(
        source: Path,
        start: int,
        end: int,
        destination: Path,
    ) -> None:
        with fitz.open(source) as original, fitz.open() as sliced:
            sliced.insert_pdf(original, from_page=start, to_page=end - 1)
            sliced.save(destination, garbage=4, deflate=True)

    @classmethod
    def _physical_parts(
        cls,
        source: Path,
        start: int,
        end: int,
        workspace: Path,
    ) -> list[tuple[Path, int, int]]:
        """Make <=200 MB page slices, recursively splitting dense ranges."""
        destination = workspace / f"{source.stem}-{start + 1}-{end}.pdf"
        cls._write_pdf_slice(source, start, end, destination)
        if destination.stat().st_size <= LIMITS["max_file_bytes"]:
            return [(destination, start, end)]
        destination.unlink(missing_ok=True)
        if end - start <= 1:
            raise CloudError(
                f"page {start + 1} of {source.name} exceeds MinerU's 200 MB limit"
            )
        midpoint = start + (end - start) // 2
        return [
            *cls._physical_parts(source, start, midpoint, workspace),
            *cls._physical_parts(source, midpoint, end, workspace),
        ]

    @classmethod
    def _build_tasks(
        cls,
        documents: list[CloudDocument],
        workspace: Path,
    ) -> list[_Task]:
        tasks: list[_Task] = []
        slices = workspace / "upload-slices"
        slices.mkdir(parents=True, exist_ok=True)
        for document in documents:
            path = Path(document.pdf_path)
            document_slices = slices / uuid.uuid4().hex
            document_slices.mkdir()
            with fitz.open(path) as pdf:
                document.total_pages = pdf.page_count
            if document.total_pages == 0:
                raise CloudError(f"{path.name} has no pages")

            if path.stat().st_size > LIMITS["max_file_bytes"]:
                ranges: list[tuple[Path, int, int]] = []
                for start in range(0, document.total_pages, LIMITS["max_pages_per_task"]):
                    end = min(start + LIMITS["max_pages_per_task"], document.total_pages)
                    ranges.extend(cls._physical_parts(path, start, end, document_slices))
                for source_path, start, end in ranges:
                    tasks.append(_Task(
                        document=document,
                        data_id=f"oculus-{uuid.uuid4().hex}",
                        upload_name=source_path.name,
                        source_path=source_path,
                        page_offset=start,
                        page_count=end - start,
                        page_ranges=None,
                    ))
                continue

            for start in range(0, document.total_pages, LIMITS["max_pages_per_task"]):
                end = min(start + LIMITS["max_pages_per_task"], document.total_pages)
                data_id = f"oculus-{uuid.uuid4().hex}"
                tasks.append(_Task(
                    document=document,
                    data_id=data_id,
                    upload_name=f"{path.stem}__oculus_{start + 1}_{end}{path.suffix}",
                    source_path=path,
                    page_offset=start,
                    page_count=end - start,
                    page_ranges=f"{start + 1}-{end}" if document.total_pages > LIMITS["max_pages_per_task"] else None,
                ))
        return tasks

    def extract_documents(self, documents: list[CloudDocument]) -> dict[str, tuple[list[dict], int]]:
        with tempfile.TemporaryDirectory(prefix="mineru-cloud-") as temporary:
            workspace = Path(temporary)
            tasks = self._build_tasks(documents, workspace)
            USAGE_LEDGER.ensure_available(len(tasks))
            for document in documents:
                document.source_images = workspace / uuid.uuid4().hex / "images"

            for batch_start in range(0, len(tasks), LIMITS["max_files_per_batch"]):
                batch = tasks[batch_start:batch_start + LIMITS["max_files_per_batch"]]
                self._run_batch(batch, workspace / f"batch-{batch_start}")

            results: dict[str, tuple[list[dict], int]] = {}
            for document in documents:
                assert document.source_images is not None
                results[document.pdf_path] = render(
                    document.content,
                    document.total_pages,
                    document.source_images,
                    document.images_dir,
                    document.images_rel,
                )
            return results

    def _run_batch(self, tasks: list[_Task], workspace: Path) -> None:
        workspace.mkdir(parents=True, exist_ok=True)
        request_body = {
            "files": [task.api_entry() for task in tasks],
            "model_version": "pipeline",
            "enable_formula": True,
            "enable_table": True,
            "language": "ch",
        }
        # Reserve atomically before network activity so concurrent batches and
        # restarts cannot overspend. Uncertain failures conservatively count.
        USAGE_LEDGER.record(len(tasks), sum(task.page_count for task in tasks))
        data = self._api_json(
            "POST", "/file-urls/batch", request_body, bucket=SUBMIT_BUCKET
        )
        urls = data.get("file_urls") or []
        if len(urls) != len(tasks):
            raise CloudError("MinerU returned the wrong number of upload URLs")
        batch_id = data.get("batch_id")
        if not batch_id:
            raise CloudError("MinerU returned no batch id")

        for task, url in zip(tasks, urls, strict=True):
            self._put_file(url, task.source_path)

        remaining = {task.data_id: task for task in tasks}
        by_name = {task.upload_name: task for task in tasks}
        delay = 2.0
        deadline = time.monotonic() + 60 * 60
        while remaining:
            if time.monotonic() >= deadline:
                raise CloudError("MinerU cloud parse timed out after 60 minutes")
            data = self._api_json(
                "GET",
                f"/extract-results/batch/{batch_id}",
                bucket=POLL_BUCKET,
            )
            for result in data.get("extract_result") or []:
                task = remaining.get(result.get("data_id")) or by_name.get(result.get("file_name"))
                if task is None or task.data_id not in remaining:
                    continue
                state = result.get("state")
                progress = result.get("extract_progress") or {}
                if state == "running":
                    done = min(task.page_count, int(progress.get("extracted_pages") or 0))
                    self._report_progress(task, done)
                if state == "failed":
                    raise CloudError(
                        f"MinerU failed {Path(task.document.pdf_path).name}"
                    )
                if state != "done":
                    continue
                zip_url = result.get("full_zip_url")
                if not zip_url:
                    raise CloudError("MinerU completed without a result zip")
                self._collect_result(task, zip_url, workspace)
                remaining.pop(task.data_id, None)
                self._report_progress(task, task.page_count)
            if remaining:
                time.sleep(delay)
                delay = min(10.0, delay * 1.4)

    @staticmethod
    def _report_progress(task: _Task, done: int) -> None:
        document = task.document
        document.task_progress[task.data_id] = max(
            document.task_progress.get(task.data_id, 0),
            max(0, min(task.page_count, done)),
        )
        if document.on_progress:
            document.on_progress({
                "pages_done": min(document.total_pages, sum(document.task_progress.values())),
                "total_pages": document.total_pages,
                "done": False,
                "backend": "mineru-cloud",
            })

    def _collect_result(self, task: _Task, zip_url: str, workspace: Path) -> None:
        result_dir = workspace / task.data_id
        zip_path = workspace / f"{task.data_id}.zip"
        self._download_zip(zip_url, zip_path)
        try:
            self._safe_extract(zip_path, result_dir)
        except (OSError, zipfile.BadZipFile) as error:
            raise CloudError(f"MinerU returned an invalid result zip: {error}") from None

        content_files = list(result_dir.rglob("*_content_list.json"))
        if not content_files:
            raise CloudError("MinerU result contains no content_list.json")
        content_path = content_files[0]
        try:
            content = json.loads(content_path.read_text(encoding="utf-8"))
        except (OSError, ValueError) as error:
            raise CloudError(f"MinerU content list is unreadable: {error}") from None
        if not isinstance(content, list) or not all(isinstance(item, dict) for item in content):
            raise CloudError("MinerU content list has an invalid shape")

        source_images = content_path.parent / "images"
        assert task.document.source_images is not None
        task.document.source_images.mkdir(parents=True, exist_ok=True)
        image_names: dict[str, str] = {}
        for item in content:
            absolute = dict(item)
            try:
                page_idx = int(item.get("page_idx") or 0)
            except (TypeError, ValueError):
                raise CloudError("MinerU content list has an invalid page index") from None
            if not 0 <= page_idx < task.page_count:
                raise CloudError("MinerU content list page index is outside its task range")
            absolute["page_idx"] = page_idx + task.page_offset
            image_path = item.get("img_path")
            if image_path:
                original = Path(image_path).name
                if original in image_names:
                    absolute["img_path"] = f"images/{image_names[original]}"
                    task.document.content.append(absolute)
                    continue
                target = original
                destination = task.document.source_images / target
                if destination.exists():
                    target = f"p{task.page_offset + 1}_{original}"
                    destination = task.document.source_images / target
                source = source_images / original
                if source.is_file():
                    shutil.copy2(source, destination)
                    absolute["img_path"] = f"images/{target}"
                    image_names[original] = target
            task.document.content.append(absolute)
