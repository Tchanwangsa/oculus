"""Local extraction/page-offset contracts without model inference."""

import tempfile
import unittest
from pathlib import Path
from unittest import mock

import fitz

import mfr_memory
import mineru_local


class MinerULocalTest(unittest.TestCase):
    def test_chunk_offsets_and_parent_owned_workspace(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            pdf_path = root / "deck.pdf"
            with fitz.open() as pdf:
                for _ in range(3):
                    pdf.new_page()
                pdf.save(pdf_path)
            calls = []
            progress = []

            def extract(_bytes, _stem, out_dir, start, end, window):
                calls.append((start, end, window))
                return [
                    {"page_idx": index, "type": "text", "text": f"page {start + index + 1}"}
                    for index in range(end - start + 1)
                ], Path(out_dir)

            with mock.patch.object(mineru_local, "_run_mineru", side_effect=extract), \
                 mock.patch.object(mineru_local.mfr_memory, "install") as install, \
                 mock.patch.object(mineru_local, "release_transient_memory"):
                pages, count = mineru_local.parse(
                    str(pdf_path), root / "out", "deck_images",
                    on_progress=progress.append, chunk_pages=2, window_pages=4,
                    mfr_batch=1, workspace=root / "owned-by-parent",
                )
            install.assert_called_once_with(1)
            self.assertEqual(calls, [(0, 1, 4), (2, 2, 4)])
            self.assertEqual([page["markdown"] for page in pages], ["page 1", "page 2", "page 3"])
            self.assertEqual([page["page_no"] for page in pages], [1, 2, 3])
            self.assertEqual([state["pages_done"] for state in progress], [2, 3])
            self.assertEqual(count, 0)

    def test_formula_batches_are_split_to_the_cap(self):
        # MinerU merges its trailing group upward, so a planned batch can
        # exceed the requested size exactly where the longest formulas sit.
        planned = [list(range(16)), list(range(16, 39))]
        capped = mfr_memory.cap_groups(planned, 2)
        self.assertTrue(all(len(group) <= 2 for group in capped))
        self.assertEqual(
            [index for group in capped for index in group],
            [index for group in planned for index in group],
        )

    def test_env_override_cannot_raise_the_formula_batch_ceiling(self):
        self.assertLessEqual(mineru_local.MFR_BATCH, mfr_memory.MAX_MFR_BATCH)
        with mock.patch.dict("os.environ", {"OCULUS_MINERU_MFR_BATCH": "16"}):
            self.assertEqual(
                mineru_local._bounded_int(
                    ("OCULUS_MINERU_MFR_BATCH",),
                    mfr_memory.MAX_MFR_BATCH,
                    mfr_memory.MAX_MFR_BATCH,
                ),
                mfr_memory.MAX_MFR_BATCH,
            )


if __name__ == "__main__":
    unittest.main()
