"""One routing decision for local/cloud quality parsing."""

from __future__ import annotations

import threading
import time
from pathlib import Path

from mineru_cloud import cloud_quota_available


_lock = threading.Lock()
_cloud_unavailable_until = 0.0
_token_rejected = False


def note_cloud_failure(cooldown_seconds: float = 60) -> None:
    global _cloud_unavailable_until
    with _lock:
        _cloud_unavailable_until = max(
            _cloud_unavailable_until, time.monotonic() + cooldown_seconds
        )


def note_token_rejected() -> None:
    """MinerU refused the token. Latched, not a cooldown: every remaining file
    would hit the same 401, and only a new token can clear it."""
    global _token_rejected
    with _lock:
        _token_rejected = True


def clear_token_rejected() -> None:
    """Rust calls this when the stored token changes."""
    global _token_rejected
    with _lock:
        _token_rejected = False


def token_rejected() -> bool:
    with _lock:
        return _token_rejected


def cloud_eligible(pdf_path: str, token: str | None) -> bool:
    if not token or not token.strip() or not cloud_quota_available():
        return False
    path = Path(pdf_path)
    try:
        path.stat()
    except OSError:
        return False
    with _lock:
        return not _token_rejected and time.monotonic() >= _cloud_unavailable_until


def route_quality(pdf_path: str, backend: str, token: str | None) -> str:
    """Return ``cloud`` or ``local`` without performing either operation."""
    if backend == "local":
        return "local"
    return "cloud" if cloud_eligible(pdf_path, token) else "local"


def may_fallback_to_cloud(pdf_path: str, backend: str, token: str | None) -> bool:
    # An explicit local choice is also the user's privacy choice: never upload.
    return backend != "local" and cloud_eligible(pdf_path, token)
