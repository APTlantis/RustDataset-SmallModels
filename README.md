# Rust Corpus Forge

`rust-corpus-forge` is a Rust CLI for building small, structured Rust programming datasets for chat fine-tuning. It favors compact, traceable examples over large raw-code dumps.

The current pipeline can:

- generate deterministic sample entries
- ingest mdBook-style Markdown into concept chunks
- ingest rustdoc JSON into API items
- ingest local Rust source trees into code completion candidates
- generate simple code repair examples from local Rust code items
- validate JSONL schema and Rust code fences
- run `cargo check` on assistant-side Rust code blocks
- export a combined Parquet dataset
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
├── rust_corpus.parquet
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
cargo run -- generate-repair --input work/code_items.jsonl --output out/rust_code_repair.jsonl
cargo run -- validate-code --input out/rust_code_completion.jsonl --output out/rust_code_completion.jsonl
cargo run -- validate --input out --report out/quality_report.json
cargo run -- export-parquet --input out --output out/rust_corpus.parquet
cargo run -- manifest --input out --output out/corpus_manifest.toml
cargo run -- hashes --input out --output out/snapshot-hashes.txt
```

## CPU LoRA Training

The `training/` folder contains a TinyLlama LoRA training harness adapted from the known-good HolyC training workflow.

Prepare train/validation JSONL with a `text` field:

```powershell
python training/prepare_rust_train.py --config training/rust_cpu_smoke.toml
```

Run a trainer dry check without loading the model:

```powershell
python training/finetune_rust.py --config training/rust_cpu_smoke.toml --dry-run
```

Run a tiny CPU smoke fine-tune:

```powershell
python training/finetune_rust.py --config training/rust_cpu_smoke.toml --max-steps 5
```

The default smoke config writes prepared data to `training/data/rust-smoke/` and LoRA adapters to `models/rust-tinyllama-lora-smoke/`. CPU training is intentionally configured with a small batch size and short default step count. Scale `max_steps`, corpus size, and epochs only after the smoke run is healthy.

## Validation Semantics

There are two validation layers:

- `quality_report.json` reports whether entries satisfy the dataset schema and formatting rules.
- `metadata.validated` means an entry has passed the strongest validation currently available for that entry.

For code entries:

- `metadata.cargo_check = true` means every assistant Rust code block passed `cargo check` in a generated temporary Cargo project.
- `metadata.cargo_check = false` means Cargo validation was attempted but failed or no assistant Rust block was available.
- `metadata.cargo_check = null` means Cargo validation has not been run for that entry.

Some crate-derived snippets can fail standalone validation because they depend on original crate context. This is expected until crate-context validation is added.
For crate-derived entries, the pipeline records each code item's source crate root when it can find a nearby `Cargo.toml`. Cargo validation checks that source crate with an isolated target directory and can mark generated completion and repair entries as validated from that context. Standalone snippet validation remains the fallback.

## Manifest And Report

`corpus_manifest.toml` is generated from actual JSONL outputs. It records:

- generated timestamp
- total entry counts
- validation counts
- per-output byte size
- per-output entry count
- per-output BLAKE3 hash
- per-output Cargo-check counts
It also records `rust_corpus.parquet` when the Parquet export exists.

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

- Code repair generation currently uses simple deterministic mutations, starting with missing closing braces.
- Crate-context validation depends on finding a local `Cargo.toml`; snippets outside a Cargo package still use standalone validation.
- The corpus is designed for curated fine-tuning data, not broad raw-code pretraining.
