# MVP Scaffold Plan: `rust-corpus-forge`

## Summary
Create a new Rust CLI crate in `D:\Training\RustForSmallModels` that implements the first usable slice of the dataset pipeline: typed chat-entry schema, JSONL writing/reading validation, deterministic sample generation, manifest output, and a basic quality report. The MVP will not ingest external sources yet; it will prove the output contract and command structure for later milestones.

## Key Changes
- Initialize a Cargo binary project named `rust-corpus-forge`.
- Add CLI subcommands with `clap`:
  - `generate-samples --output out`
  - `validate --input out --report out/quality_report.json`
  - `manifest --input out --output out/corpus_manifest.toml`
  - `hashes --input out --output out/snapshot-hashes.txt`
- Implement the core schema from `OVERVIEW-RustCorpus.md`:
  - `DatasetEntry`, `Message`, `Metadata`
  - `EntryType`, `Role`, `Difficulty`
  - Serde naming rules exactly matching the JSON examples.
- Write JSONL outputs:
  - `rust_concepts_sft.jsonl`
  - `rust_api_qa.jsonl`
  - `rust_code_completion.jsonl`
  - `rust_code_repair.jsonl`
- Generate a small deterministic sample set, at least 2 entries per file, using fixed IDs and fixed metadata timestamps.
- Validate JSONL by checking:
  - every line parses as `DatasetEntry`
  - each entry has at least one `system`, `user`, and `assistant` message
  - `metadata.language == "rust"`
  - `quality_score` is between `0.0` and `1.0`
  - code-containing assistant/user content uses fenced `rust` blocks
- Generate `quality_report.json` with counts by file, entry type, validation status, topic, and license.
- Generate `corpus_manifest.toml` using the v1 manifest shape from the overview.
- Generate `snapshot-hashes.txt` with BLAKE3 hashes for generated output files.

## Implementation Notes
- Use this initial module layout:
  - `src/main.rs` for entrypoint
  - `src/cli.rs` for command definitions
  - `src/schema.rs` for dataset structs/enums
  - `src/export/jsonl.rs` for JSONL writer/reader helpers
  - `src/export/manifest.rs` for TOML manifest generation
  - `src/export/hashes.rs` for snapshot hashes
  - `src/generate/samples.rs` for deterministic sample entries
  - `src/quality/report.rs` for validation and reporting
- Use dependencies:
  - `anyhow`, `clap`, `serde`, `serde_json`, `toml`, `chrono`, `blake3`, `walkdir`
- Keep generated sample content compact and hand-authored, not LLM-generated at runtime.
- Keep output deterministic so repeated runs produce stable files and stable hashes, except when output content intentionally changes.

## Test Plan
- Add unit tests for schema serialization:
  - `EntryType::ConceptQa` serializes as `concept_qa`
  - `Role::Assistant` serializes as `assistant`
  - `Difficulty::Beginner` serializes as `beginner`
- Add JSONL tests:
  - writing entries produces one valid JSON object per line
  - reading invalid JSONL returns a validation error
- Add quality tests:
  - valid sample entries pass
  - missing assistant message fails
  - invalid quality score fails
  - unlabeled Rust code fence fails when Rust code is present
- Verify manually with:
  - `cargo test`
  - `cargo run -- generate-samples --output out`
  - `cargo run -- validate --input out --report out/quality_report.json`
  - `cargo run -- manifest --input out --output out/corpus_manifest.toml`
  - `cargo run -- hashes --input out --output out/snapshot-hashes.txt`

## Assumptions
- The MVP should create a new Rust project because the workspace currently contains only `OVERVIEW-RustCorpus.md`.
- Rust toolchain is available locally: `cargo 1.94.1` and `rustc 1.94.1`.
- Parquet export, mdBook ingest, rustdoc ingest, crate walking, and Cargo snippet validation are deferred to later milestones.
- The first implementation prioritizes stable schema and reproducible outputs over large dataset volume.
