"""Parent-side client for long-lived JSON-lines model workers."""

from __future__ import annotations

import json
import os
import queue
import signal
import subprocess
import sys
import threading
import time
import uuid
from pathlib import Path
from typing import Callable


class WorkerError(RuntimeError):
    pass


class WorkerDied(WorkerError):
    pass


class WorkerRequestError(WorkerError):
    pass


class WorkerProcess:
    """One serial request stream to a model-owning child interpreter."""

    def __init__(self, script: str, label: str, idle_timeout: float = 15 * 60):
        self.script = script
        self.label = label
        self.idle_timeout = idle_timeout
        self._process: subprocess.Popen | None = None
        self._messages: queue.Queue[str | None] = queue.Queue()
        self._request_lock = threading.Lock()
        self._state_lock = threading.Lock()
        self._active: str | None = None
        self._last_used = 0.0
        self._stop_reason: str | None = None
        self._generation = 0

    @property
    def pid(self) -> int | None:
        with self._state_lock:
            process = self._process
            return process.pid if process is not None and process.poll() is None else None

    @property
    def active(self) -> str | None:
        with self._state_lock:
            return self._active

    @property
    def generation(self) -> int:
        with self._state_lock:
            return self._generation

    @property
    def is_busy(self) -> bool:
        return self.active is not None

    @property
    def is_alive(self) -> bool:
        return self.pid is not None

    def _start(self) -> subprocess.Popen:
        with self._state_lock:
            if self._process is not None and self._process.poll() is None:
                return self._process

            messages: queue.Queue[str | None] = queue.Queue()
            self._messages = messages
            self._stop_reason = None
            sidecar_dir = Path(__file__).resolve().parent
            process = subprocess.Popen(
                [sys.executable, self.script],
                cwd=str(sidecar_dir),
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                bufsize=1,
                env={**os.environ, "PYTHONUNBUFFERED": "1"},
                start_new_session=os.name != "nt",
            )
            self._process = process
            self._generation += 1
            self._last_used = time.monotonic()

        threading.Thread(
            target=self._read_stdout, args=(process, messages), daemon=True,
            name=f"{self.label}-stdout",
        ).start()
        threading.Thread(
            target=self._read_stderr, args=(process,), daemon=True,
            name=f"{self.label}-stderr",
        ).start()
        return process

    def _read_stdout(
        self,
        process: subprocess.Popen,
        messages: queue.Queue[str | None],
    ) -> None:
        assert process.stdout is not None
        try:
            for line in process.stdout:
                messages.put(line.rstrip("\n"))
        finally:
            process.stdout.close()
            messages.put(None)

    def _read_stderr(self, process: subprocess.Popen) -> None:
        assert process.stderr is not None
        try:
            for line in process.stderr:
                print(f"[{self.label}] {line.rstrip()}", flush=True)
        finally:
            process.stderr.close()

    def request(
        self,
        op: str,
        payload: dict | None = None,
        *,
        on_progress: Callable[[dict], None] | None = None,
        timeout: float | None = None,
        start: bool = True,
    ):
        """Send one request and wait for its result.

        ``start=False`` makes maintenance calls a no-op when the worker is not
        already resident, avoiding a model process launch just to reclaim it.
        """
        with self._request_lock:
            if not start and not self.is_alive:
                return None
            process = self._start()
            messages = self._messages
            request_id = uuid.uuid4().hex
            message = {"id": request_id, "op": op, **(payload or {})}

            with self._state_lock:
                self._active = op
                self._last_used = time.monotonic()
            try:
                if process.stdin is None:
                    raise WorkerDied(f"{self.label} has no input pipe")
                process.stdin.write(json.dumps(message, ensure_ascii=False) + "\n")
                process.stdin.flush()

                deadline = time.monotonic() + timeout if timeout else None
                while True:
                    wait = 0.5 if deadline is None else max(0.0, min(0.5, deadline - time.monotonic()))
                    if wait == 0.0:
                        self.stop(f"{op} timed out", force=True)
                        raise WorkerDied(f"{self.label} {op} timed out")
                    try:
                        line = messages.get(timeout=wait)
                    except queue.Empty:
                        if process.poll() is not None:
                            # A render descendant may still hold stdout open
                            # after its owner crashes. EOF alone is not enough.
                            code = process.returncode
                            self.stop(f"exited with status {code}", force=True)
                            raise WorkerDied(f"{self.label} exited with status {code}") from None
                        continue
                    if line is None:
                        with self._state_lock:
                            reason = self._stop_reason
                        code = process.poll()
                        detail = reason or f"exited with status {code}"
                        self.stop(detail, force=True)
                        raise WorkerDied(f"{self.label} {detail}")
                    try:
                        response = json.loads(line)
                    except json.JSONDecodeError:
                        print(f"[{self.label}] {line}", flush=True)
                        continue
                    if response.get("id") != request_id:
                        continue
                    event = response.get("event")
                    if event == "progress":
                        if on_progress:
                            on_progress(response.get("data") or {})
                        continue
                    if event == "result":
                        return response.get("data")
                    if event == "error":
                        raise WorkerRequestError(response.get("error") or f"{op} failed")
            except (BrokenPipeError, OSError) as error:
                raise WorkerDied(f"{self.label} pipe failed: {error}") from error
            finally:
                with self._state_lock:
                    self._active = None
                    self._last_used = time.monotonic()

    def reap_if_idle(self) -> bool:
        with self._state_lock:
            idle_for = time.monotonic() - self._last_used
            can_reap = self._active is None and self._process is not None and idle_for >= self.idle_timeout
        if can_reap:
            return self.stop_if_idle("idle timeout")
        return False

    def stop_if_idle(self, reason: str) -> bool:
        # Checking is_busy alone races the next request's model startup.
        if not self._request_lock.acquire(blocking=False):
            return False
        try:
            if not self.is_alive:
                return False
            self.stop(reason)
            return True
        finally:
            self._request_lock.release()

    def stop(self, reason: str = "stopped", *, force: bool = False) -> None:
        with self._state_lock:
            process = self._process
            if process is None:
                return
            self._stop_reason = reason

        try:
            if os.name != "nt":
                os.killpg(process.pid, signal.SIGKILL if force else signal.SIGTERM)
            elif force:
                process.kill()
            else:
                process.terminate()
        except ProcessLookupError:
            pass

        if process.stdin is not None:
            try:
                process.stdin.close()
            except OSError:
                pass

        if not force:
            try:
                process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                try:
                    if os.name != "nt":
                        os.killpg(process.pid, signal.SIGKILL)
                    else:
                        process.kill()
                except ProcessLookupError:
                    pass
        try:
            process.wait(timeout=1)
        except subprocess.TimeoutExpired:
            pass
        with self._state_lock:
            if self._process is process:
                self._process = None

    def shutdown(self) -> None:
        if self.is_alive and not self.is_busy:
            try:
                self.request("shutdown", timeout=3, start=False)
            except WorkerError:
                pass
        self.stop("sidecar shutdown", force=True)
