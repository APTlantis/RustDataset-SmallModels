#!/usr/bin/env python
"""Prepare Rust Corpus Forge chat JSONL for TRL SFT training."""

from __future__ import annotations

import argparse
import json
import random
import sys
import tomllib
from collections import Counter
from pathlib import Path
from typing import Any


DEFAULT_DATASET_FILES = [
    "rust_concepts_sft.jsonl",
    "rust_api_qa.jsonl",
    "rust_code_completion.jsonl",
    "rust_code_repair.jsonl",
]


def load_config(path: Path | None) -> dict[str, Any]:
    if path is None:
        return {}
    with path.open("rb") as handle:
        return tomllib.load(handle)


def config_get(config: dict[str, Any], section: str, key: str, default: Any) -> Any:
    return config.get(section, {}).get(key, default)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Prepare Rust chat JSONL for TinyLlama SFT.")
    parser.add_argument("--config", type=Path, default=Path("training/rust_cpu_smoke.toml"))
    parser.add_argument("--input-dir", type=Path)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--quality-report", type=Path)
    parser.add_argument("--model-name", type=str)
    parser.add_argument("--validation-ratio", type=float)
    parser.add_argument("--max-examples", type=int)
    parser.add_argument("--seed", type=int)
    parser.add_argument(
        "--no-chat-template",
        action="store_true",
        help="Use a simple manual chat rendering instead of tokenizer.apply_chat_template.",
    )
    return parser.parse_args()


def configured_dataset_files(config: dict[str, Any]) -> list[str] | None:
    files = config.get("data", {}).get("dataset_files")
    if files is None:
        return None
    if not isinstance(files, list) or not all(isinstance(name, str) for name in files):
        raise ValueError("data.dataset_files must be a list of file names")
    return files


def dataset_files(input_dir: Path, configured_files: list[str] | None = None) -> list[Path]:
    if configured_files is not None:
        return [input_dir / name for name in configured_files]

    discovered = sorted(input_dir.glob("rust_*.jsonl"))
    if discovered:
        return discovered
    return [input_dir / name for name in DEFAULT_DATASET_FILES]


def read_entries(input_dir: Path, configured_files: list[str] | None = None) -> list[dict[str, Any]]:
    entries: list[dict[str, Any]] = []
    for path in dataset_files(input_dir, configured_files):
        if not path.exists():
            continue
        with path.open("r", encoding="utf-8") as handle:
            for line_no, line in enumerate(handle, start=1):
                if not line.strip():
                    continue
                item = json.loads(line)
                if "messages" not in item:
                    raise ValueError(f"{path}:{line_no} is missing messages")
                item["_source_file"] = path.name
                entries.append(item)
    return entries


def invalid_entry_ids(report_path: Path | None) -> set[tuple[str, str]]:
    if report_path is None or not report_path.exists():
        return set()
    report = json.loads(report_path.read_text(encoding="utf-8"))
    invalid: set[tuple[str, str]] = set()
    for error in report.get("errors", []):
        file_name = error.get("file")
        entry_id = error.get("id")
        if isinstance(file_name, str) and isinstance(entry_id, str):
            invalid.add((file_name, entry_id))
    return invalid


def filter_invalid_entries(entries: list[dict[str, Any]], invalid_ids: set[tuple[str, str]]) -> list[dict[str, Any]]:
    if not invalid_ids:
        return entries
    return [
        entry
        for entry in entries
        if (str(entry.get("_source_file", "")), str(entry.get("id", ""))) not in invalid_ids
    ]


def sample_entries(entries: list[dict[str, Any]], max_examples: int | None, seed: int) -> list[dict[str, Any]]:
    if not max_examples or len(entries) <= max_examples:
        return entries

    grouped: dict[str, list[dict[str, Any]]] = {}
    for entry in entries:
        key = str(entry.get("type", "unknown"))
        grouped.setdefault(key, []).append(entry)

    rng = random.Random(seed)
    for key, rows in grouped.items():
        if key == "code_repair":
            rows[:] = repair_balanced_order(rows, rng)
        else:
            rng.shuffle(rows)

    sampled: list[dict[str, Any]] = []
    keys = sorted(grouped)
    while len(sampled) < max_examples and keys:
        next_keys = []
        for key in keys:
            rows = grouped[key]
            if rows and len(sampled) < max_examples:
                sampled.append(rows.pop())
            if rows:
                next_keys.append(key)
        keys = next_keys

    sampled.sort(key=lambda entry: str(entry.get("id", "")))
    return sampled


def repair_error_kind(entry: dict[str, Any]) -> str:
    metadata = entry.get("metadata")
    if isinstance(metadata, dict):
        error_kind = metadata.get("error_kind")
        if isinstance(error_kind, str) and error_kind:
            return error_kind
    return "unknown"


def repair_balanced_order(entries: list[dict[str, Any]], rng: random.Random) -> list[dict[str, Any]]:
    """Return repair entries in a deterministic round-robin by error kind.

    sample_entries pops from the end of each type bucket, so this returns the
    reverse of the desired draw order.
    """

    grouped: dict[str, list[dict[str, Any]]] = {}
    for entry in entries:
        grouped.setdefault(repair_error_kind(entry), []).append(entry)

    for rows in grouped.values():
        rng.shuffle(rows)

    draw_order: list[dict[str, Any]] = []
    keys = sorted(grouped)
    while keys:
        next_keys = []
        for key in keys:
            rows = grouped[key]
            if rows:
                draw_order.append(rows.pop())
            if rows:
                next_keys.append(key)
        keys = next_keys

    return list(reversed(draw_order))


def count_types(entries: list[dict[str, Any]]) -> dict[str, int]:
    return dict(sorted(Counter(str(entry.get("type", "unknown")) for entry in entries).items()))


def count_repair_error_kinds(entries: list[dict[str, Any]]) -> dict[str, int]:
    return dict(
        sorted(
            Counter(
                repair_error_kind(entry)
                for entry in entries
                if str(entry.get("type", "unknown")) == "code_repair"
            ).items()
        )
    )


def load_tokenizer(model_name: str):
    from transformers import AutoTokenizer

    tokenizer = AutoTokenizer.from_pretrained(model_name, trust_remote_code=True)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
    return tokenizer


def render_manual(messages: list[dict[str, str]]) -> str:
    parts: list[str] = []
    for message in messages:
        role = message["role"].strip()
        content = message["content"].strip()
        parts.append(f"<|{role}|>\n{content}")
    parts.append("<|end|>")
    return "\n".join(parts)


def render_entry(entry: dict[str, Any], tokenizer: Any | None) -> dict[str, str]:
    messages = entry["messages"]
    if tokenizer is None:
        text = render_manual(messages)
    else:
        text = tokenizer.apply_chat_template(
            messages,
            tokenize=False,
            add_generation_prompt=False,
        )
    return {"text": text, "id": entry.get("id", "")}


def write_jsonl(path: Path, rows: list[dict[str, str]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="\n") as handle:
        for row in rows:
            handle.write(json.dumps(row, ensure_ascii=False) + "\n")


def main() -> int:
    args = parse_args()
    config = load_config(args.config if args.config.exists() else None)

    input_dir = args.input_dir or Path(config_get(config, "data", "input_dir", "out"))
    output_dir = args.output_dir or Path(
        config_get(config, "data", "prepared_dir", "training/data/rust-smoke")
    )
    quality_report = args.quality_report
    if quality_report is None:
        quality_report = input_dir / "quality_report.json"
    model_name = args.model_name or config_get(
        config, "model", "name", "TinyLlama/TinyLlama-1.1B-Chat-v1.0"
    )
    validation_ratio = args.validation_ratio
    if validation_ratio is None:
        validation_ratio = float(config_get(config, "data", "validation_ratio", 0.15))
    max_examples = args.max_examples
    if max_examples is None:
        max_examples = config_get(config, "data", "max_examples", None)
    seed = args.seed
    if seed is None:
        seed = int(config_get(config, "training", "seed", 42))

    entries = read_entries(input_dir, configured_dataset_files(config))
    invalid_ids = invalid_entry_ids(quality_report)
    before_filter = len(entries)
    entries = filter_invalid_entries(entries, invalid_ids)
    filtered_invalid = before_filter - len(entries)
    valid_available = len(entries)
    entries = sample_entries(entries, int(max_examples) if max_examples else None, seed)
    capped_examples = valid_available - len(entries)
    if not entries:
        raise ValueError(f"No dataset entries found in {input_dir}")

    tokenizer = None if args.no_chat_template else load_tokenizer(model_name)
    rows = [render_entry(entry, tokenizer) for entry in entries]

    rng = random.Random(seed)
    rng.shuffle(rows)
    validation_count = max(1, int(len(rows) * validation_ratio)) if len(rows) > 1 else 0
    validation_rows = rows[:validation_count]
    train_rows = rows[validation_count:]
    if not train_rows:
        train_rows, validation_rows = rows, []

    write_jsonl(output_dir / "train.jsonl", train_rows)
    write_jsonl(output_dir / "validation.jsonl", validation_rows)
    print(
        json.dumps(
            {
                "input_dir": str(input_dir),
                "output_dir": str(output_dir),
                "train": len(train_rows),
                "validation": len(validation_rows),
                "chat_template": tokenizer is not None,
                "input_examples": len(rows),
                "valid_available": valid_available,
                "filtered_invalid": filtered_invalid,
                "capped_examples": capped_examples,
                "counts_by_type": count_types(entries),
                "code_repair_error_kinds": count_repair_error_kinds(entries),
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
