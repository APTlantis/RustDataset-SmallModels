#!/usr/bin/env python
"""Tests for combining intermediate JSONL files."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from combine_intermediates import read_jsonl, write_jsonl


class CombineIntermediatesTests(unittest.TestCase):
    def test_write_jsonl_reindexes_stably(self) -> None:
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            output = root / "combined.jsonl"
            rows = [
                {"id": "old-9", "source_path": "z.rs", "code": "fn z() {}"},
                {"id": "old-1", "source_path": "a.rs", "code": "fn a() {}"},
            ]

            write_jsonl(output, rows, "code-item")
            combined = read_jsonl(output)

            self.assertEqual(["code-item-000000", "code-item-000001"], [row["id"] for row in combined])
            self.assertEqual(["a.rs", "z.rs"], [row["source_path"] for row in combined])

    def test_read_jsonl_keeps_source_metadata_internal(self) -> None:
        with tempfile.TemporaryDirectory() as raw_root:
            path = Path(raw_root) / "items.jsonl"
            path.write_text(json.dumps({"id": "x", "source_path": "src/lib.rs"}) + "\n", encoding="utf-8")

            rows = read_jsonl(path)

            self.assertEqual("items.jsonl", rows[0]["_combine_source"])
            self.assertEqual(1, rows[0]["_combine_line"])


if __name__ == "__main__":
    unittest.main()
