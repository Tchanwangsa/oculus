import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import pymupdf4llm
from pathlib import Path


def pages_path(pdf_path) -> Path:
    """Sibling JSON holding per-page markdown.

    `with_suffix` can't produce a two-dot suffix, hence the manual join.
    """
    p = Path(pdf_path)
    return p.parent / f"{p.stem}.pages.json"


def images_dir_for(pdf_path) -> Path:
    """Where both tiers put extracted images, beside the PDF.

    The directory name is also the link prefix written into the markdown, so a
    reader resolving `foo_images/x.png` against the .md's own folder finds it.
    """
    p = Path(pdf_path)
    return p.parent / f"{p.stem}_images"


# Bump when a change makes previously written markdown worth redoing. Version 2
# extracts images in the fast tier instead of emitting "picture … intentionally
# omitted" placeholders.
PARSER_VERSION = 2


def parse_mode(pdf_path) -> str:
    """'quality' | 'fast' | 'none' — what still needs doing for this PDF.

    Deliberately not inferred from the images directory. That directory is
    created before a parse runs and survives a crash, so an interrupted quality
    pass used to leave an empty folder that read as "already done" — which
    pinned the file to fast markdown forever, because every later request
    skipped it. `.pages.json` is only written once a parse has finished.

    Markdown from an older parser reports `none`, so it gets redone. Quality
    output is never invalidated this way: it is the better text either way, and
    re-running MinerU across a library costs hours.
    """
    if not Path(pdf_path).with_suffix(".md").exists():
        return "none"
    record = pages_path(pdf_path)
    if not record.exists():
        # Markdown with no record: treat as the weaker tier so the quality
        # pass still gets a chance.
        return "fast"
    try:
        data = json.loads(record.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return "fast"

    mode = data.get("mode")
    if mode == "quality":
        return "quality"
    if mode == "fast":
        return "fast" if data.get("parser_version", 1) >= PARSER_VERSION else "none"
    return "fast"


_MD_IMAGE = re.compile(r"!\[([^\]]*)\]\(([^)\s]+)[^)]*\)")


def _localise_images(md: str, images_rel: str) -> str:
    """Point every image link at `{stem}_images/<file>`.

    pymupdf4llm resolves the paths it writes against the *current working
    directory* — the sidecar's own folder — which produces links that mean
    nothing to a reader opening the markdown from the library. Only the
    filename is load-bearing, so keep that and rebuild the prefix.
    """
    def fix(m):
        alt, target = m.group(1), m.group(2)
        if target.startswith(("http://", "https://", "data:")):
            return m.group(0)
        return f"![{alt}]({images_rel}/{Path(target).name})"

    return _MD_IMAGE.sub(fix, md)


_PYMUPDF_NAME = re.compile(r"-(\d+)-(\d+)\.(\w+)$")

# Characters pymupdf4llm rewrites in any path handed to it. A scratch directory
# containing one of them would hit the very bug the scratch directory exists to
# avoid, so pick a root that has none.
_UNSAFE_IN_PATH = set(" ()[]")


def _scratch_root() -> str:
    for candidate in (tempfile.gettempdir(), "/tmp"):
        if candidate and not (_UNSAFE_IN_PATH & set(candidate)):
            return candidate
    raise RuntimeError("no temp directory with a path safe for image extraction")


def _normalise_images(directory: Path) -> dict[str, str]:
    """Dedupe and rename extracted images. Returns {original name: final name}.

    Two jobs, one pass over the files:

    * Slide templates repeat their furniture on every page, so a 40-page deck
      yields 40 byte-identical copies of the same logo. Hashing costs far less
      than the disk they would take, and the reader sees no difference.
    * pymupdf4llm names each crop after the whole document, which on a deck of
      160 pictures puts several kilobytes of repeated filename into the
      markdown — text that is then embedded and fed to a model. `p3_1.png`
      carries the same information.
    """
    if not directory.is_dir():
        return {}

    by_digest: dict[str, str] = {}
    mapping: dict[str, str] = {}

    for f in sorted(directory.iterdir()):
        if not f.is_file():
            continue
        digest = hashlib.sha1(f.read_bytes()).hexdigest()
        if digest in by_digest:
            mapping[f.name] = by_digest[digest]
            f.unlink()
            continue

        m = _PYMUPDF_NAME.search(f.name)
        target = f"p{int(m.group(1))}_{int(m.group(2))}.{m.group(3)}" if m else f.name
        # Distinct images that normalise to one name would clobber each other.
        if target != f.name and (directory / target).exists():
            target = f.name
        if target != f.name:
            f.rename(directory / target)

        by_digest[digest] = target
        mapping[f.name] = target

    return mapping


def _write_pages(
    pdf_path,
    mode: str,
    pages: list[dict],
    *,
    backend: str | None = None,
) -> Path:
    """Persist per-page markdown keyed by 1-based page_no.

    This is the join key the retrieval layer rests on: embeddings are computed
    from the page *image*, answers are hydrated from the page *markdown*, and
    the two meet on (file, page_no). Without it a hit can only be resolved to a
    whole document.
    """
    out = pages_path(pdf_path)
    payload = {
        "pdf": Path(pdf_path).name,
        "mode": mode,
        "parser_version": PARSER_VERSION,
        "page_count": len(pages),
        "pages": pages,
    }
    if backend:
        payload["backend"] = backend
    out.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8")
    return out


def parse_fast(pdf_path: str) -> dict:
    """pymupdf4llm — ~2s, good for text-heavy PDFs.

    `page_chunks=True` costs nothing and gives page attribution immediately, so
    embedding can start off the fast tier without waiting for the quality pass.

    Images are written, not skipped. Left to itself pymupdf4llm replaces every
    picture and formula with `==> picture [w x h] intentionally omitted <==`,
    which on a slide deck is most of the document — worthless to read and worse
    than nothing in the retrieval index.
    """
    path = Path(pdf_path)
    if not path.exists():
        raise FileNotFoundError(pdf_path)

    images_dir = images_dir_for(path)
    images_rel = images_dir.name
    # Start clean: a re-parse renumbers crops, so anything left from a previous
    # run is unreferenced and would only confuse the next reader of this folder.
    shutil.rmtree(images_dir, ignore_errors=True)

    # Extract to a scratch directory, then move. pymupdf4llm sanitises the path
    # it is given — spaces become underscores — and uses that *same* mangled
    # string to save the file, so writing straight into the library fails on
    # macOS, where the data directory is always ".../Application Support/…".
    with tempfile.TemporaryDirectory(prefix="oculus-img-", dir=_scratch_root()) as scratch:
        chunks = pymupdf4llm.to_markdown(
            str(path),
            page_chunks=True,
            write_images=True,
            image_path=scratch,
            image_format="png",
        )
        images_dir.mkdir(parents=True, exist_ok=True)
        for f in Path(scratch).iterdir():
            if f.is_file():
                shutil.move(str(f), images_dir / f.name)

    renamed = _normalise_images(images_dir)

    def clean(text: str) -> str:
        text = _localise_images(text, images_rel)
        for original, final in renamed.items():
            if original != final:
                text = text.replace(f"{images_rel}/{original}", f"{images_rel}/{final}")
        return text.strip()

    pages = [
        {"page_no": c["metadata"]["page_number"], "markdown": clean(c["text"])}
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
        "image_count": len(list(images_dir.iterdir())) if images_dir.is_dir() else 0,
        "page_count": len(pages),
    }


# The fast tier costs ~2 GB of unreclaimable C-level memory per deck, so it runs
# in a throwaway process. See parse_worker.py for the measurements and why no
# in-process cleanup reaches it.
#
# A whole-library parse can be slow but it is never 15 minutes for one fast
# pass; a worker still running at that point is wedged, and the Rust caller's
# own timeout is 20 minutes, so time out under it and return a real error
# rather than letting the request hang.
PARSE_WORKER_TIMEOUT = int(os.environ.get("OCULUS_PARSE_WORKER_TIMEOUT", "900"))


def parse_fast_isolated(pdf_path: str) -> dict:
    """`parse_fast` in its own interpreter. Same return value, same exceptions.

    Set OCULUS_INPROCESS_PARSE=1 to call `parse_fast` directly instead — useful
    when debugging a parse under a profiler, where a subprocess hides the thing
    you are trying to watch. It leaks; do not ship with it on.
    """
    if os.environ.get("OCULUS_INPROCESS_PARSE"):
        return parse_fast(pdf_path)

    if not Path(pdf_path).exists():
        # Raised here rather than in the worker so the caller gets it without
        # paying for an interpreter start.
        raise FileNotFoundError(pdf_path)

    from parse_worker import SENTINEL

    try:
        proc = subprocess.run(
            [sys.executable, "parse_worker.py", str(pdf_path)],
            cwd=str(Path(__file__).resolve().parent),
            capture_output=True,
            text=True,
            timeout=PARSE_WORKER_TIMEOUT,
            env={**os.environ, "PYTHONUNBUFFERED": "1"},
        )
    except subprocess.TimeoutExpired:
        raise RuntimeError(
            f"fast parse timed out after {PARSE_WORKER_TIMEOUT}s: {Path(pdf_path).name}"
        ) from None

    # The worker forwards anything its dependencies printed; keep it visible so
    # the sidecar log still reads as one stream.
    payload = None
    for line in proc.stdout.splitlines():
        if line.startswith(SENTINEL):
            payload = line[len(SENTINEL):]
        else:
            print(line)
    if proc.stderr.strip():
        print(proc.stderr.rstrip())

    if payload is None:
        # No result line at all: the worker died before it could report — an
        # OOM kill or a segfault in MuPDF both land here.
        raise RuntimeError(
            f"fast parse worker exited {proc.returncode} without a result "
            f"({Path(pdf_path).name})"
        )

    data = json.loads(payload)
    if data.get("ok"):
        return data["result"]

    message = data.get("error") or "fast parse failed"
    if data.get("exc") == "FileNotFoundError":
        raise FileNotFoundError(message)
    raise RuntimeError(message)


def _parse_quality_mineru(path: Path, images_dir: Path, on_progress) -> tuple[list[dict], int]:
    """MinerU pipeline backend. Returns (pages, image_count)."""
    from quality_client import parse_local

    return parse_local(
        str(path), images_dir, f"{path.stem}_images", on_progress=on_progress
    )


def parse_quality(
    pdf_path: str,
    on_progress=None,
    *,
    backend: str = "mineru-local",
    mineru_token: str | None = None,
) -> dict:
    """Quality parse via MinerU: LaTeX formulas + extracted images.

    Overwrites the fast .md.
    """
    path = Path(pdf_path)
    if not path.exists():
        raise FileNotFoundError(pdf_path)

    images_dir = images_dir_for(path)
    # Quality output is staged beside the PDF, then swapped into place only on
    # success. A killed worker therefore leaves the fast markdown and its image
    # directory intact as the usable fallback.
    with tempfile.TemporaryDirectory(
        prefix=f".{path.stem}-quality-", dir=str(path.parent)
    ) as temporary:
        staged_images = Path(temporary) / images_dir.name
        if backend == "mineru-cloud":
            if not mineru_token:
                raise RuntimeError("MinerU cloud token is missing")
            from cloud_batcher import CLOUD_BATCHER

            pages, image_count = CLOUD_BATCHER.submit(
                mineru_token,
                str(path),
                staged_images,
                f"{path.stem}_images",
                on_progress=on_progress,
            )
        else:
            pages, image_count = _parse_quality_mineru(
                path, staged_images, on_progress
            )
        if images_dir.is_dir():
            shutil.rmtree(images_dir, ignore_errors=True)
        if staged_images.is_dir():
            staged_images.replace(images_dir)

    md = "\n\n".join(p["markdown"] for p in pages)
    md_path = path.with_suffix(".md")
    md_path.write_text(md, encoding="utf-8")
    _write_pages(path, "quality", pages, backend=backend)

    return {
        "md_path": str(md_path),
        "pages_path": str(pages_path(path)),
        "mode": "quality",
        "backend": backend,
        "image_count": image_count,
        "page_count": len(pages),
    }
