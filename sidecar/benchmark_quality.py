"""Opt-in real-deck regression. All outputs live in a disposable copy."""

import argparse
import json
import shutil
import tempfile
import time
from pathlib import Path

from model_workers import MEMORY_GOVERNOR, shutdown_workers
from parser import parse_fast_isolated, parse_mode, parse_quality


def main() -> None:
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument("pdf", type=Path)
    args.add_argument("--memory-cap", type=int, default=8192)
    options = args.parse_args()
    MEMORY_GOVERNOR.update_cap(options.memory_cap)
    MEMORY_GOVERNOR.start()
    started = time.monotonic()
    try:
        with tempfile.TemporaryDirectory(prefix="oculus-quality-regression-") as temporary:
            copied = Path(temporary) / options.pdf.name
            shutil.copy2(options.pdf, copied)
            parse_fast_isolated(str(copied))
            try:
                result = parse_quality(str(copied), on_progress=lambda state: print(
                    json.dumps({"progress": state, "memory": MEMORY_GOVERNOR.health()}), flush=True
                ))
            except Exception as error:
                assert parse_mode(copied) == "fast", "failure destroyed fast output"
                print(json.dumps({"quality_error": str(error), "fallback": "fast"}), flush=True)
                raise
            print(json.dumps({
                "result": result,
                "elapsed_seconds": round(time.monotonic() - started, 1),
                "memory": MEMORY_GOVERNOR.health(),
            }), flush=True)
    finally:
        shutdown_workers()


if __name__ == "__main__":
    main()
