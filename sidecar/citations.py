"""Map a location in generated markdown back to the page of its source PDF.

Both parse tiers build their markdown the same way — `"\n\n".join(page
markdown)`, see `parse_fast` and `parse_quality` — and record the same per-page
pieces in `.pages.json`. That makes the offset of every page in the .md a pure
function of the recorded page lengths: no fuzzy matching, no re-parsing, and no
bbox data needed. A citation of `05.md:214` resolves to `05.pdf` page 9 exactly.

The mapping is only as good as that invariant, so `page_for_line` verifies it
once per file and refuses to guess if the .md has been edited since the parse.
"""

import bisect
import json
from pathlib import Path

SEPARATOR = "\n\n"


class PageMapError(Exception):
    """The markdown and its .pages.json no longer agree, or there is no record."""


class PageMap:
    """Line and offset lookups between one .md and the PDF it came from."""

    def __init__(self, md_path, pdf_name: str, mode: str, spans: list[tuple[int, int, int]], line_starts: list[int]):
        self.md_path = Path(md_path)
        self.pdf_name = pdf_name
        self.mode = mode
        # (page_no, start_offset, end_offset), ascending, half-open.
        self.spans = spans
        self._line_starts = line_starts
        self._starts = [s for _, s, _ in spans]

    @property
    def page_count(self) -> int:
        return len(self.spans)

    def page_for_offset(self, offset: int) -> int:
        """1-based PDF page containing this character offset into the .md."""
        if offset < 0:
            raise PageMapError(f"negative offset {offset}")
        i = bisect.bisect_right(self._starts, offset) - 1
        if i < 0:
            i = 0
        # An offset landing in a separator belongs to the page that just ended,
        # which is what bisect already returns. Empty pages have zero-width
        # spans and are skipped over naturally.
        return self.spans[i][0]

    def page_for_line(self, line: int) -> int:
        """1-based PDF page containing this 1-based line of the .md."""
        if line < 1 or line > len(self._line_starts):
            raise PageMapError(f"line {line} outside {self.md_path.name} (1..{len(self._line_starts)})")
        return self.page_for_offset(self._line_starts[line - 1])

    def lines_for_page(self, page_no: int) -> tuple[int, int]:
        """Inclusive 1-based line range of the .md holding this PDF page."""
        for no, start, end in self.spans:
            if no == page_no:
                first = bisect.bisect_right(self._line_starts, start) - 1 + 1
                last = bisect.bisect_right(self._line_starts, max(end - 1, start)) - 1 + 1
                return first, last
        raise PageMapError(f"page {page_no} not in {self.md_path.name}")

    def cite(self, line: int) -> str:
        """`05.pdf p.9` — what a citation of `05.md:214` should actually say."""
        return f"{self.pdf_name} p.{self.page_for_line(line)}"


def pages_path(md_path) -> Path:
    p = Path(md_path)
    return p.parent / f"{p.stem}.pages.json"


def load(md_path) -> PageMap:
    """Build the map for one .md, or raise PageMapError if it can't be trusted."""
    md_path = Path(md_path)
    record = pages_path(md_path)
    if not record.exists():
        raise PageMapError(f"no .pages.json beside {md_path.name} — not PDF-derived markdown")
    try:
        data = json.loads(record.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        raise PageMapError(f"unreadable {record.name}: {exc}") from exc

    pages = data.get("pages") or []
    md = md_path.read_text(encoding="utf-8")

    # The .md is written with the pages joined and nothing else. Rebuilding it
    # is the whole verification: if it matches, every offset below is exact.
    rebuilt = SEPARATOR.join(p["markdown"] for p in pages)
    if rebuilt.strip() != md.strip():
        raise PageMapError(
            f"{md_path.name} does not match {record.name} "
            f"({len(md)} chars vs {len(rebuilt)} rebuilt) — edited or stale parse"
        )

    # md may have been stripped on write; align offsets to the file as it is.
    base = md.find(rebuilt[:64]) if rebuilt else 0
    if base < 0:
        base = 0

    spans, cursor = [], base
    for p in pages:
        text = p["markdown"]
        spans.append((p["page_no"], cursor, cursor + len(text)))
        cursor += len(text) + len(SEPARATOR)

    line_starts, pos = [0], md.find("\n")
    while pos != -1:
        line_starts.append(pos + 1)
        pos = md.find("\n", pos + 1)

    return PageMap(md_path, data.get("pdf") or f"{md_path.stem}.pdf", data.get("mode", "?"), spans, line_starts)


def resolve(md_path, line: int) -> str | None:
    """Best-effort `foo.pdf p.N` for a grep hit; None if not PDF-derived."""
    try:
        return load(md_path).cite(line)
    except PageMapError:
        return None


def _main(argv: list[str]) -> int:
    """Rewrite `grep -n` hits as PDF page citations.

        grep -rn "resolution" library/ | python -m citations

    Lines that aren't PDF-derived markdown pass through untouched, so this is
    safe to put on the end of any grep.
    """
    import sys

    if argv and argv[0] not in ("-",):
        # Direct query: citations.py FILE.md LINE
        path, line = argv[0], int(argv[1])
        answer = resolve(path, line)
        print(answer or f"{Path(path).name}:{line} (not PDF-derived)")
        return 0

    cache: dict[str, PageMap | None] = {}
    for raw in sys.stdin:
        raw = raw.rstrip("\n")
        head, sep, rest = raw.partition(":")
        num, sep2, text = rest.partition(":")
        if not (sep and sep2 and num.isdigit() and head.endswith(".md")):
            print(raw)
            continue
        if head not in cache:
            try:
                cache[head] = load(head)
            except PageMapError:
                cache[head] = None
        pm = cache[head]
        if pm is None:
            print(raw)
            continue
        try:
            print(f"{pm.pdf_name} p.{pm.page_for_line(int(num))}:{text}")
        except PageMapError:
            print(raw)
    return 0


if __name__ == "__main__":
    import sys

    raise SystemExit(_main(sys.argv[1:]))
