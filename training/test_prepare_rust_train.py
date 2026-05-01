#!/usr/bin/env python
"""Tests for SFT training data preparation."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from prepare_rust_train import dataset_files, read_entries


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


if __name__ == "__main__":
    unittest.main()
