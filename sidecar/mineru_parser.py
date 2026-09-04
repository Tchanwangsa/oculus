"""Compatibility facade for the local MinerU backend.

New code should import :mod:`mineru_local` or :mod:`mineru_render` directly.
Keeping this module preserves the pre-split parser entry point while cloud and
worker routing are introduced incrementally.
"""

from mineru_local import (
    CHUNK_PAGES,
    MAX_CHUNK_PAGES,
    MAX_PROCESSING_WINDOW_PAGES,
    PROCESSING_WINDOW_PAGES,
    parse,
    release_transient_memory,
    reset,
)
from mineru_render import (
    BOILERPLATE_PAGE_RATIO,
    IMAGE_TYPES,
    MIN_IMAGE_AREA,
    _find_boilerplate,
    _pages_from_content_list,
    _render_item,
    _sort_key,
    _too_small,
)

__all__ = [
    "BOILERPLATE_PAGE_RATIO",
    "CHUNK_PAGES",
    "IMAGE_TYPES",
    "MAX_CHUNK_PAGES",
    "MAX_PROCESSING_WINDOW_PAGES",
    "MIN_IMAGE_AREA",
    "PROCESSING_WINDOW_PAGES",
    "_find_boilerplate",
    "_pages_from_content_list",
    "_render_item",
    "_sort_key",
    "_too_small",
    "parse",
    "release_transient_memory",
    "reset",
]
