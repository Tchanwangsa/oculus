"""Quality-tier PDF parse via MinerU's pipeline backend.

Replaces docling's formula enrichment. On the linear-algebra deck that motivated
this, docling's CodeFormulaV2 cost 72s/page and still emitted two math blocks
KaTeX refuses to render (one a 4KB run of `\\ \\ \\` spacers — the classic
seq2seq repetition collapse) plus at least one wrong formula: it dropped the `i`
from `e^{iθ} = cosθ + i sinθ`. MinerU's UniMERNet gets that line right at
~1.4s/page.

We build per-page markdown from MinerU's `content_list.json` rather than its
`.md`, for two reasons:

  * the content list carries `page_idx` on every item, which is the join key
    retrieval rests on (see parser._write_pages); the flat `.md` has no page
    boundaries at all.
  * MinerU classifies slide titles as `header` and drops them from the `.md`
    entirely — correct for a running head on a paper, wrong for a slide deck
    where the title is the content. Rebuilding lets us keep real titles and
    drop the repeated university-logo boilerplate that docling leaked as body
    text.
"""

import json
import os
import shutil
import tempfile
from pathlib import Path

from modellock import MODEL_INIT_LOCK

# MinerU loads its layout/OCR/formula weights on the first do_parse, not at
# import. Same hazard as docling had: a concurrent load from embedder.py
# materialises the same meta tensors twice and one side loses.
_warmed = False

# A header/footer repeated on at least this fraction of a document's pages is
# template furniture — a running head, a unit code, a university name — not
# content. Detected per document rather than hardcoded, because every faculty
# has its own. 0.5 is deliberately loose: a genuine slide title is essentially
# never repeated across half a deck, and section-divider titles that recur a few
# times stay well under it.
BOILERPLATE_PAGE_RATIO = float(os.environ.get("OCULUS_BOILERPLATE_RATIO", "0.5"))

# Below this page count the ratio is meaningless (on a 2-page PDF, one repeat is
# already 50%), so nothing is treated as furniture.
_BOILERPLATE_MIN_PAGES = 4

# content_list types we render as an <img>. Equations also carry an img_path
# (MinerU crops every region it detects) but we render those as LaTeX.
_IMAGE_TYPES = {"image", "chart", "table"}


def _norm(text: str) -> str:
    return " ".join(text.split()).strip().lower()


def _find_boilerplate(content_list: list[dict], total_pages: int) -> set[str]:
    """Normalised header/footer strings that repeat across most of the document.

    Only `header`/`footer` items are candidates. MinerU has already separated
    those from body text, so a slide's real title is in scope but its bullet
    points are not — which keeps the ratio test from ever eating body content
    that happens to repeat (a recurring "Definition:" label, say).
    """
    if total_pages < _BOILERPLATE_MIN_PAGES:
        return set()

    pages_with: dict[str, set[int]] = {}
    for item in content_list:
        if item.get("type") not in ("header", "footer"):
            continue
        key = _norm(item.get("text") or "")
        if key:
            pages_with.setdefault(key, set()).add(item.get("page_idx", 0))

    threshold = total_pages * BOILERPLATE_PAGE_RATIO
    return {k for k, pages in pages_with.items() if len(pages) >= threshold}


# Minimum crop area, in pixels², for an extracted figure to be worth keeping.
# MinerU's layout model already discards this deck's 72pt university logo before
# it ever becomes a crop — it extracts 6 images where docling extracted 33, of
# which ~24 were that logo repeated on every slide. This is a backstop for
# templates whose furniture *does* survive as a picture region. Crops render at
# ~1.5x, so a 72pt logo lands near 108x108 (11.7k px²) while the smallest real
# figure on this deck is 284x206 (58k px²); 20k sits between with margin.
MIN_IMAGE_AREA = int(os.environ.get("OCULUS_MIN_IMAGE_AREA", "20000"))


def _too_small(path: Path) -> bool:
    try:
        from PIL import Image

        with Image.open(path) as im:
            w, h = im.size
        return w * h < MIN_IMAGE_AREA
    except Exception:
        # Unreadable crop: keep it rather than silently losing a real figure.
        return False


def _render_item(
    item: dict, images_rel: str,
    dropped: set[str] = frozenset(), boilerplate: set[str] = frozenset(),
) -> str | None:
    """One content_list entry -> a markdown block, or None to drop it."""
    kind = item.get("type")

    if kind == "equation":
        # Already delimited with $$ by MinerU.
        return (item.get("text") or "").strip() or None

    if kind in _IMAGE_TYPES:
        # Captions and footnotes are keyed by the item's own type, and the
        # footnote is where the real explanatory prose lands — the eigenvector
        # slide's "Matrix A acts by stretching the vector x…" is a
        # chart_footnote, and reading only image_caption dropped it from the
        # page entirely.
        caption = " ".join(item.get(f"{kind}_caption") or []).strip()
        footnote = " ".join(item.get(f"{kind}_footnote") or []).strip()

        img = item.get("img_path")
        if img and Path(img).name in dropped:
            # Page furniture. The figure goes, but any caption/footnote text on
            # it is still real content, so keep that.
            block = ""
        elif img:
            block = f"![{caption}]({images_rel}/{Path(img).name})"
        else:
            # A table may carry HTML instead of an image.
            block = (item.get("table_body") or "").strip()
        return "\n\n".join(p for p in (block, footnote) if p) or None

    text = (item.get("text") or "").strip()
    if not text or _norm(text) in boilerplate:
        return None

    if kind == "header" or item.get("text_level"):
        return f"## {text}"
    if kind == "footer":
        return None
    return text


def _sort_key(item: dict) -> tuple:
    """Headers first (by vertical position), then body in MinerU's own order.

    MinerU treats headers as page furniture and emits them *after* the body,
    which for a paper is harmless and for a slide deck puts every title at the
    bottom of its slide. Body order is left alone — MinerU has already resolved
    multi-column reading order there and bbox sorting would undo it.
    """
    if item.get("type") == "header":
        bbox = item.get("bbox") or [0, 0, 0, 0]
        return (0, bbox[1], bbox[0])
    return (1, 0, 0)


def _pages_from_content_list(
    content_list: list[dict], page_offset: int, images_rel: str,
    dropped: set[str] = frozenset(), boilerplate: set[str] = frozenset(),
) -> dict[int, list[str]]:
    """Group rendered blocks by 1-based absolute page number."""
    by_page: dict[int, list[dict]] = {}
    for item in content_list:
        page_no = item.get("page_idx", 0) + 1 + page_offset
        by_page.setdefault(page_no, []).append(item)

    out: dict[int, list[str]] = {}
    for page_no, items in by_page.items():
        # sorted() is stable, so body items keep their original relative order.
        blocks = [
            b
            for b in (
                _render_item(i, images_rel, dropped, boilerplate)
                for i in sorted(items, key=_sort_key)
            )
            if b is not None
        ]
        if blocks:
            out[page_no] = blocks
    return out


def _run_mineru(pdf_bytes: bytes, stem: str, out_dir: str, start: int, end: int):
    """Invoke MinerU in-process; returns (content_list, produced_dir).

    `start`/`end` are 0-based inclusive page ids. MinerU slices the PDF before
    parsing, so page_idx in the result is relative to the slice — the caller
    adds the offset back.
    """
    global _warmed

    from mineru.cli.common import do_parse

    def call():
        do_parse(
            output_dir=out_dir,
            pdf_file_names=[stem],
            pdf_bytes_list=[pdf_bytes],
            p_lang_list=["ch"],  # PP-OCRv5 'ch' is the bilingual zh/en model
            backend="pipeline",
            parse_method="auto",
            formula_enable=True,
            table_enable=True,
            # Debug artefacts: layout/span-annotated PDFs and a copy of the
            # original. Pure overhead for us and they triple the output size.
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
    cl_path = produced / f"{stem}_content_list.json"
    if not cl_path.exists():
        raise RuntimeError(f"MinerU produced no content list at {cl_path}")
    return json.loads(cl_path.read_text(encoding="utf-8")), produced


def reset() -> None:
    """Force the next parse to re-take the init lock.

    Mirrors parser.reset_docling: if weights failed to materialise we want the
    next attempt serialised again rather than racing straight in.
    """
    global _warmed
    _warmed = False


# Chunking exists only so the UI gets progress, and it is expensive: each extra
# window costs ~40s of fixed setup regardless of its size. Measured on the
# 24-page matrices deck — 1 window 21.1s, 2 windows 73.5s, 3 windows 101.8s.
# 64 is MinerU's own internal window_size, so at this value a normal lecture
# deck is a single window and only genuinely long PDFs are split (reporting
# progress roughly once a minute, which is the point).
CHUNK_PAGES = int(os.environ.get("OCULUS_MINERU_CHUNK_PAGES", "64"))


def parse(pdf_path: str, images_dir: Path, images_rel: str, on_progress=None):
    """Parse `pdf_path` -> ([{page_no, markdown}, …], image_count).

    Images land in the caller's `{stem}_images/` directory and are referenced
    relative to the PDF, which is what the viewer's <img> resolver expects.
    """
    import fitz

    path = Path(pdf_path)
    pdf_bytes = path.read_bytes()
    total_pages = len(fitz.open(str(path)))
    total_chunks = max(1, (total_pages + CHUNK_PAGES - 1) // CHUNK_PAGES)

    pages: list[dict] = []
    image_count = [0]
    chunk_no = 0

    with tempfile.TemporaryDirectory(prefix="mineru-") as tmp:
        for start in range(0, total_pages, CHUNK_PAGES):
            end = min(start + CHUNK_PAGES - 1, total_pages - 1)
            chunk_dir = os.path.join(tmp, f"c{start}")
            content_list, produced = _run_mineru(
                pdf_bytes, path.stem, chunk_dir, start, end
            )

            # MinerU crops *every* detected region to disk — on this deck that
            # is 67 files of which the markdown references 6, the rest being
            # equation and table crops we render as LaTeX instead. Copy only
            # what the markdown actually points at.
            wanted = {
                Path(i["img_path"]).name
                for i in content_list
                if i.get("img_path") and i.get("type") in _IMAGE_TYPES
            }
            src_images = produced / "images"
            dropped: set[str] = set()
            if wanted and src_images.is_dir():
                images_dir.mkdir(parents=True, exist_ok=True)
                for name in wanted:
                    src = src_images / name
                    if not src.is_file():
                        continue
                    if _too_small(src):
                        dropped.add(name)
                        continue
                    shutil.copy2(src, images_dir / name)
                    image_count[0] += 1

            boilerplate = _find_boilerplate(content_list, end - start + 1)
            by_page = _pages_from_content_list(
                content_list, start, images_rel, dropped, boilerplate
            )
            for page_no in sorted(by_page):
                pages.append(
                    {"page_no": page_no, "markdown": "\n\n".join(by_page[page_no])}
                )

            chunk_no += 1
            if on_progress:
                on_progress({
                    "chunk": chunk_no,
                    "total_chunks": total_chunks,
                    "pages_done": end + 1,
                    "total_pages": total_pages,
                    "done": False,
                })

    # A page whose every block was dropped (blank slide, logo only) never
    # appears in the content list. Retrieval joins on page_no, so emit it empty
    # rather than leaving a hole in the sequence.
    seen = {p["page_no"] for p in pages}
    pages.extend({"page_no": n, "markdown": ""} for n in range(1, total_pages + 1) if n not in seen)
    pages.sort(key=lambda p: p["page_no"])
    return pages, image_count[0]
