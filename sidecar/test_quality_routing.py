"""HTTP/queue/routing contract tests without loading model weights."""

import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from fastapi import HTTPException

import main
import quality_router
from mineru_cloud import CloudAuthError, CloudError
from quality_client import LocalQualityMemoryError
import quality_client
from worker_client import WorkerDied


class QualityRoutingTest(unittest.TestCase):
    def test_local_retry_reduces_window_and_cleans_parent_owned_scratch(self):
        import fitz

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "deck.pdf"
            with fitz.open() as pdf:
                pdf.new_page()
                pdf.save(path)
            payloads = []

            def request(_op, payload, **_kwargs):
                payloads.append(payload)
                self.assertTrue(Path(payload["workspace"]).is_dir())
                if len(payloads) == 1:
                    raise WorkerDied("memory cap")
                return {"pages": [{"page_no": 1, "markdown": "ok"}], "image_count": 0}

            with mock.patch.object(quality_client.MEMORY_GOVERNOR, "admit"), \
                 mock.patch.object(quality_client.QUALITY_WORKER, "request", side_effect=request):
                pages, count = quality_client.parse_local(str(path), root / "images", "images")
            self.assertEqual(pages[0]["markdown"], "ok")
            self.assertEqual(count, 0)
            self.assertLessEqual(payloads[1]["window_pages"], 4)
            self.assertLessEqual(payloads[1]["chunk_pages"], 16)
            self.assertTrue(all(not Path(payload["workspace"]).exists() for payload in payloads))

    def test_parent_import_does_not_load_models(self):
        self.assertNotIn("torch", sys.modules)
        self.assertNotIn("embedder", sys.modules)
        self.assertNotIn("mineru.cli.common", sys.modules)

    def test_local_never_uploads_even_with_token(self):
        with mock.patch.object(quality_router, "cloud_eligible", return_value=True):
            self.assertEqual(quality_router.route_quality("deck.pdf", "local", "token"), "local")
            self.assertFalse(quality_router.may_fallback_to_cloud("deck.pdf", "local", "token"))
            self.assertEqual(quality_router.route_quality("deck.pdf", "auto", "token"), "cloud")

    def test_bad_limits_do_not_partially_change_cap(self):
        before = main.MEMORY_GOVERNOR.cap_mb
        with self.assertRaises(HTTPException):
            main.update_limits(main.LimitsRequest(memory_cap_mb=before + 1024, backend="bad"))
        self.assertEqual(main.MEMORY_GOVERNOR.cap_mb, before)

    def test_cloud_does_not_acquire_local_gate(self):
        with mock.patch.object(quality_router, "route_quality", return_value="cloud"), \
             mock.patch.object(main, "parse_quality", return_value={"backend": "mineru-cloud"}), \
             mock.patch.object(main, "_acquire_heavy_operation") as gate:
            main._run_quality("test-cloud.pdf", {}, "auto", "test-token")
            gate.assert_not_called()
            self.assertTrue(main._progress["test-cloud.pdf"]["done"])

    def test_cloud_failure_falls_back_and_queue_recovers(self):
        with mock.patch.object(quality_router, "route_quality", return_value="cloud"), \
             mock.patch.object(quality_router, "note_cloud_failure"), \
             mock.patch.object(main, "parse_quality", side_effect=[
                 CloudError("offline"), {"backend": "mineru-local"},
             ]):
            main._run_quality("test-fallback.pdf", {}, "auto", "test-token")
        self.assertIsNone(main._heavy_current)
        self.assertEqual(main._progress["test-fallback.pdf"]["backend"], "mineru-local")

    def test_local_memory_failure_falls_back_only_when_opted_in(self):
        with mock.patch.object(quality_router, "route_quality", return_value="local"), \
             mock.patch.object(quality_router, "cloud_eligible", return_value=True), \
             mock.patch.object(main, "parse_quality", side_effect=[
                 LocalQualityMemoryError("two attempts failed"), {"backend": "mineru-cloud"},
             ]):
            main._run_quality("test-memory.pdf", {}, "auto", "test-token")
        self.assertIsNone(main._heavy_current)
        self.assertEqual(main._progress["test-memory.pdf"]["backend"], "mineru-cloud")

    def test_rejected_token_latches_cloud_off_until_a_new_one_is_saved(self):
        try:
            with mock.patch.object(quality_router, "route_quality", return_value="cloud"), \
                 mock.patch.object(main, "parse_quality", side_effect=[
                     CloudAuthError("expired", code="A0211", expired=True),
                     {"backend": "mineru-local"},
                 ]):
                main._run_quality("test-token.pdf", {}, "auto", "stale-token")

            # The file still gets parsed, locally...
            self.assertEqual(main._progress["test-token.pdf"]["backend"], "mineru-local")
            # ...and no later file retries the same dead token.
            self.assertTrue(quality_router.token_rejected())
            self.assertFalse(quality_router.cloud_eligible(__file__, "stale-token"))
            self.assertTrue(main.health()["parse"]["cloud_token_rejected"])

            main.mineru_token_reset()
            self.assertFalse(quality_router.token_rejected())
            self.assertFalse(main.health()["parse"]["cloud_token_rejected"])
        finally:
            quality_router.clear_token_rejected()

    def test_duplicate_request_does_not_start_another_parse(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = str((Path(temporary) / "deck.pdf").resolve())
            main._parse_inflight.add(path)
            try:
                with mock.patch.object(main, "_parse_pdf") as parse:
                    result = main.parse_pdf_endpoint(main.ParseRequest(pdf_path=path, subject_code="test"))
                parse.assert_not_called()
                self.assertEqual(result["quality_status"], "queued")
            finally:
                main._parse_inflight.discard(path)


if __name__ == "__main__":
    unittest.main()
