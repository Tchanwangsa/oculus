"""Run one fast parse in a throwaway interpreter, then exit.

`pymupdf4llm.to_markdown` retains roughly 2 GB per lecture deck and never gives
it back. Measured on this library (PyMuPDF 1.27.2, pymupdf4llm 1.27.2.3):

    deck                                pages   RSS after
    Wk_1_B_-_Info_Sec_26_Privacy.pdf       52     2140 MB
    MULT20015_Lecture_3.pdf                27     2165 MB
    MULT20015_Lecture_4.pdf                36     2888 MB
    lecture_bash_slides_1.pdf               3     3048 MB
    Wk_2_-_Authentication.pdf             108     4967 MB
    MULT20015_Lecture_8.pdf                47     5777 MB

It is not Python objects — a gc sweep after each parse finds ~250 leaked
objects, no live `Document`s, and nothing that accounts for gigabytes. It is
not the MuPDF store either: `TOOLS.store_shrink(100)` moves the number not at
all. It is C-level allocation the process keeps, so no in-process cleanup
reaches it, and `write_images=False` makes no difference (2081 MB on the same
deck) — the retention is in the text/layout analysis, not image extraction.

A library sync parses every new PDF. At 105 decks that curve walks straight
through 36 GB of RAM, which is the OOM. Process exit is the only thing that
reliably returns the memory, so each fast parse gets its own interpreter:
across the same six decks the parent stays at 21 MB.

Cost is one interpreter start plus the pymupdf4llm import, ~1s per deck
against a 2-15s parse. Invoked by `parser.parse_fast_isolated`.
"""

import json
import sys

# Printed immediately before the JSON result so the parent can find it even if
# a dependency writes to stdout on its way past.
SENTINEL = "__OCULUS_PARSE_RESULT__"


def main() -> int:
    if len(sys.argv) != 2:
        print(f"{SENTINEL}{json.dumps({'ok': False, 'exc': 'ValueError', 'error': 'usage: parse_worker.py <pdf_path>'})}")
        return 2

    from parser import parse_fast

    try:
        result = parse_fast(sys.argv[1])
    except Exception as e:
        # The type name travels so the parent can rebuild the distinction the
        # HTTP layer cares about: a missing PDF is a 404, anything else a 500.
        print(f"{SENTINEL}{json.dumps({'ok': False, 'exc': type(e).__name__, 'error': str(e)})}")
        return 1

    print(f"{SENTINEL}{json.dumps({'ok': True, 'result': result})}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
