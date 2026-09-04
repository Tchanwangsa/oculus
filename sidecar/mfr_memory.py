"""Bound MinerU's formula-recognition batch size.

UniMERNet's vision encoder (``UnimerSwinModel``) has no SDPA kernel, so MinerU
loads it with eager attention on Metal. Decoding is autoregressive to
``max_new_tokens`` and every crop in a batch runs until the *longest* one
finishes, so transient attention memory grows with batch size times the longest
formula in that batch. A batch of short formulas costs about 1.2 GiB; the same
batch holding one long derivation costs tens of gigabytes.

MinerU tries to shrink oversized batches itself and cannot here:
``get_mfr_min_dynamic_batch_size`` floors the batch at 16, which is exactly the
size low-VRAM and Metal hosts request, so the shrink loop never runs. Worse,
``finalize_mfr_batch_groups`` merges the trailing group — the one holding the
largest crops, and therefore the longest formulas — into its predecessor,
producing batches *above* the requested size precisely where they hurt most.

Measured whole-tree physical footprint, 2026-09-03, at an 8-page window:

| PDF                            | stock    | capped at 2 |
| ------------------------------ | -------- | ----------- |
| MULT20015_Lab_03_Solutions (6) | 32.6 GiB | 5.6 GiB     |
| COMP30026 09.pdf (50)          | 27.2 GiB | 5.8 GiB     |

Neither run got slower; the 6-page deck finished 5s faster. A cap of 4 was
still measured at 30.0 GiB, so 2 is the ceiling, not a starting point.

The cap is applied by wrapping the one batch-planning function rather than
MinerU's batch-size constants, so merges and floors downstream of it cannot
reintroduce an oversized group.
"""

from __future__ import annotations

import threading


# Raising this reintroduces the blow-up; 4 was measured at 30 GiB.
MAX_MFR_BATCH = 2

_lock = threading.Lock()
_original = None
_installed: int | None = None


def cap_groups(groups, cap: int) -> list[list[int]]:
    """Split every planned batch so none exceeds ``cap`` crops."""
    capped: list[list[int]] = []
    for group in groups:
        for start in range(0, len(group), cap):
            capped.append(list(group[start:start + cap]))
    return capped


def install(max_batch: int = MAX_MFR_BATCH) -> int:
    """Cap every MinerU formula batch at ``max_batch`` crops. Idempotent."""
    cap = max(1, min(int(max_batch), MAX_MFR_BATCH))
    global _original, _installed

    with _lock:
        from mineru.model.mfr.unimernet import Unimernet

        if _original is None:
            _original = Unimernet.build_mfr_batch_groups
        plan = _original

        def build_capped_batch_groups(sorted_areas, requested_batch_size):
            return cap_groups(plan(sorted_areas, requested_batch_size), cap)

        Unimernet.build_mfr_batch_groups = build_capped_batch_groups
        _installed = cap
    return cap


def installed_cap() -> int | None:
    """The cap currently in force, or ``None`` before the first install."""
    with _lock:
        return _installed
