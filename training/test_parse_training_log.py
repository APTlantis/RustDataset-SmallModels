#!/usr/bin/env python
"""Tests for training log parsing."""

from __future__ import annotations

import unittest
from pathlib import Path

from parse_training_log import build_summary, parse_metric_records


class TrainingLogParserTests(unittest.TestCase):
    def test_dict_log_with_control_sequences_parses_metric_records(self) -> None:
        text = (
            "\x1b[Apython : {'pad_token_id': 2}\n"
            "\x1b[31m{'loss': '2.44', 'num_tokens': '511', 'mean_token_accuracy': '0.6038', 'epoch': '0.2105'}\x1b[0m\n"
            "100%|##########| {'eval_loss': '1.141', 'eval_num_tokens': '5298', "
            "'eval_mean_token_accuracy': '0.7698', 'epoch': '2'}\n"
            "{'train_runtime': '150.3', 'train_steps_per_second': '0.133', 'train_loss': '1.12', 'epoch': '4'}\n"
        )

        records = parse_metric_records(text)

        self.assertEqual(3, len(records))
        self.assertEqual(2.44, records[0]["loss"])
        self.assertEqual(1.141, records[1]["eval_loss"])
        self.assertEqual(150.3, records[2]["train_runtime"])

    def test_tsv_holyc_style_log_parses(self) -> None:
        text = (
            "percentage\tstep\ttotal_steps\tloss\tgrad_norm\tlearning_rate\tentropy\tnum_tokens\t"
            "mean_token_accuracy\tepoch\n"
            "1\t10\t1915\t1.4824\t0.451\t3.1e-05\t1.32\t81920.0\t0.6967\t0.03\n"
            "2\t20\t1915\t1.2781\t0.392\t6.5e-05\t1.22\t163840.0\t0.7306\t0.05\n"
        )

        records = parse_metric_records(text)

        self.assertEqual(2, len(records))
        self.assertEqual(1.2781, records[1]["loss"])
        self.assertEqual(0.05, records[1]["epoch"])

    def test_overfit_hint_flags_final_loss_rebound(self) -> None:
        text = (
            "{'loss': '0.5642', 'num_tokens': '6486', 'mean_token_accuracy': '0.8662', 'epoch': '2.421'}\n"
            "{'loss': '0.4043', 'num_tokens': '9030', 'mean_token_accuracy': '0.9112', 'epoch': '3.421'}\n"
            "{'loss': '0.3955', 'num_tokens': '9579', 'mean_token_accuracy': '0.9175', 'epoch': '3.632'}\n"
            "{'loss': '0.9481', 'num_tokens': '1.06e+04', 'mean_token_accuracy': '0.8173', 'epoch': '4'}\n"
            "{'eval_loss': '0.9419', 'eval_num_tokens': '1.06e+04', 'eval_mean_token_accuracy': '0.8118', 'epoch': '4'}\n"
        )

        summary = build_summary(Path("sample.log"), parse_metric_records(text), 0.15)

        self.assertTrue(summary["overfit_hints"]["train_loss_worse_than_best"])
        self.assertGreaterEqual(summary["overfit_hints"]["train_curve_turn_epoch"], 3.4)
        self.assertLessEqual(summary["overfit_hints"]["train_curve_turn_epoch"], 3.7)
        self.assertGreater(summary["train"]["loss_degradation_from_best"], 1.0)

    def test_existing_rust_smoke_log_parses_when_present(self) -> None:
        log_path = Path("training/reports/rust-smoke-20.log")
        if not log_path.exists():
            self.skipTest(f"{log_path} is not present")

        records = parse_metric_records(log_path.read_text(encoding="utf-8", errors="replace"))
        summary = build_summary(log_path, records, 0.15)

        self.assertEqual(23, summary["record_count"])
        self.assertEqual(20, summary["train_record_count"])
        self.assertEqual(2, summary["eval_record_count"])
        self.assertEqual(1, summary["final_summary_record_count"])
        self.assertTrue(summary["overfit_hints"]["train_loss_worse_than_best"])


if __name__ == "__main__":
    unittest.main()
