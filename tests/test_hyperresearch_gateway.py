"""Safety regressions for the optional Hyperresearch MCP response adapter.

Run without external services: python -m unittest discover -s tests -p test_hyperresearch_gateway.py
"""
import json
import unittest

from tools import hyperresearch_gateway_core as core


class HyperresearchGatewayTests(unittest.TestCase):
    def test_search_never_returns_full_note_bodies(self):
        raw = json.dumps([{
            "id": "alpha", "title": "Title", "body": "PRIVATE_UNTRUSTED_BODY",
            "summary": "A summary", "source": "https://example.org/reference",
            "status": "evergreen",
        }])
        response = core.search_results(raw)
        self.assertNotIn("PRIVATE_UNTRUSTED_BODY", response)
        data = json.loads(response)
        self.assertFalse(data["trusted"])
        self.assertEqual(data["evidence_status"], "cached_unverified")
        self.assertEqual(data["results"][0]["source"], "https://example.org/reference")

    def test_read_note_truncates_and_preserves_unverified_label(self):
        data = json.loads(core.note_result(json.dumps({
            "id": "alpha", "title": "Source", "source": "https://example.org/a",
            "body": "x" * 9500
        })))
        self.assertTrue(data["truncated"])
        self.assertEqual(len(data["body"]), 6000)
        self.assertEqual(data["evidence_status"], "cached_unverified")

    def test_rejects_pathlike_note_ids(self):
        for bad in ("../secret", "/absolute", "C:\\temp\\file", "", ".hidden", "a/b", " "):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                core.note_id(bad)
        self.assertEqual(core.note_id("paper_2026-v2"), "paper_2026-v2")

    def test_source_urls_with_credentials_or_tokens_are_not_exposed(self):
        for bad in ("https://user:pass@example.com/a", "https://example.com/a?key=secret",
                    "file:///var/private", "https://example.com/a#token",
                    "http://[broken", "http://127.0.0.1:9/private?token=secret"):
            with self.subTest(url=bad):
                self.assertIsNone(core.public_source(bad))
        self.assertEqual(core.public_source("https://example.org/guide"), "https://example.org/guide")
        for internal in ("http://127.0.0.1/secret", "http://[::1]/config",
                         "http://192.168.1.1/", "https://localhost/",
                         "https://service.internal/", "http://10.0.0.2/",
                         "https://example.org/evil" + chr(13) + "Injected"):
            with self.subTest(internal=internal):
                self.assertIsNone(core.public_source(internal))

    def test_search_and_sources_have_bounded_counts(self):
        notes = [{"id": str(i), "title": "x" * 1000, "body": "secret"} for i in range(200)]
        result = json.loads(core.search_results(json.dumps(notes)))
        self.assertEqual(len(result["results"]), 8)
        self.assertEqual(len(result["results"][0]["title"]), 180)
        sources = [{"url": "https://example.org/" + str(i), "note_id": str(i)} for i in range(200)]
        result = json.loads(core.sources_result(json.dumps(sources)))
        self.assertEqual(len(result["sources"]), 20)

    def test_invalid_responses_fail_closed(self):
        for raw in ('{}', 'null', '"hi"'):
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                core.search_results(raw)
        with self.assertRaises(ValueError):
            core.note_result("[]")

    def test_upstream_lookup_errors_and_oversize_payloads_fail_closed(self):
        with self.assertRaises(ValueError):
            core.note_result(json.dumps({"error": "Note not found: missing"}))
        with self.assertRaises(ValueError):
            core.search_results(" " * 2_000_001)

    def test_source_check_does_not_promote_cache_to_proof(self):
        response = json.loads(core.source_check(json.dumps({
            "exists": True, "url": "https://example.org/guide",
            "note_id": "alpha", "raw_api_key": "sensitive"
        })))
        self.assertEqual(response["evidence_status"], "cached_unverified")
        self.assertNotIn("raw_api_key", response)


if __name__ == "__main__":
    unittest.main()
