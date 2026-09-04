"""Backend-independent rendering and transactional quality output tests."""

import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from PIL import Image

import mineru_render
import parser


class MinerURenderTest(unittest.TestCase):
    def test_legacy_64_page_boilerplate_grouping_is_preserved(self):
        content = [
            {"type": "header", "text": "Repeated section", "page_idx": page}
            for page in range(32)
        ] + [{"type": "header", "text": "Last title", "page_idx": 129}]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            pages, _ = mineru_render.render(content, 130, root / "source", root / "out", "out")
        self.assertTrue(all(not page["markdown"] for page in pages[:32]))
        self.assertEqual(pages[129]["markdown"], "## Last title")

    def test_content_list_renders_stably_with_blank_pages(self):
        content = [
            {"type": "text", "text": "Body", "page_idx": 0},
            {"type": "header", "text": "Title", "page_idx": 0, "bbox": [0, 10, 1, 20]},
            {
                "type": "chart",
                "page_idx": 1,
                "img_path": "images/chart.png",
                "chart_caption": ["A chart"],
                "chart_footnote": ["Explaining prose"],
            },
            *[
                {"type": "footer", "text": "University", "page_idx": page}
                for page in range(4)
            ],
        ]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            output = root / "deck_images"
            source.mkdir()
            Image.new("RGB", (300, 200), "white").save(source / "chart.png")

            pages, count = mineru_render.render(
                content, 4, source, output, "deck_images"
            )

        self.assertEqual(count, 1)
        self.assertEqual(pages, [
            {"page_no": 1, "markdown": "## Title\n\nBody"},
            {
                "page_no": 2,
                "markdown": "![A chart](deck_images/chart.png)\n\nExplaining prose",
            },
            {"page_no": 3, "markdown": ""},
            {"page_no": 4, "markdown": ""},
        ])

    def test_failed_quality_parse_preserves_fast_artifacts(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            pdf = root / "deck.pdf"
            pdf.write_bytes(b"not opened by the mocked backend")
            pdf.with_suffix(".md").write_text("fast markdown", encoding="utf-8")
            parser.pages_path(pdf).write_text(
                json.dumps({"mode": "fast", "pages": []}), encoding="utf-8"
            )
            images = parser.images_dir_for(pdf)
            images.mkdir()
            (images / "fast.png").write_bytes(b"fast")

            def fail(_path, staged, _progress):
                staged.mkdir(parents=True)
                (staged / "partial.png").write_bytes(b"partial")
                raise RuntimeError("worker died")

            with mock.patch.object(parser, "_parse_quality_mineru", side_effect=fail):
                with self.assertRaisesRegex(RuntimeError, "worker died"):
                    parser.parse_quality(str(pdf))

            self.assertEqual(pdf.with_suffix(".md").read_text(), "fast markdown")
            self.assertTrue((images / "fast.png").is_file())
            self.assertFalse((images / "partial.png").exists())


if __name__ == "__main__":
    unittest.main()
