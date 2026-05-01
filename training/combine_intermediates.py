#!/usr/bin/env python
"""Combine and reindex Rust Corpus Forge intermediate JSONL files."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


PREFIX_BY_KIND = {
    "mdbook": "mdbook-chunk",
    "code": "code-item",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Combine intermediate JSONL files with stable IDs.")
    parser.add_argument("--kind", choices=sorted(PREFIX_BY_KIND), required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("inputs", type=Path, nargs="+")
    return parser.parse_args()


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            item = json.loads(line)
            item["_combine_source"] = path.name
            item["_combine_line"] = line_no
            rows.append(item)
    return rows


def sort_key(item: dict[str, Any]) -> tuple[str, str, str, str]:
    return (
        str(item.get("source_path", "")),
        str(item.get("heading", "")),
        str(item.get("item_kind", "")),
        str(item.get("code", item.get("content", ""))),
    )


def write_jsonl(path: Path, rows: list[dict[str, Any]], prefix: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    rows = sorted(rows, key=sort_key)
    with path.open("w", encoding="utf-8", newline="\n") as handle:
        for index, row in enumerate(rows):
            row = dict(row)
            row["id"] = f"{prefix}-{index:06}"
            row.pop("_combine_source", None)
            row.pop("_combine_line", None)
            handle.write(json.dumps(row, ensure_ascii=False) + "\n")


def main() -> int:
    args = parse_args()
    rows: list[dict[str, Any]] = []
    for path in args.inputs:
        if path.exists():
            rows.extend(read_jsonl(path))
    if not rows:
        raise ValueError("No rows found in input files")
    write_jsonl(args.output, rows, PREFIX_BY_KIND[args.kind])
    print(json.dumps({"kind": args.kind, "output": str(args.output), "rows": len(rows)}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
