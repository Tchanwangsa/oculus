"""Whole-process-tree memory accounting and kill-based enforcement.

macOS has no cgroup equivalent for this service and torch/Metal maps virtual
regions far larger than their physical cost. The governor therefore samples
``ri_phys_footprint`` — the value jetsam acts on — for the sidecar and every
descendant, and terminates a model worker when the tree crosses the hard cap.
"""

from __future__ import annotations

import ctypes
import os
import signal
import struct
import subprocess
import sys
import threading
import time
from dataclasses import dataclass


MB = 1024 * 1024
DEFAULT_CAP_MB = 8192
MIN_CAP_MB = 5120
SOFT_RATIO = 0.85
SAMPLE_SECONDS = float(os.environ.get("OCULUS_MEMORY_SAMPLE_SECONDS", "1"))


class MemoryAdmissionError(RuntimeError):
    """An operation cannot fit under the configured hard cap."""


@dataclass(frozen=True)
class TreeSnapshot:
    total_bytes: int
    by_pid: dict[int, int]
    parents: dict[int, int]
    complete: bool = True

    def branch_bytes(self, root: int | None) -> int:
        if root is None or root not in self.by_pid:
            return 0
        children: dict[int, list[int]] = {}
        for pid, parent in self.parents.items():
            children.setdefault(parent, []).append(pid)
        total = 0
        stack = [root]
        seen: set[int] = set()
        while stack:
            pid = stack.pop()
            if pid in seen:
                continue
            seen.add(pid)
            total += self.by_pid.get(pid, 0)
            stack.extend(children.get(pid, ()))
        return total


_libproc = None
if sys.platform == "darwin":
    try:
        _libproc = ctypes.CDLL("/usr/lib/libproc.dylib", use_errno=True)
        _libproc.proc_pid_rusage.argtypes = [
            ctypes.c_int,
            ctypes.c_int,
            ctypes.c_void_p,
        ]
        _libproc.proc_pid_rusage.restype = ctypes.c_int
    except OSError:
        _libproc = None


def physical_footprint(pid: int) -> int | None:
    """Return macOS physical footprint, falling back to RSS elsewhere."""
    if _libproc is not None:
        # rusage_info_v2 is larger than this; only ri_phys_footprint at offset
        # 72 is needed. A generous buffer keeps this stable across SDK versions.
        buffer = ctypes.create_string_buffer(256)
        if _libproc.proc_pid_rusage(pid, 2, ctypes.byref(buffer)) == 0:
            return struct.unpack_from("Q", buffer.raw, 72)[0]
        return None

    try:
        output = subprocess.check_output(
            ["ps", "-o", "rss=", "-p", str(pid)],
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
        return int(output) * 1024 if output else None
    except (OSError, subprocess.SubprocessError, ValueError):
        return None


def process_parents() -> dict[int, int]:
    try:
        output = subprocess.check_output(
            ["ps", "-eo", "pid=,ppid="],
            text=True,
            stderr=subprocess.DEVNULL,
        )
    except (OSError, subprocess.SubprocessError):
        return {}
    parents: dict[int, int] = {}
    for line in output.splitlines():
        fields = line.split()
        if len(fields) != 2:
            continue
        try:
            parents[int(fields[0])] = int(fields[1])
        except ValueError:
            continue
    return parents


def descendant_pids(root: int, parents: dict[int, int] | None = None) -> list[int]:
    parents = parents or process_parents()
    children: dict[int, list[int]] = {}
    for pid, parent in parents.items():
        children.setdefault(parent, []).append(pid)
    found: list[int] = []
    stack = [root]
    seen = {root}
    while stack:
        parent = stack.pop()
        for pid in children.get(parent, ()):
            if pid in seen:
                continue
            seen.add(pid)
            found.append(pid)
            stack.append(pid)
    return found


def sample_tree(root: int, extra_pids: tuple[int, ...] = ()) -> TreeSnapshot:
    parents = process_parents()
    descendants = descendant_pids(root, parents)
    pids = list(dict.fromkeys([root, *descendants, *extra_pids]))
    # Known managed children remain measurable even in a restricted test
    # environment where launching ps is denied.
    for pid in extra_pids:
        parents.setdefault(pid, root)
    by_pid = {
        pid: footprint
        for pid in pids
        if (footprint := physical_footprint(pid)) is not None
    }
    return TreeSnapshot(sum(by_pid.values()), by_pid, parents, root in parents)


def terminate_tree(root: int, *, force: bool = True) -> None:
    """Terminate descendants before their parent so none are orphaned."""
    children = descendant_pids(root)
    sig = signal.SIGKILL if force else signal.SIGTERM
    for pid in reversed(children):
        try:
            os.kill(pid, sig)
        except ProcessLookupError:
            pass
    try:
        os.kill(root, sig)
    except ProcessLookupError:
        pass


def estimate_branch_mb(kind: str, pages: int, *, window_pages: int = 8) -> int:
    """Conservative resident branch estimate used before an operation starts."""
    if kind == "quality":
        # Calibrated to the measured 6.8 GB physical peak at an 8-page window.
        return 4200 + min(max(1, pages), max(1, window_pages)) * 325
    if kind == "embed":
        # Real Qwen worker + parent query run: 5335 MiB physical footprint.
        return 5600
    if kind == "fast":
        return 2500 + min(max(1, pages), 64) * 6
    return 512


class MemoryGovernor:
    def __init__(
        self,
        workers: dict[str, object],
        cap_mb: int | None = None,
        *,
        minimum_cap_mb: int = MIN_CAP_MB,
    ):
        configured = cap_mb
        if configured is None:
            try:
                configured = int(os.environ.get("OCULUS_SIDECAR_MEMORY_CAP_MB", DEFAULT_CAP_MB))
            except ValueError:
                configured = DEFAULT_CAP_MB
        self._minimum_cap_mb = minimum_cap_mb
        self._cap_mb = max(minimum_cap_mb, configured)
        self.workers = workers
        self.root_pid = os.getpid()
        self._lock = threading.Condition()
        self._sample_lock = threading.Lock()
        self._snapshot = TreeSnapshot(0, {}, {}, False)
        self._peak_bytes = 0
        self._kills = 0
        self._last_kill: dict | None = None
        self._running = False
        self._thread: threading.Thread | None = None
        self._last_hard_action = 0.0

    @property
    def cap_mb(self) -> int:
        with self._lock:
            return self._cap_mb

    def update_cap(self, cap_mb: int) -> int:
        if cap_mb < self._minimum_cap_mb:
            raise ValueError(
                f"memory cap must be at least {self._minimum_cap_mb} MB"
            )
        with self._lock:
            self._cap_mb = cap_mb
            self._lock.notify_all()
        return cap_mb

    def start(self) -> None:
        with self._lock:
            if self._running:
                return
            self._running = True
        self._sample_once()
        self._thread = threading.Thread(
            target=self._run, daemon=True, name="memory-governor"
        )
        self._thread.start()

    def stop(self) -> None:
        with self._lock:
            self._running = False
            self._lock.notify_all()

    def _run(self) -> None:
        while True:
            with self._lock:
                if not self._running:
                    return
            self._sample_once()
            for worker in self.workers.values():
                worker.reap_if_idle()
            time.sleep(SAMPLE_SECONDS)

    def _sample_once(self) -> TreeSnapshot:
        # Admission and the watchdog both sample. Serialize enforcement so a
        # single breach cannot count twice or kill a freshly restarted worker.
        with self._sample_lock:
            return self._sample_and_enforce()

    def _sample_and_enforce(self) -> TreeSnapshot:
        known = tuple(
            pid for worker in self.workers.values()
            if (pid := worker.pid) is not None
        )
        snapshot = sample_tree(self.root_pid, known)
        with self._lock:
            self._snapshot = snapshot
            self._peak_bytes = max(self._peak_bytes, snapshot.total_bytes)
            cap_bytes = self._cap_mb * MB
            self._lock.notify_all()

        if snapshot.total_bytes >= cap_bytes:
            self._enforce_hard(snapshot)
        elif snapshot.total_bytes >= cap_bytes * SOFT_RATIO:
            self._reclaim_soft()
        return snapshot

    def _reclaim_soft(self) -> None:
        # Per-chunk local parsing already clears accelerator caches. At the tree
        # level, process exit is the only reliable reclaim, so evict idle Qwen,
        # the largest cheap-to-reload block. Quality is retained until another
        # operation needs its room or the hard cap is crossed.
        embed = self.workers.get("embed")
        if embed is not None and embed.is_alive and not embed.is_busy:
            embed.stop_if_idle("soft memory pressure")

    def _enforce_hard(self, snapshot: TreeSnapshot) -> None:
        # Give a killed process a moment to disappear before acting on the same
        # stale accounting sample again.
        if time.monotonic() - self._last_hard_action < 2:
            return

        candidates = []
        for name, worker in self.workers.items():
            pid = worker.pid
            branch = snapshot.branch_bytes(pid)
            if pid and branch:
                candidates.append((bool(worker.is_busy), branch, name, worker, pid))
        if candidates:
            # Prefer the active owner; otherwise discard the largest idle model.
            active = [candidate for candidate in candidates if candidate[0]]
            _, branch, name, worker, pid = max(active or candidates, key=lambda item: item[1])
            reason = f"killed at {snapshot.total_bytes / MB:.0f} MB tree footprint"
            worker.stop(reason, force=True)
        else:
            # The fast parser is already a one-shot subprocess rather than a
            # managed model worker. Kill its whole direct-child branch.
            direct = [
                pid for pid, parent in snapshot.parents.items()
                if parent == self.root_pid and pid in snapshot.by_pid
            ]
            if not direct:
                return
            pid = max(direct, key=snapshot.branch_bytes)
            branch = snapshot.branch_bytes(pid)
            name = "fast"
            reason = f"killed at {snapshot.total_bytes / MB:.0f} MB tree footprint"
            terminate_tree(pid)

        with self._lock:
            self._kills += 1
            self._last_kill = {
                "worker": name,
                "pid": pid,
                "branch_mb": round(branch / MB),
                "reason": reason,
            }
            self._last_hard_action = time.monotonic()

    def reclaim_for(self, kind: str) -> None:
        """Evict the idle model that cannot overlap the requested operation."""
        for name, other in self.workers.items():
            if name != kind:
                other.stop_if_idle(f"making room for {kind}")

    def admit(
        self,
        kind: str,
        pages: int,
        *,
        worker=None,
        window_pages: int = 8,
        timeout: float = 60,
    ) -> None:
        """Wait until the projected whole tree fits, or reject the operation."""
        self.start()
        expected = estimate_branch_mb(kind, pages, window_pages=window_pages) * MB
        deadline = time.monotonic() + timeout
        reclaimed = False
        while True:
            snapshot = self._sample_once()
            if not snapshot.complete:
                raise MemoryAdmissionError(
                    "cannot measure the whole process tree; local model work is deferred"
                )
            current_branch = snapshot.branch_bytes(worker.pid if worker else None)
            non_branch = max(0, snapshot.total_bytes - current_branch)
            cap_bytes = self.cap_mb * MB
            if non_branch + max(current_branch, expected) <= cap_bytes:
                return
            if not reclaimed:
                self.reclaim_for(kind)
                reclaimed = True
                continue
            # Reject only the irreducible parent + operation estimate. Other
            # active branches can finish, so wait for those rather than fail.
            parent_bytes = snapshot.by_pid.get(self.root_pid, 0)
            if parent_bytes + expected > cap_bytes:
                raise MemoryAdmissionError(
                    f"{kind} needs about {(parent_bytes + expected) / MB:.0f} MB "
                    f"but the sidecar cap is {self.cap_mb} MB"
                )
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise MemoryAdmissionError(
                    f"{kind} could not fit under the {self.cap_mb} MB cap"
                )
            with self._lock:
                self._lock.wait(timeout=min(1.0, remaining))
            self.reclaim_for(kind)

    def health(self) -> dict:
        with self._lock:
            return {
                "footprint_mb": round(self._snapshot.total_bytes / MB),
                "cap_mb": self._cap_mb,
                "peak_mb": round(self._peak_bytes / MB),
                "kills": self._kills,
                "last_kill": self._last_kill,
                "measurement_complete": self._snapshot.complete,
                "metric": "phys_footprint" if _libproc is not None else "rss",
            }
