"""Long-lived Qwen embedding worker using the sidecar JSON-line protocol."""

import contextlib
import base64
import gc
import json
import sys
import traceback


_protocol_out = sys.stdout


def _send(message: dict) -> None:
    _protocol_out.write(json.dumps(message, ensure_ascii=False) + "\n")
    _protocol_out.flush()


def _release() -> None:
    import embedder

    embedder._embedder = None
    gc.collect()
    try:
        import torch

        if torch.backends.mps.is_available():
            torch.mps.empty_cache()
        if torch.cuda.is_available():
            torch.cuda.empty_cache()
    except Exception:
        pass


def _handle(request: dict) -> bool:
    request_id = request.get("id")
    op = request.get("op")
    if op == "shutdown":
        _send({"id": request_id, "event": "result", "data": {"status": "bye"}})
        return False

    try:
        with contextlib.redirect_stdout(sys.stderr):
            import embedder

            if op == "embed_pdf":
                def progress(state: dict) -> None:
                    _send({"id": request_id, "event": "progress", "data": state})

                result = embedder.embed_pdf(request["pdf_path"], on_progress=progress)
            elif op == "embed_query":
                vector = embedder.embed_query(request["text"])
                result = {
                    "vector": base64.b64encode(vector.tobytes()).decode("ascii"),
                    "shape": list(vector.shape),
                }
            elif op == "release":
                _release()
                result = {"released": True}
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
