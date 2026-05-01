#!/usr/bin/env python
"""Tests for SFT training data preparation."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from prepare_rust_train import dataset_files, filter_invalid_entries, invalid_entry_ids, read_entries


def write_entry(path: Path, entry_id: str) -> None:
    path.write_text(
        json.dumps(
            {
                "id": entry_id,
                "messages": [
                    {"role": "system", "content": "You teach Rust."},
                    {"role": "user", "content": "Question?"},
                    {"role": "assistant", "content": "Answer."},
                ],
            }
        )
        + "\n",
        encoding="utf-8",
    )


class PrepareRustTrainTests(unittest.TestCase):
    def test_discovers_all_rust_jsonl_files_deterministically(self) -> None:
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            write_entry(root / "rust_zeta.jsonl", "zeta")
            write_entry(root / "rust_alpha.jsonl", "alpha")
            write_entry(root / "notes.jsonl", "ignore")

            files = dataset_files(root)
            entries = read_entries(root)

            self.assertEqual(["rust_alpha.jsonl", "rust_zeta.jsonl"], [path.name for path in files])
            self.assertEqual(["alpha", "zeta"], [entry["id"] for entry in entries])
            self.assertEqual(["rust_alpha.jsonl", "rust_zeta.jsonl"], [entry["_source_file"] for entry in entries])

    def test_configured_dataset_files_override_discovery(self) -> None:
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            write_entry(root / "rust_alpha.jsonl", "alpha")
            write_entry(root / "rust_beta.jsonl", "beta")

            entries = read_entries(root, ["rust_beta.jsonl"])

            self.assertEqual(["beta"], [entry["id"] for entry in entries])

    def test_quality_report_filters_invalid_entries_by_file_and_id(self) -> None:
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            report = root / "quality_report.json"
            report.write_text(
                json.dumps(
                    {
                        "errors": [
                            {
                                "file": "rust_alpha.jsonl",
                                "id": "bad",
                                "message": "Rust code fences must open with ```rust",
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            entries = [
                {"id": "good", "_source_file": "rust_alpha.jsonl"},
                {"id": "bad", "_source_file": "rust_alpha.jsonl"},
                {"id": "bad", "_source_file": "rust_beta.jsonl"},
            ]

            filtered = filter_invalid_entries(entries, invalid_entry_ids(report))

            self.assertEqual(
                [("rust_alpha.jsonl", "good"), ("rust_beta.jsonl", "bad")],
                [(entry["_source_file"], entry["id"]) for entry in filtered],
            )


if __name__ == "__main__":
    unittest.main()
