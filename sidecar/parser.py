import json
import pymupdf4llm
from pathlib import Path


def pages_path(pdf_path) -> Path:
    """Sibling JSON holding per-page markdown.

    `with_suffix` can't produce a two-dot suffix, hence the manual join.
    """
    p = Path(pdf_path)
    return p.parent / f"{p.stem}.pages.json"


def _write_pages(pdf_path, mode: str, pages: list[dict]) -> Path:
    """Persist per-page markdown keyed by 1-based page_no.

    This is the join key the retrieval layer rests on: embeddings are computed
    from the page *image*, answers are hydrated from the page *markdown*, and
    the two meet on (file, page_no). Without it a hit can only be resolved to a
    whole document.
    """
    out = pages_path(pdf_path)
    out.write_text(json.dumps({
        "pdf": Path(pdf_path).name,
        "mode": mode,
        "page_count": len(pages),
        "pages": pages,
    }, ensure_ascii=False), encoding="utf-8")
    return out


def parse_fast(pdf_path: str) -> dict:
    """pymupdf4llm — ~2s, good for text-heavy PDFs.

    `page_chunks=True` costs nothing and gives page attribution immediately, so
    embedding can start off the fast tier without waiting for the quality pass.
    """
    path = Path(pdf_path)
    if not path.exists():
        raise FileNotFoundError(pdf_path)

    chunks = pymupdf4llm.to_markdown(str(path), page_chunks=True)
    pages = [
        {"page_no": c["metadata"]["page_number"], "markdown": c["text"].strip()}
        for c in chunks
    ]

    md = "\n\n".join(p["markdown"] for p in pages)
    md_path = path.with_suffix(".md")
    md_path.write_text(md, encoding="utf-8")
    _write_pages(path, "fast", pages)

    return {
        "md_path": str(md_path),
        "pages_path": str(pages_path(path)),
        "mode": "fast",
        "image_count": 0,
        "page_count": len(pages),
    }


def _parse_quality_mineru(path: Path, images_dir: Path, on_progress) -> tuple[list[dict], int]:
    """MinerU pipeline backend. Returns (pages, image_count)."""
    import mineru_parser

    try:
        return mineru_parser.parse(
            str(path), images_dir, f"{path.stem}_images", on_progress=on_progress
        )
    except Exception:
        # Don't let a failed weight load leave state that makes every
        # subsequent parse fail the same way.
        mineru_parser.reset()
        raise


def parse_quality(pdf_path: str, on_progress=None) -> dict:
    """Quality parse via MinerU: LaTeX formulas + extracted images.

    Overwrites the fast .md.
    """
    path = Path(pdf_path)
    if not path.exists():
        raise FileNotFoundError(pdf_path)

    images_dir = path.parent / f"{path.stem}_images"
    images_dir.mkdir(parents=True, exist_ok=True)

    pages, image_count = _parse_quality_mineru(path, images_dir, on_progress)

    md = "\n\n".join(p["markdown"] for p in pages)
    md_path = path.with_suffix(".md")
    md_path.write_text(md, encoding="utf-8")
    _write_pages(path, "quality", pages)

    return {
        "md_path": str(md_path),
        "pages_path": str(pages_path(path)),
        "mode": "quality",
        "image_count": image_count,
        "page_count": len(pages),
    }
