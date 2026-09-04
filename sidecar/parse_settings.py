"""Live parse settings shared by HTTP requests and health reporting."""

from __future__ import annotations

import os
import threading
from dataclasses import dataclass

from memory_governor import DEFAULT_CAP_MB, MIN_CAP_MB


BACKENDS = {"local", "cloud", "auto"}


@dataclass(frozen=True)
class ParseSettings:
    memory_cap_mb: int = DEFAULT_CAP_MB
    backend: str = "local"


class ParseSettingsState:
    def __init__(self):
        backend = os.environ.get("OCULUS_MINERU_BACKEND", "local")
        if backend not in BACKENDS:
            backend = "local"
        try:
            cap = int(os.environ.get("OCULUS_SIDECAR_MEMORY_CAP_MB", DEFAULT_CAP_MB))
        except ValueError:
            cap = DEFAULT_CAP_MB
        self._value = ParseSettings(max(MIN_CAP_MB, cap), backend)
        self._lock = threading.Lock()

    def get(self) -> ParseSettings:
        with self._lock:
            return self._value

    def update(
        self,
        *,
        memory_cap_mb: int | None = None,
        backend: str | None = None,
    ) -> ParseSettings:
        if backend is not None and backend not in BACKENDS:
            raise ValueError(f"backend must be one of {', '.join(sorted(BACKENDS))}")
        if memory_cap_mb is not None and memory_cap_mb < MIN_CAP_MB:
            raise ValueError(f"memory_cap_mb must be at least {MIN_CAP_MB}")
        with self._lock:
            current = self._value
            self._value = ParseSettings(
                memory_cap_mb=current.memory_cap_mb if memory_cap_mb is None else memory_cap_mb,
                backend=current.backend if backend is None else backend,
            )
            return self._value


PARSE_SETTINGS = ParseSettingsState()
