"""Negative controls for the CI index checker. These fixtures are not product tests."""
from contextlib import redirect_stderr
from functools import partial
from io import StringIO
from pathlib import Path
import runpy
import tempfile
from textwrap import dedent
import unittest


build_index = partial(runpy.run_path(str(Path(__file__).with_name("rfc-test-index.py")))["build_index"],
                      checked_tables={"3.3": "字段"})


class IndexChecks(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.spec = self.root / "docs/RFC.md"
        self.source = self.root / "tests/test_example.py"
        self.catalog = self.root / "tests/rfc-requirements.tsv"
        self.spec.parent.mkdir()
        self.source.parent.mkdir()
        self.spec.write_text("**版本** v1.2.0\n### 3.3 Fields\n| 字段 | 行为 |\n| --- | --- |\n"
                             "| default | Default is reply. |\n| unknown | Invalid value rejects. |\n")
        self.catalog.write_text("3.3-01\tDefault is reply.\t默认回复\t!test_default\n"
                                "3.3-02\tInvalid value rejects.\t非法值\ttest_invalid\n")
        self.source.write_text(dedent("""\
            import unittest
            class Sample(unittest.TestCase):
                # RFC §3.3-01 — 默认返回 reply。
                def test_default(self):
                    observed = self.id().split(".")[-1]
                    # RFC-ASSERT §3.3-01
                    self.assertEqual(observed, "test_default")

                # RFC §3.3-02 — 拒绝非法值。
                def test_invalid(self):
                    self.assertTrue(True)
            """))

    def mutate(self, path, original, replacement, diagnostic):
        text = path.read_text()
        self.assertIn(original, text)
        path.write_text(text.replace(original, replacement, 1))
        try:
            with self.assertRaisesRegex(ValueError, diagnostic):
                build_index(self.root)
        finally:
            path.write_text(text)

    def test_links_reach_the_assertion_and_exact_rfc_line_from_both_indexes(self):
        docs, _ = build_index(self.root)
        for lines in docs.values():
            doc = "\n".join(lines)
            self.assertIn("docs/RFC.md?plain=1#L5)", doc)
            self.assertIn("[断言 L7](../tests/test_example.py#L7)", doc)

    def test_rejects_edges_missing_in_either_direction_even_if_indexes_are_regenerated(self):
        self.mutate(self.catalog, "!test_default", "test_invalid", "missing catalog reference: §3.3-01")
        self.mutate(self.source, "§3.3-02 —", "§3.3-01 —", "missing source annotation: §3.3-02")
        self.mutate(self.catalog, "test_invalid", "removed_test", "unknown or ambiguous test")
        self.mutate(self.source, "    # RFC §3.3-02", "    # §3.3-02", "missing a detailed RFC annotation")

    def test_a_new_table_row_cannot_disappear_from_both_indexes_or_be_covered_only_by_an_issue(self):
        self.mutate(self.spec, "| unknown |", "| future | New behavior. |\n| unknown |",
                    r"§3.3 future: table row needs executable test evidence")
        self.mutate(self.catalog, "!test_default", "#12 TODO", "table row needs executable test evidence")
        self.mutate(self.spec, "| unknown |", "| default |", "duplicate table key")

    def test_rejects_missing_stale_duplicate_or_ambiguous_anchors(self):
        self.mutate(self.spec, "Default is reply.", "Default changed.", "0 matches")
        self.mutate(self.spec, "Default is reply.", "Default is reply.\nDefault is reply.", "2 matches")
        self.mutate(self.catalog, "3.3-02", "3.3-01", "duplicate ID")
        self.mutate(self.source, "# RFC §3.3-02 —", "# RFC §3.3-02 — 重复。\n    # RFC §3.3-02 —",
                    "duplicate annotation")

    def test_rejects_unowned_or_empty_assertion_witnesses(self):
        self.mutate(self.source, "# RFC-ASSERT §3.3-01", "# ordinary comment", "missing assertion witness")
        self.mutate(self.catalog, "!test_default", "test_default", "undeclared assertion witness")
        self.mutate(self.source, 'self.assertEqual(observed, "test_default")', "pass", "immediately precede an assertion")
        self.mutate(self.source, "self.assertEqual", "self.assertSomething", "immediately precede an assertion")
        self.mutate(self.source, "    # RFC §3.3-02", "    # RFC §3.3-02 — dangling\n\n    # RFC §3.3-02",
                    "orphan or malformed")
        self.mutate(self.source, "    # RFC §3.3-02", "    def helper(self):\n        # RFC-ASSERT §3.3-01\n"
                    "        self.assertTrue(True)\n    # RFC §3.3-02", "orphan or malformed")

    def test_literal_comparisons_cannot_gain_evidence_from_a_dynamic_failure_message(self):
        for expression in ('self.assertEqual("reply", "reply")', 'self.assertEqual(1, 1, observed)',
                           'self.assertTrue(True, msg=observed)', 'assert 2 + 2 == 4, observed'):
            with self.subTest(expression=expression):
                self.mutate(self.source, 'self.assertEqual(observed, "test_default")', expression,
                            "immediately precede an assertion")

    def test_unreached_comparison_fails_even_when_the_test_itself_passes(self):
        # An early return leaves both static links intact and unittest reports OK.
        text = self.source.read_text().replace("        # RFC-ASSERT", "        return\n        # RFC-ASSERT")
        self.source.write_text(text)
        build_index(self.root)
        runner = runpy.run_path(str(Path(__file__).with_name("check-rfc-assertions.py")))
        cases = runner["index"]["read_cases"](self.root)
        with redirect_stderr(StringIO()), self.assertRaisesRegex(ValueError, "§3.3-01.*assertion did not execute"):
            runner["run_witnesses"](self.root, cases)
