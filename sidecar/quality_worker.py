"""Long-lived MinerU worker, spoken to as JSON lines over stdin/stdout."""

import contextlib
import json
import sys
import time
import traceback
from pathlib import Path


_protocol_out = sys.stdout


def _send(message: dict) -> None:
    _protocol_out.write(json.dumps(message, ensure_ascii=False) + "\n")
    _protocol_out.flush()


def _handle(request: dict) -> bool:
    request_id = request.get("id")
    op = request.get("op")
    if op == "shutdown":
        _send({"id": request_id, "event": "result", "data": {"status": "bye"}})
        return False

    try:
        # Dependencies occasionally print to stdout. Keep the protocol stream
        # pure by routing all operation output to the inherited stderr pipe.
        with contextlib.redirect_stdout(sys.stderr):
            if op == "parse":
                import mineru_local

                def progress(state: dict) -> None:
                    _send({"id": request_id, "event": "progress", "data": state})

                pages, image_count = mineru_local.parse(
                    request["pdf_path"],
                    Path(request["images_dir"]),
                    request["images_rel"],
                    on_progress=progress,
                    chunk_pages=int(request.get("chunk_pages", 64)),
                    window_pages=int(request.get("window_pages", 8)),
                    mfr_batch=int(request.get("mfr_batch", 2)),
                    workspace=Path(request["workspace"]) if request.get("workspace") else None,
                )
                result = {"pages": pages, "image_count": image_count}
            elif op == "release":
                import mineru_local

                mineru_local.release_transient_memory()
                result = {"released": True}
            elif op == "balloon":
                # Internal regression hook: allocations are held long enough
                # for the parent's 1s governor sampler to terminate this worker.
                chunks = []
                total = int(request.get("megabytes", 256))
                step = max(1, int(request.get("step_mb", 16)))
                for allocated in range(0, total, step):
                    chunks.append(bytearray(min(step, total - allocated) * 1024 * 1024))
                    _send({
                        "id": request_id,
                        "event": "progress",
                        "data": {"allocated_mb": min(total, allocated + step)},
                    })
                    time.sleep(0.03)
                time.sleep(float(request.get("hold_seconds", 10)))
                result = {"allocated_mb": total}
            else:
                raise ValueError(f"unknown operation: {op}")
        _send({"id": request_id, "event": "result", "data": result})
    except BaseException as error:
        traceback.print_exc(file=sys.stderr)
        _send({
            "id": request_id,
            "event": "error",
            "error": str(error),
            "error_type": type(error).__name__,
        })
    return True


def main() -> None:
    for line in sys.stdin:
        try:
            request = json.loads(line)
        except json.JSONDecodeError:
            continue
        if not _handle(request):
            return


if __name__ == "__main__":
    main()
