"""Cloud task splitting and persistent usage-ledger tests."""

import io
import tempfile
import unittest
import json
import zipfile
from pathlib import Path
from unittest import mock

import fitz

import urllib.error

from mineru_cloud import (
    CloudAuthError,
    CloudDocument,
    CloudError,
    CloudQuotaExhausted,
    LIMITS,
    MinerUCloudClient,
    UsageLedger,
)


class MinerUCloudTest(unittest.TestCase):
    def test_page_ranges_split_at_documented_limit(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            pdf_path = root / "long.pdf"
            with fitz.open() as pdf:
                for _ in range(401):
                    pdf.new_page(width=72, height=72)
                pdf.save(pdf_path)

            document = CloudDocument(
                pdf_path=str(pdf_path),
                images_dir=root / "out",
                images_rel="out",
            )
            tasks = MinerUCloudClient._build_tasks([document], root / "work")

        self.assertEqual(len(tasks), 3)
        self.assertEqual(
            [(task.page_offset, task.page_count, task.page_ranges) for task in tasks],
            [(0, 200, "1-200"), (200, 200, "201-400"), (400, 1, "401-401")],
        )
        self.assertTrue(all(task.api_entry()["is_ocr"] is False for task in tasks))

    def test_401_is_an_auth_error_not_a_retryable_failure(self):
        client = MinerUCloudClient("stale-token")
        bucket = mock.Mock()
        for code, expired in (("A0211", True), ("A0202", False)):
            body = json.dumps({"msgCode": code, "msg": "user authenticate failed"})
            error = urllib.error.HTTPError(
                "https://mineru.net/api/v4/extract/task/x", 401, "Unauthorized", {},
                io.BytesIO(body.encode("utf-8")),
            )
            with mock.patch("urllib.request.urlopen", side_effect=error) as urlopen:
                with self.assertRaises(CloudAuthError) as caught:
                    client._api_json("GET", "/extract/task/x", bucket=bucket)
            # One attempt: a rejected token cannot be retried into working.
            self.assertEqual(urlopen.call_count, 1)
            self.assertEqual(caught.exception.code, code)
            self.assertIs(caught.exception.expired, expired)
            self.assertIsInstance(caught.exception, CloudError)

    def test_usage_ledger_persists(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "mineru-usage.json"
            first = UsageLedger(path)
            first.record(3, 401)
            second = UsageLedger(path)
            value = second.snapshot()

        self.assertEqual(value["files"], 3)
        self.assertEqual(value["pages"], 401)
        self.assertFalse(value["quota_exhausted"])
        self.assertEqual(LIMITS["max_files_per_batch"], 50)

    def test_upload_poll_download_render_and_out_of_order_progress(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "deck.pdf"
            with fitz.open() as pdf:
                for _ in range(3):
                    pdf.new_page()
                pdf.save(path)
            progress = []
            document = CloudDocument(str(path), root / "images", "deck_images", progress.append)
            client = MinerUCloudClient("test-only-token")
            submitted = []

            def api(method, endpoint, body=None, **_kwargs):
                if method == "POST":
                    self.assertEqual(endpoint, "/file-urls/batch")
                    self.assertEqual(body["language"], "ch")
                    self.assertEqual(body["model_version"], "pipeline")
                    self.assertTrue(body["enable_formula"] and body["enable_table"])
                    submitted.extend(body["files"])
                    return {"batch_id": "batch", "file_urls": ["https://upload/1", "https://upload/2"]}
                self.assertEqual(endpoint, "/extract-results/batch/batch")
                return {"extract_result": [
                    {"data_id": entry["data_id"], "state": "done", "full_zip_url": f"https://result/{index}"}
                    for index, entry in reversed(list(enumerate(submitted)))
                ]}

            def download(url, destination):
                # This intentionally relies on _run_batch creating its folder.
                index = int(url.rsplit("/", 1)[1])
                content = [
                    {"page_idx": page, "type": "text", "text": f"page {index * 2 + page + 1}"}
                    for page in range(2 if index == 0 else 1)
                ]
                with zipfile.ZipFile(destination, "w") as archive:
                    archive.writestr("result/deck_content_list.json", json.dumps(content))

            with mock.patch.dict(LIMITS, {"max_pages_per_task": 2}), \
                 mock.patch("mineru_cloud.USAGE_LEDGER", UsageLedger(root / "usage.json")), \
                 mock.patch.object(client, "_api_json", side_effect=api), \
                 mock.patch.object(client, "_put_file") as upload, \
                 mock.patch.object(client, "_download_zip", side_effect=download):
                pages, count = client.extract_documents([document])[str(path)]
                self.assertEqual(upload.call_count, 2)

            self.assertEqual([page["markdown"] for page in pages], ["page 1", "page 2", "page 3"])
            self.assertEqual(count, 0)
            self.assertEqual([state["pages_done"] for state in progress], [1, 3])

    def test_signed_put_omits_content_type(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "deck.pdf"
            path.write_bytes(b"pdf")
            with mock.patch("mineru_cloud.http.client.HTTPSConnection") as factory:
                connection = factory.return_value
                connection.getresponse.return_value.status = 200
                MinerUCloudClient._put_file("https://upload.example/file?signature=private", path)
                headers = [call.args[0].lower() for call in connection.putheader.call_args_list]
                self.assertNotIn("content-type", headers)
                self.assertIn("content-length", headers)

    def test_quota_reservation_is_hard_and_persistent(self):
        with tempfile.TemporaryDirectory() as temporary:
            ledger = UsageLedger(Path(temporary) / "usage.json")
            with mock.patch.dict(LIMITS, {"daily_files": 2}):
                ledger.record(2, 2001)
                with self.assertRaises(CloudQuotaExhausted):
                    ledger.record(1, 1)
                self.assertEqual(ledger.snapshot()["pages"], 2001)
                ledger.latch_exhausted()
                with self.assertRaises(CloudQuotaExhausted):
                    UsageLedger(ledger.path).ensure_available(0)

    def test_zip_path_traversal_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with zipfile.ZipFile(root / "bad.zip", "w") as archive:
                archive.writestr("../escape.txt", "bad")
            with self.assertRaises(CloudError):
                MinerUCloudClient._safe_extract(root / "bad.zip", root / "out")
            self.assertFalse((root / "escape.txt").exists())


if __name__ == "__main__":
    unittest.main()
