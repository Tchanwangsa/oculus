"""Local MinerU pipeline backend.

The backend returns MinerU's legacy content-list shape with absolute page
indexes plus one flat image directory. Markdown policy lives in
``mineru_render`` so the cloud backend can share it exactly.
"""

import gc
import json
import os
import shutil
import tempfile
from pathlib import Path

import mfr_memory
from mfr_memory import MAX_MFR_BATCH
from mineru_render import IMAGE_TYPES, render
from modellock import MODEL_INIT_LOCK


_warmed = False

# MinerU's 64-page internal default exhausted a 36 GB Mac on high-resolution
# slides. Neither environment variable may raise this hard safety ceiling.
MAX_PROCESSING_WINDOW_PAGES = 8

# A larger outer chunk gives useful progress without paying do_parse's fixed
# setup cost every internal window.
MAX_CHUNK_PAGES = 64


def _bounded_int(names: tuple[str, ...], default: int, maximum: int) -> int:
    raw = next((os.environ[name] for name in names if name in os.environ), str(default))
    try:
        requested = int(raw)
    except ValueError:
        requested = default
    return max(1, min(requested, maximum))


PROCESSING_WINDOW_PAGES = _bounded_int(
    ("OCULUS_MINERU_WINDOW_PAGES", "MINERU_PROCESSING_WINDOW_SIZE"),
    MAX_PROCESSING_WINDOW_PAGES,
    MAX_PROCESSING_WINDOW_PAGES,
)
CHUNK_PAGES = _bounded_int(
    ("OCULUS_MINERU_CHUNK_PAGES",), MAX_CHUNK_PAGES, MAX_CHUNK_PAGES
)
# Formula batching, not page batching, is what exhausts memory on Metal.
# See mfr_memory for the measurements behind the ceiling.
MFR_BATCH = _bounded_int(("OCULUS_MINERU_MFR_BATCH",), MAX_MFR_BATCH, MAX_MFR_BATCH)
os.environ["MINERU_PROCESSING_WINDOW_SIZE"] = str(PROCESSING_WINDOW_PAGES)


def _run_mineru(
    pdf_bytes: bytes,
    stem: str,
    out_dir: str,
    start: int,
    end: int,
    window_pages: int = PROCESSING_WINDOW_PAGES,
):
    """Invoke MinerU and return ``(content_list, produced_dir)``."""
    global _warmed

    from mineru.cli.common import do_parse

    def call() -> None:
        bounded_window = max(1, min(window_pages, MAX_PROCESSING_WINDOW_PAGES))
        os.environ["MINERU_PROCESSING_WINDOW_SIZE"] = str(bounded_window)
        do_parse(
            output_dir=out_dir,
            pdf_file_names=[stem],
            pdf_bytes_list=[pdf_bytes],
            p_lang_list=["ch"],
            backend="pipeline",
            parse_method="auto",
            formula_enable=True,
            table_enable=True,
            f_draw_layout_bbox=False,
            f_draw_span_bbox=False,
            f_dump_orig_pdf=False,
            f_dump_model_output=False,
            f_dump_middle_json=False,
            f_dump_md=False,
            f_dump_content_list=True,
            start_page_id=start,
            end_page_id=end,
        )

    if not _warmed:
        with MODEL_INIT_LOCK:
            call()
            _warmed = True
    else:
        call()

    produced = Path(out_dir) / stem / "auto"
    content_path = produced / f"{stem}_content_list.json"
    if not content_path.exists():
        raise RuntimeError(f"MinerU produced no content list at {content_path}")
    return json.loads(content_path.read_text(encoding="utf-8")), produced


def reset() -> None:
    """Force the next parse to re-take the model-initialisation lock."""
    global _warmed
    _warmed = False


def release_transient_memory() -> None:
    """Return dead objects and accelerator caches between outer chunks."""
    gc.collect()
    try:
        import torch

        if torch.backends.mps.is_available():
            torch.mps.empty_cache()
        if torch.cuda.is_available():
            torch.cuda.empty_cache()
    except Exception:
        pass


def extract(
    pdf_path: str,
    workspace: Path,
    on_progress=None,
    *,
    chunk_pages: int = CHUNK_PAGES,
    window_pages: int = PROCESSING_WINDOW_PAGES,
    mfr_batch: int = MFR_BATCH,
) -> tuple[list[dict], Path, int]:
    """Return absolute content items, a flat crop directory, and page count."""
    import fitz

    # Applied per parse, not at import, so a retry can lower it further.
    mfr_memory.install(mfr_batch)

    path = Path(pdf_path)
    pdf_bytes = path.read_bytes()
    with fitz.open(str(path)) as document:
        total_pages = document.page_count
    chunk_pages = max(1, min(chunk_pages, MAX_CHUNK_PAGES))
    window_pages = max(1, min(window_pages, MAX_PROCESSING_WINDOW_PAGES))
    total_chunks = max(1, (total_pages + chunk_pages - 1) // chunk_pages)

    content: list[dict] = []
    staged_images = workspace / "images"
    chunk_no = 0
    for start in range(0, total_pages, chunk_pages):
        end = min(start + chunk_pages - 1, total_pages - 1)
        chunk_dir = workspace / f"c{start}"
        content_list = None
        produced = None
        try:
            content_list, produced = _run_mineru(
                pdf_bytes, path.stem, str(chunk_dir), start, end, window_pages
            )
            for item in content_list:
                absolute = dict(item)
                absolute["page_idx"] = item.get("page_idx", 0) + start
                content.append(absolute)

            wanted = {
                Path(item["img_path"]).name
                for item in content_list
                if item.get("img_path") and item.get("type") in IMAGE_TYPES
            }
            source_images = produced / "images"
            if wanted and source_images.is_dir():
                staged_images.mkdir(parents=True, exist_ok=True)
                for name in wanted:
                    source = source_images / name
                    if source.is_file():
                        shutil.copy2(source, staged_images / name)

            chunk_no += 1
            if on_progress:
                on_progress({
                    "chunk": chunk_no,
                    "total_chunks": total_chunks,
                    "pages_done": end + 1,
                    "total_pages": total_pages,
                    "done": False,
                })
        finally:
            content_list = None
            produced = None
            release_transient_memory()

    return content, staged_images, total_pages


def parse(
    pdf_path: str,
    images_dir: Path,
    images_rel: str,
    on_progress=None,
    *,
    chunk_pages: int = CHUNK_PAGES,
    window_pages: int = PROCESSING_WINDOW_PAGES,
    mfr_batch: int = MFR_BATCH,
    workspace: Path | None = None,
):
    """Parse locally and return ``(page_markdown_records, image_count)``."""
    if workspace is not None:
        content, source_images, total_pages = extract(
            pdf_path, workspace, on_progress=on_progress,
            chunk_pages=chunk_pages, window_pages=window_pages,
            mfr_batch=mfr_batch,
        )
        return render(content, total_pages, source_images, images_dir, images_rel)
    with tempfile.TemporaryDirectory(prefix="mineru-") as temporary:
        content, source_images, total_pages = extract(
            pdf_path,
            Path(temporary),
            on_progress=on_progress,
            chunk_pages=chunk_pages,
            window_pages=window_pages,
            mfr_batch=mfr_batch,
        )
        return render(content, total_pages, source_images, images_dir, images_rel)
