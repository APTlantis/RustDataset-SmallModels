#!/usr/bin/env python
"""Summarize Trainer logs into a compact training report."""

from __future__ import annotations

import argparse
import ast
import json
import math
import re
from pathlib import Path
from typing import Any


DICT_PATTERN = re.compile(r"\{[^{}]*\}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Parse Rust Corpus Forge fine-tune logs.")
    parser.add_argument("--input", type=Path, required=True, help="Text log containing Trainer metric dictionaries.")
    parser.add_argument("--output", type=Path, required=True, help="JSON summary path to write.")
    parser.add_argument(
        "--turnaround-threshold",
        type=float,
        default=0.15,
        help="Relative worsening after the best point that should be flagged.",
    )
    return parser.parse_args()


def coerce_value(value: Any) -> Any:
    if not isinstance(value, str):
        return value
    text = value.strip()
    try:
        number = float(text)
    except ValueError:
        return value
    if math.isfinite(number):
        return number
    return value


def normalize_record(record: dict[str, Any]) -> dict[str, Any]:
    return {key: coerce_value(value) for key, value in record.items()}


def parse_dict_metric_records(text: str) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for match in DICT_PATTERN.finditer(text):
        try:
            value = ast.literal_eval(match.group(0))
        except (SyntaxError, ValueError):
            continue
        if isinstance(value, dict):
            records.append(normalize_record(value))
    return records


def parse_table_metric_records(text: str) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    header: list[str] | None = None

    for raw_line in text.splitlines():
        line = raw_line.strip()
        if not line:
            continue

        parts = re.split(r"\t+|\s{2,}", line)
        if {"loss", "epoch"}.issubset(set(parts)):
            header = parts
            continue
        if header is None or len(parts) != len(header):
            continue

        record = normalize_record(dict(zip(header, parts, strict=True)))
        if numeric(record, "loss") is not None or numeric(record, "eval_loss") is not None:
            records.append(record)

    return records


def parse_metric_records(text: str) -> list[dict[str, Any]]:
    records = parse_dict_metric_records(text)
    if records:
        return records
    return parse_table_metric_records(text)


def numeric(record: dict[str, Any], key: str) -> float | None:
    value = record.get(key)
    return value if isinstance(value, float | int) else None


def best_record(records: list[dict[str, Any]], key: str, *, highest: bool = False) -> dict[str, Any] | None:
    candidates = [record for record in records if numeric(record, key) is not None]
    if not candidates:
        return None
    return max(candidates, key=lambda record: numeric(record, key) or 0.0) if highest else min(
        candidates, key=lambda record: numeric(record, key) or 0.0
    )


def rolling_best_loss(records: list[dict[str, Any]], window: int = 3) -> dict[str, Any] | None:
    loss_records = [record for record in records if numeric(record, "loss") is not None]
    if len(loss_records) < window:
        return best_record(loss_records, "loss")

    best: dict[str, Any] | None = None
    best_average: float | None = None
    for index in range(0, len(loss_records) - window + 1):
        group = loss_records[index : index + window]
        average = sum(numeric(record, "loss") or 0.0 for record in group) / window
        if best_average is None or average < best_average:
            middle = group[window // 2]
            best_average = average
            best = {
                "epoch": middle.get("epoch"),
                "step_index": index + window // 2 + 1,
                "window": window,
                "average_loss": round(average, 6),
                "losses": [record.get("loss") for record in group],
            }
    return best


def compact_record(record: dict[str, Any] | None, keys: list[str]) -> dict[str, Any] | None:
    if record is None:
        return None
    return {key: record[key] for key in keys if key in record}


def relative_worse(current: float | None, best: float | None, threshold: float) -> bool:
    if current is None or best is None:
        return False
    if best == 0.0:
        return current > threshold
    return current > best * (1.0 + threshold)


def build_summary(source: Path, records: list[dict[str, Any]], turnaround_threshold: float) -> dict[str, Any]:
    train_records = [record for record in records if "loss" in record]
    eval_records = [record for record in records if "eval_loss" in record]

    first_train = train_records[0] if train_records else None
    last_train = train_records[-1] if train_records else None
    first_eval = eval_records[0] if eval_records else None
    last_eval = eval_records[-1] if eval_records else None

    best_train_loss = best_record(train_records, "loss")
    best_train_accuracy = best_record(train_records, "mean_token_accuracy", highest=True)
    best_eval_loss = best_record(eval_records, "eval_loss")
    best_eval_accuracy = best_record(eval_records, "eval_mean_token_accuracy", highest=True)
    rolling_loss = rolling_best_loss(train_records)

    best_loss_value = numeric(best_train_loss or {}, "loss")
    last_loss_value = numeric(last_train or {}, "loss")
    best_eval_value = numeric(best_eval_loss or {}, "eval_loss")
    last_eval_value = numeric(last_eval or {}, "eval_loss")

    train_turnaround = relative_worse(last_loss_value, best_loss_value, turnaround_threshold)
    eval_turnaround = relative_worse(last_eval_value, best_eval_value, turnaround_threshold)

    suggested_stop_epoch = None
    stop_basis = None
    if eval_turnaround and best_eval_loss is not None:
        suggested_stop_epoch = best_eval_loss.get("epoch")
        stop_basis = "best_eval_loss"
    elif train_turnaround and rolling_loss is not None:
        suggested_stop_epoch = rolling_loss.get("epoch")
        stop_basis = "best_rolling_train_loss"

    total_tokens = None
    if last_eval is not None:
        total_tokens = last_eval.get("eval_num_tokens")
    if total_tokens is None and last_train is not None:
        total_tokens = last_train.get("num_tokens")

    summary: dict[str, Any] = {
        "source_log": str(source),
        "record_count": len(records),
        "train_record_count": len(train_records),
        "eval_record_count": len(eval_records),
        "total_tokens": total_tokens,
        "last_epoch": (last_eval or last_train or {}).get("epoch"),
        "train": {
            "first": compact_record(first_train, ["epoch", "loss", "mean_token_accuracy", "learning_rate", "num_tokens"]),
            "last": compact_record(last_train, ["epoch", "loss", "mean_token_accuracy", "learning_rate", "num_tokens"]),
            "best_loss": compact_record(best_train_loss, ["epoch", "loss", "mean_token_accuracy", "learning_rate", "num_tokens"]),
            "best_accuracy": compact_record(
                best_train_accuracy, ["epoch", "loss", "mean_token_accuracy", "learning_rate", "num_tokens"]
            ),
            "best_rolling_loss": rolling_loss,
        },
        "eval": {
            "first": compact_record(first_eval, ["epoch", "eval_loss", "eval_mean_token_accuracy", "eval_num_tokens"]),
            "last": compact_record(last_eval, ["epoch", "eval_loss", "eval_mean_token_accuracy", "eval_num_tokens"]),
            "best_loss": compact_record(best_eval_loss, ["epoch", "eval_loss", "eval_mean_token_accuracy", "eval_num_tokens"]),
            "best_accuracy": compact_record(
                best_eval_accuracy, ["epoch", "eval_loss", "eval_mean_token_accuracy", "eval_num_tokens"]
            ),
        },
        "overfit_hints": {
            "turnaround_threshold": turnaround_threshold,
            "train_loss_worse_than_best": train_turnaround,
            "eval_loss_worse_than_best": eval_turnaround,
            "suggested_stop_epoch": suggested_stop_epoch,
            "stop_basis": stop_basis,
        },
        "notes": [],
    }

    if train_turnaround:
        summary["notes"].append(
            "Training loss worsened materially after its best point; prefer adding data before extending epochs."
        )
    if eval_turnaround:
        summary["notes"].append("Evaluation loss worsened after its best checkpoint; use the best eval checkpoint.")
    if eval_records and not eval_turnaround:
        summary["notes"].append("Evaluation loss did not show a later worsening at the configured threshold.")
    if not eval_records:
        summary["notes"].append("No evaluation records were found; overfit detection is based on training metrics only.")

    return summary


def main() -> None:
    args = parse_args()
    text = args.input.read_text(encoding="utf-8", errors="replace")
    records = parse_metric_records(text)
    summary = build_summary(args.input, records, args.turnaround_threshold)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(f"parsed {len(records)} metric records -> {args.output}")


if __name__ == "__main__":
    main()
