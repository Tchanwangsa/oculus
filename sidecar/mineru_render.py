"""Backend-independent rendering of MinerU ``content_list.json`` output.

Both the local pipeline and MinerU's cloud API produce the legacy content-list
shape. Keeping all markdown and image decisions here makes the backend choice
invisible to retrieval: callers receive the same 1-based page records either
way.
"""

import os
import shutil
from pathlib import Path


# A header/footer repeated on at least this fraction of a document's pages is
# template furniture — a running head, a unit code, a university name — not
# content. Detected per document rather than hardcoded, because every faculty
# has its own.
BOILERPLATE_PAGE_RATIO = float(os.environ.get("OCULUS_BOILERPLATE_RATIO", "0.5"))

_BOILERPLATE_MIN_PAGES = 4

# Equations may also carry an img_path, but are rendered as LaTeX.
IMAGE_TYPES = {"image", "chart", "table"}

# Crops render at ~1.5x: 20k px² rejects small template furniture while
# retaining the smallest real figure in the measured deck (58k px²).
MIN_IMAGE_AREA = int(os.environ.get("OCULUS_MIN_IMAGE_AREA", "20000"))

# Preserve the legacy default parser's boilerplate decisions independently of
# backend task size or a memory retry's smaller inference chunks.
RENDER_GROUP_PAGES = 64


def _norm(text: str) -> str:
    return " ".join(text.split()).strip().lower()


def _find_boilerplate(content_list: list[dict], total_pages: int) -> set[str]:
    """Return normalised header/footer strings repeated across most pages."""
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
    return {key for key, pages in pages_with.items() if len(pages) >= threshold}


def _too_small(path: Path) -> bool:
    try:
        from PIL import Image

        with Image.open(path) as image:
            width, height = image.size
        return width * height < MIN_IMAGE_AREA
    except Exception:
        # Unreadable crop: keep it rather than silently losing a real figure.
        return False


def _render_item(
    item: dict,
    images_rel: str,
    dropped: set[str] = frozenset(),
    boilerplate: set[str] = frozenset(),
) -> str | None:
    """Convert one content-list entry to a markdown block, or drop it."""
    kind = item.get("type")

    if kind == "equation":
        return (item.get("text") or "").strip() or None

    if kind in IMAGE_TYPES:
        caption = " ".join(item.get(f"{kind}_caption") or []).strip()
        footnote = " ".join(item.get(f"{kind}_footnote") or []).strip()

        image = item.get("img_path")
        if image and Path(image).name in dropped:
            block = ""
        elif image:
            block = f"![{caption}]({images_rel}/{Path(image).name})"
        else:
            block = (item.get("table_body") or "").strip()
        return "\n\n".join(part for part in (block, footnote) if part) or None

    text = (item.get("text") or "").strip()
    if not text or _norm(text) in boilerplate:
        return None

    if kind == "header" or item.get("text_level"):
        return f"## {text}"
    if kind == "footer":
        return None
    return text


def _sort_key(item: dict) -> tuple:
    """Put headers first while preserving MinerU's body reading order."""
    if item.get("type") == "header":
        bbox = item.get("bbox") or [0, 0, 0, 0]
        return (0, bbox[1], bbox[0])
    return (1, 0, 0)


def _pages_from_content_list(
    content_list: list[dict],
    page_offset: int,
    images_rel: str,
    dropped: set[str] = frozenset(),
    boilerplate: set[str] = frozenset(),
) -> dict[int, list[str]]:
    """Group rendered blocks by 1-based absolute page number."""
    by_page: dict[int, list[dict]] = {}
    for item in content_list:
        page_no = item.get("page_idx", 0) + 1 + page_offset
        by_page.setdefault(page_no, []).append(item)

    out: dict[int, list[str]] = {}
    for page_no, items in by_page.items():
        blocks = [
            block
            for block in (
                _render_item(item, images_rel, dropped, boilerplate)
                for item in sorted(items, key=_sort_key)
            )
            if block is not None
        ]
        if blocks:
            out[page_no] = blocks
    return out


def render(
    content_list: list[dict],
    total_pages: int,
    source_images: Path,
    images_dir: Path,
    images_rel: str,
) -> tuple[list[dict], int]:
    """Render a backend result and copy only referenced, useful image crops."""
    wanted = {
        Path(item["img_path"]).name
        for item in content_list
        if item.get("img_path") and item.get("type") in IMAGE_TYPES
    }
    dropped: set[str] = set()
    image_count = 0
    if wanted and source_images.is_dir():
        images_dir.mkdir(parents=True, exist_ok=True)
        for name in sorted(wanted):
            source = source_images / name
            if not source.is_file():
                continue
            if _too_small(source):
                dropped.add(name)
                continue
            shutil.copy2(source, images_dir / name)
            image_count += 1

    groups: dict[int, list[dict]] = {}
    for item in content_list:
        group = item.get("page_idx", 0) // RENDER_GROUP_PAGES
        groups.setdefault(group, []).append(item)
    by_page: dict[int, list[str]] = {}
    for group, items in groups.items():
        group_pages = min(RENDER_GROUP_PAGES, total_pages - group * RENDER_GROUP_PAGES)
        boilerplate = _find_boilerplate(items, group_pages)
        by_page.update(_pages_from_content_list(items, 0, images_rel, dropped, boilerplate))
    pages = [
        {"page_no": page_no, "markdown": "\n\n".join(by_page[page_no])}
        for page_no in sorted(by_page)
    ]

    # Blank or furniture-only pages still need records: page_no is retrieval's
    # join key and holes would attach good vectors to the wrong markdown.
    seen = {page["page_no"] for page in pages}
    pages.extend(
        {"page_no": page_no, "markdown": ""}
        for page_no in range(1, total_pages + 1)
        if page_no not in seen
    )
    pages.sort(key=lambda page: page["page_no"])
    return pages, image_count
