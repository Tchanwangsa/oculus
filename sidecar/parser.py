import fitz
import pymupdf4llm
from pathlib import Path
from PIL import Image

# Lazy-loaded — heavy imports, don't block server startup
_latex_model = None
_docling_converter = None


def _get_latex_model():
    global _latex_model
    if _latex_model is None:
        from pix2tex.cli import LatexOCR
        print("[sidecar] loading pix2tex model…")
        _latex_model = LatexOCR()
        print("[sidecar] pix2tex ready")
    return _latex_model


def _get_docling_converter():
    global _docling_converter
    if _docling_converter is None:
        from docling.document_converter import DocumentConverter
        print("[sidecar] loading docling…")
        _docling_converter = DocumentConverter()
        print("[sidecar] docling ready")
    return _docling_converter


def _crop_bbox(pymupdf_doc, page_no: int, bb, scale: int = 2, pad: int = 4) -> Image.Image:
    page = pymupdf_doc[page_no - 1]
    ph = page.rect.height
    rect = fitz.Rect(bb.l - pad, ph - bb.t - pad, bb.r + pad, ph - bb.b + pad)
    pix = page.get_pixmap(matrix=fitz.Matrix(scale, scale), clip=rect)
    return Image.frombytes("RGB", [pix.width, pix.height], pix.samples)


def parse_fast(pdf_path: str) -> dict:
    """pymupdf4llm — ~10s, good for text-heavy PDFs."""
    path = Path(pdf_path)
    if not path.exists():
        raise FileNotFoundError(pdf_path)

    md = pymupdf4llm.to_markdown(str(path))
    md_path = path.with_suffix(".md")
    md_path.write_text(md, encoding="utf-8")

    return {"md_path": str(md_path), "mode": "fast", "image_count": 0}


_CHUNK_PAGES = 10


def _process_chunk(
    converter, latex_model, pymupdf_doc, pdf_path: str,
    images_dir: Path, pdf_stem: str, img_counter: list,
    start_page: int, end_page: int,
) -> str:
    result = converter.convert(str(pdf_path), page_range=(start_page, end_page))
    doc = result.document

    for item, _ in doc.iterate_items():
        if item.label.value == "formula" and item.prov:
            try:
                img = _crop_bbox(pymupdf_doc, item.prov[0].page_no, item.prov[0].bbox, scale=3)
                item.text = latex_model(img)
            except Exception:
                pass

    img_paths: list[str] = []
    for item, _ in doc.iterate_items():
        if item.label.value == "picture" and item.prov:
            try:
                img = _crop_bbox(pymupdf_doc, item.prov[0].page_no, item.prov[0].bbox, scale=2, pad=6)
                fname = f"p{item.prov[0].page_no}_{img_counter[0]}.png"
                img_counter[0] += 1
                img.save(images_dir / fname)
                img_paths.append(f"{pdf_stem}_images/{fname}")
            except Exception:
                pass

    md = doc.export_to_markdown()
    for img_path in img_paths:
        md = md.replace("<!-- image -->", f"![]({img_path})", 1)
    return md


def parse_quality(pdf_path: str, on_progress=None) -> dict:
    """docling + pix2tex + image extraction in 10-page chunks. Overwrites fast .md."""
    path = Path(pdf_path)
    if not path.exists():
        raise FileNotFoundError(pdf_path)

    images_dir = path.parent / f"{path.stem}_images"
    images_dir.mkdir(parents=True, exist_ok=True)

    converter = _get_docling_converter()
    latex_model = _get_latex_model()
    pymupdf_doc = fitz.open(str(path))
    total_pages = len(pymupdf_doc)
    total_chunks = max(1, (total_pages + _CHUNK_PAGES - 1) // _CHUNK_PAGES)
    img_counter = [0]

    chunks: list[str] = []
    for start in range(1, total_pages + 1, _CHUNK_PAGES):
        end = min(start + _CHUNK_PAGES - 1, total_pages)
        chunk_md = _process_chunk(
            converter, latex_model, pymupdf_doc, str(path),
            images_dir, path.stem, img_counter, start, end,
        )
        chunks.append(chunk_md)
        if on_progress:
            on_progress({
                "chunk": len(chunks),
                "total_chunks": total_chunks,
                "pages_done": end,
                "total_pages": total_pages,
                "done": False,
            })

    md = "\n\n".join(chunks)
    md_path = path.with_suffix(".md")
    md_path.write_text(md, encoding="utf-8")

    return {"md_path": str(md_path), "mode": "quality", "image_count": img_counter[0]}
