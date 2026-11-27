import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("research_trial", Path(__file__).with_name("research_trial.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class VerifySourceTests(unittest.TestCase):
    URL = "https://www.w3.org/WAI/tutorials/images/informative/"
    QUOTE = "Provide alternative text to describe meaningful informative images in the document."
    HTML = "<html><main><p>" + QUOTE + "</p></main><script>evil()</script></html>"

    def test_exact_document_quote_is_valid(self):
        self.assertTrue(module.quote_verified(self.URL, self.URL, self.QUOTE, self.QUOTE, self.HTML))
    def test_wrong_host_is_rejected(self):
        self.assertFalse(module.quote_verified(self.URL,
            "https://www.w3.org.evil.example/WAI/tutorials/images/informative/",
            self.QUOTE, self.QUOTE, self.HTML))
    def test_sibling_and_query_rejected(self):
        for url in ("https://www.w3.org/WAI/tutorials/images/",
                    self.URL + "?test=1", self.URL + "#fragment"):
            self.assertFalse(module.quote_verified(self.URL, url, self.QUOTE, self.QUOTE, self.HTML))
    def test_invented_or_script_quote_rejected(self):
        self.assertFalse(module.quote_verified(self.URL, self.URL,
            "This passage is not present in the source text.", "This passage is not present in the source text.", self.HTML))
        self.assertFalse(module.quote_verified(self.URL, self.URL,
            "evil() is a script payload that must not be reported.", "evil() is a script payload that must not be reported.", self.HTML))
    def test_claim_has_to_be_source_quote(self):
        self.assertFalse(module.quote_verified(self.URL, self.URL,
            self.QUOTE, self.QUOTE + " It is always legally required.", self.HTML))
    def test_no_local_executable_authorization(self):
        import os
        from unittest.mock import patch
        with patch.dict(os.environ, {"GITHUB_ACTIONS": "false", "LUCIA_UPSTREAM_AUTONOMY_TEST": "1"}):
            with self.assertRaises(RuntimeError):
                import asyncio
                asyncio.run(module.trial("w3c"))

if __name__ == "__main__":
    unittest.main()
