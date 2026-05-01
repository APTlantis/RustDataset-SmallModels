#!/usr/bin/env python
"""Tests for qualitative Rust eval harness."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

from qual_eval_rust import PROMPTS, model_specs, run_eval, write_markdown


class QualEvalRustTests(unittest.TestCase):
    def test_prompt_set_has_expected_categories(self) -> None:
        categories = [prompt.category for prompt in PROMPTS]

        self.assertEqual(18, len(PROMPTS))
        self.assertEqual(5, categories.count("concept"))
        self.assertEqual(5, categories.count("completion"))
        self.assertEqual(5, categories.count("repair"))
        self.assertEqual(3, categories.count("api"))

    def test_model_specs_defaults_to_base_without_adapter(self) -> None:
        args = SimpleNamespace(include_base=False, adapter=[])

        self.assertEqual([("base", None)], model_specs(args))

    def test_dry_run_writes_prompt_metadata_without_responses(self) -> None:
        args = SimpleNamespace(
            model_name="local-model",
            seed=123,
            max_new_tokens=64,
            temperature=0.0,
            limit=2,
            adapter=[Path("adapter-a")],
            include_base=True,
            dry_run=True,
            cpu=True,
        )

        report = run_eval(args, {"model": {"name": "ignored"}})

        self.assertEqual(2, len(report["prompts"]))
        self.assertEqual(["base", "adapter-a"], [model["name"] for model in report["models"]])
        self.assertEqual([], report["models"][0]["responses"])

    def test_write_markdown_renders_responses(self) -> None:
        report = {
            "model_name": "model",
            "max_new_tokens": 32,
            "temperature": 0.0,
            "prompts": [
                {
                    "id": "prompt-1",
                    "category": "concept",
                    "system": "system",
                    "user": "Explain ownership.",
                }
            ],
            "models": [
                {
                    "name": "adapter",
                    "adapter": "adapter",
                    "responses": [
                        {
                            "prompt_id": "prompt-1",
                            "category": "concept",
                            "response": "Ownership controls drops.",
                        }
                    ],
                }
            ],
        }
        with tempfile.TemporaryDirectory() as raw_root:
            path = Path(raw_root) / "eval.md"

            write_markdown(path, report)

            rendered = path.read_text(encoding="utf-8")
            self.assertIn("## adapter", rendered)
            self.assertIn("Explain ownership.", rendered)
            self.assertIn("Ownership controls drops.", rendered)


if __name__ == "__main__":
    unittest.main()
