# Rust Corpus Forge

`rust-corpus-forge` is a Rust CLI for building small, structured Rust programming datasets for chat fine-tuning. It favors compact, traceable examples over large raw-code dumps.

The current pipeline can:

- generate deterministic sample entries
- ingest mdBook-style Markdown into concept chunks
- ingest rustdoc JSON into API items
- ingest local Rust source trees into code completion candidates
- validate JSONL schema and Rust code fences
- run `cargo check` on assistant-side Rust code blocks
- write a manifest, quality report, and BLAKE3 hash snapshot

## Quick Start

Run the fixture pipeline:

```powershell
cargo run -- pipeline `
  --mdbook README.md `
  --rustdoc fixtures/rustdoc_sample.json `
  --crates fixtures/sample_crate `
  --out out `
  --work work `
  --clean
```

Run verification:

```powershell
cargo fmt --check
cargo test
cargo clippy -- -D warnings
```

## Canonical Outputs

The canonical `out/` directory contains:

```text
out/
├── rust_concepts_sft.jsonl
├── rust_api_qa.jsonl
├── rust_code_completion.jsonl
├── rust_code_repair.jsonl
├── corpus_manifest.toml
├── quality_report.json
└── snapshot-hashes.txt
```

Intermediate files are written to `work/`:

```text
work/
├── book_chunks.jsonl
├── api_items.jsonl
└── code_items.jsonl
```

Temporary Cargo validation projects are created under `work/cargo_validate` and removed by the pipeline after validation.

## Commands

`pipeline` runs the canonical flow end to end:

```powershell
cargo run -- pipeline --mdbook <book-or-md> --rustdoc <rustdoc-json-or-dir> --crates <crate-or-dir> --out out --work work --clean
```

Useful flags:

- `--clean` removes canonical and legacy generated artifacts before running.
- `--no-validate-code` skips Cargo validation for generated completion entries.
- Any source input can be omitted; omitted lanes fall back to deterministic sample output where applicable.

Individual commands are also available:

```powershell
cargo run -- generate-samples --output out
cargo run -- ingest-mdbook --input README.md --output work/book_chunks.jsonl
cargo run -- generate-sft --input work/book_chunks.jsonl --output out/rust_concepts_sft.jsonl
cargo run -- ingest-rustdoc --input fixtures/rustdoc_sample.json --output work/api_items.jsonl
cargo run -- generate-api-qa --input work/api_items.jsonl --output out/rust_api_qa.jsonl
cargo run -- ingest-crates --input fixtures/sample_crate --output work/code_items.jsonl
cargo run -- generate-completion --input work/code_items.jsonl --output out/rust_code_completion.jsonl
cargo run -- validate-code --input out/rust_code_completion.jsonl --output out/rust_code_completion.jsonl
cargo run -- validate --input out --report out/quality_report.json
cargo run -- manifest --input out --output out/corpus_manifest.toml
cargo run -- hashes --input out --output out/snapshot-hashes.txt
```

## Validation Semantics

There are two validation layers:

- `quality_report.json` reports whether entries satisfy the dataset schema and formatting rules.
- `metadata.validated` means an entry has passed the strongest validation currently available for that entry.

For code entries:

- `metadata.cargo_check = true` means every assistant Rust code block passed `cargo check` in a generated temporary Cargo project.
- `metadata.cargo_check = false` means Cargo validation was attempted but failed or no assistant Rust block was available.
- `metadata.cargo_check = null` means Cargo validation has not been run for that entry.

Some crate-derived snippets can fail standalone validation because they depend on original crate context. This is expected until crate-context validation is added.

## Manifest And Report

`corpus_manifest.toml` is generated from actual JSONL outputs. It records:

- generated timestamp
- total entry counts
- validation counts
- per-output byte size
- per-output entry count
- per-output BLAKE3 hash
- per-output Cargo-check counts

`quality_report.json` records:

- counts by file
- counts by dataset entry type
- counts by validation status
- counts by Cargo-check result
- counts by topic and license
- schema/formatting errors

## Suggested Source Layout

For real corpus runs, use explicit local source folders:

```text
sources/
├── book/
├── rust-by-example/
├── rustdoc-json/
└── crates/
```

Example:

```powershell
cargo run -- pipeline `
  --mdbook sources/book `
  --rustdoc sources/rustdoc-json `
  --crates sources/crates `
  --out out `
  --work work `
  --clean
```

## Current Limits

- Parquet export is not implemented yet.
- Code repair generation is still sample-only.
- Cargo validation currently validates snippets in temporary standalone crates, not inside the original source crate.
- The corpus is designed for curated fine-tuning data, not broad raw-code pretraining.
