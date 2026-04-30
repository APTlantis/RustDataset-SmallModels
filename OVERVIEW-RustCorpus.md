# Rust Corpus Forge — TinyLlama Rust Fine-Tuning Dataset Pipeline

## Goal

Build a Rust-based dataset generation pipeline that produces high-quality, small-model-friendly Rust programming datasets for fine-tuning `TinyLlama-1.1B-Chat-v1.0`.

This project is not intended to scrape massive raw code blindly. The goal is to produce a curated, structured, validated, reproducible corpus that teaches a small model Rust concepts, idioms, APIs, code repair, and code generation.

TinyLlama-1.1B-Chat-v1.0 uses chat-style message formatting through the tokenizer chat template, so the primary output should be JSONL entries containing `messages` arrays compatible with chat fine-tuning. The Hugging Face model card demonstrates using `tokenizer.apply_chat_template(...)` with `system`, `user`, and `assistant` roles.  
Reference: https://huggingface.co/TinyLlama/TinyLlama-1.1B-Chat-v1.0

## Project Name

`rust-corpus-forge`

## Primary Outputs

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
````

## Dataset Philosophy

TinyLlama is small, so quality and consistency matter more than volume.

Preferred strategy:

1. Teach core Rust concepts.
2. Teach idiomatic Rust patterns.
3. Teach API usage from structured documentation.
4. Add small validated code examples.
5. Add code repair/refactor examples only when they pass validation.

Avoid dumping large raw crates into the model. Prefer compact, focused examples.

## Recommended Corpus Layers

### 1. Concepts / Learning Text

Sources:

* Rust Book
* Rust By Example
* Cargo Book
* Rust Reference
* selected official docs

The official Rust language projects are generally MIT OR Apache-2.0 licensed.
Reference: [https://rust-lang.org/policies/licenses/](https://rust-lang.org/policies/licenses/)

The Rust Book repository is MIT OR Apache-2.0.
Reference: [https://github.com/rust-lang/book](https://github.com/rust-lang/book)

Output:

* `rust_concepts_sft.jsonl`

### 2. API QA

Sources:

* docs.rs rustdoc JSON
* local rustdoc JSON if generated from crates

docs.rs provides rustdoc JSON for programmatic inspection of crates and APIs.
Reference: [https://docs.rs/about/rustdoc-json](https://docs.rs/about/rustdoc-json)

Output:

* `rust_api_qa.jsonl`

### 3. Code Completion

Sources:

* curated crates.io source
* examples/
* tests/
* small modules
* simple standalone functions

Output:

* `rust_code_completion.jsonl`

### 4. Code Repair / Refactor

Sources:

* generated broken examples
* real compiler errors where available
* mutation of known-good examples

Only keep examples that:

* compile after repair
* pass `cargo check`
* optionally pass `cargo test`
* optionally pass `cargo clippy`

Output:

* `rust_code_repair.jsonl`

## JSONL Schema

Each line should be one JSON object.

### Core Required Shape

```json
{
  "id": "rust-concept-ownership-0001",
  "type": "concept_qa",
  "messages": [
    {
      "role": "system",
      "content": "You are a precise and idiomatic Rust programming assistant."
    },
    {
      "role": "user",
      "content": "Explain ownership in Rust with a simple example."
    },
    {
      "role": "assistant",
      "content": "Ownership is Rust's system for managing memory without a garbage collector..."
    }
  ],
  "metadata": {
    "language": "rust",
    "source": "rust-book",
    "license": "MIT OR Apache-2.0",
    "dataset": "aptlantis-rust-concepts-v1",
    "created_at": "2026-04-30T00:00:00Z",
    "difficulty": "beginner",
    "topics": ["ownership", "borrowing", "memory"],
    "quality_score": 0.95,
    "validated": true
  }
}
```

## Supported `type` Values

```text
concept_qa
api_qa
code_completion
code_generation
code_repair
refactor
explanation
```

## Standard System Prompts

Use a small fixed set of system prompts. Do not generate unique system prompts for every entry.

Recommended:

```text
You are a precise and idiomatic Rust programming assistant.
```

```text
You explain Rust concepts clearly with small correct examples.
```

```text
You fix Rust code errors and explain the correction briefly.
```

```text
You complete Rust code accurately and idiomatically.
```

## Formatting Rules

* Always use fenced code blocks.
* Always label Rust code blocks as `rust`.
* Keep examples small: usually 10–40 lines.
* Prefer concise explanations.
* Avoid long essays.
* Avoid huge production files.
* Avoid examples that depend on unknown external state.
* Preserve licensing/provenance metadata.

## Example: Concept QA

````json
{
  "id": "rust-concept-borrowing-0001",
  "type": "concept_qa",
  "messages": [
    {
      "role": "system",
      "content": "You explain Rust concepts clearly with small correct examples."
    },
    {
      "role": "user",
      "content": "What is borrowing in Rust?"
    },
    {
      "role": "assistant",
      "content": "Borrowing lets code use a value without taking ownership of it.\n\n```rust\nfn print_name(name: &String) {\n    println!(\"{}\", name);\n}\n\nfn main() {\n    let name = String::from(\"Ferris\");\n    print_name(&name);\n    println!(\"{}\", name);\n}\n```\n\nThe function receives `&String`, so it borrows `name` instead of moving it."
    }
  ],
  "metadata": {
    "language": "rust",
    "source": "rust-book",
    "license": "MIT OR Apache-2.0",
    "difficulty": "beginner",
    "topics": ["borrowing", "ownership"],
    "quality_score": 0.95,
    "validated": true
  }
}
````

## Example: API QA

````json
{
  "id": "rust-api-iterator-map-0001",
  "type": "api_qa",
  "messages": [
    {
      "role": "system",
      "content": "You are a precise and idiomatic Rust programming assistant."
    },
    {
      "role": "user",
      "content": "What does `Iterator::map` do in Rust?"
    },
    {
      "role": "assistant",
      "content": "`Iterator::map` transforms each item of an iterator using a closure.\n\n```rust\nfn main() {\n    let nums = vec![1, 2, 3];\n    let doubled: Vec<i32> = nums.iter().map(|n| n * 2).collect();\n    println!(\"{:?}\", doubled);\n}\n```\n\nThis prints `[2, 4, 6]`."
    }
  ],
  "metadata": {
    "language": "rust",
    "source": "rustdoc-json",
    "api_item": "Iterator::map",
    "topics": ["iterator", "map", "closure"],
    "quality_score": 0.93,
    "validated": true
  }
}
````

## Example: Code Repair

````json
{
  "id": "rust-repair-move-error-0001",
  "type": "code_repair",
  "messages": [
    {
      "role": "system",
      "content": "You fix Rust code errors and explain the correction briefly."
    },
    {
      "role": "user",
      "content": "Fix this Rust code:\n\n```rust\nfn main() {\n    let s = String::from(\"hello\");\n    let t = s;\n    println!(\"{}\", s);\n}\n```"
    },
    {
      "role": "assistant",
      "content": "The value `s` is moved into `t`, so `s` cannot be used afterward. Borrow or clone depending on the intent.\n\n```rust\nfn main() {\n    let s = String::from(\"hello\");\n    let t = s.clone();\n    println!(\"{}\", s);\n    println!(\"{}\", t);\n}\n```"
    }
  ],
  "metadata": {
    "language": "rust",
    "error_kind": "move_after_move",
    "topics": ["ownership", "move", "clone"],
    "cargo_check": true,
    "quality_score": 0.96,
    "validated": true
  }
}
````

## Rust CLI Design

Use `clap` subcommands.

```text
rust-corpus-forge ingest-mdbook --input ./sources/book --output work/book_chunks.jsonl
rust-corpus-forge ingest-rustdoc --input ./sources/rustdoc-json --output work/api_items.jsonl
rust-corpus-forge ingest-crates --input ./crates-mirror --output work/code_items.jsonl
rust-corpus-forge generate-sft --input work/book_chunks.jsonl --output out/rust_concepts_sft.jsonl
rust-corpus-forge generate-api-qa --input work/api_items.jsonl --output out/rust_api_qa.jsonl
rust-corpus-forge generate-repair --input work/code_items.jsonl --output out/rust_code_repair.jsonl
rust-corpus-forge validate --input out --report out/quality_report.json
rust-corpus-forge export-parquet --input out/*.jsonl --output out/rust_corpus.parquet
rust-corpus-forge manifest --input out --output out/corpus_manifest.toml
```

## Suggested Rust Crates

```toml
[dependencies]
anyhow = "1"
thiserror = "1"
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
rayon = "1"
walkdir = "2"
ignore = "0.4"
regex = "1"
chrono = { version = "0.4", features = ["serde"] }
blake3 = "1"
sha2 = "0.10"
sha3 = "0.10"
zstd = "0.13"
tar = "0.4"
pulldown-cmark = "0.10"
tree-sitter = "0.22"
tree-sitter-rust = "0.21"
uuid = { version = "1", features = ["v4", "serde"] }
```

Optional later:

```toml
polars = { version = "0.40", features = ["parquet", "json", "lazy"] }
```

## Proposed Module Layout

```text
src/
├── main.rs
├── cli.rs
├── schema.rs
├── ingest/
│   ├── mod.rs
│   ├── mdbook.rs
│   ├── rustdoc_json.rs
│   ├── crates_mirror.rs
│   └── git_repo.rs
├── generate/
│   ├── mod.rs
│   ├── concepts.rs
│   ├── api_qa.rs
│   ├── completion.rs
│   └── repair.rs
├── quality/
│   ├── mod.rs
│   ├── license.rs
│   ├── dedupe.rs
│   ├── heuristics.rs
│   ├── secrets.rs
│   └── cargo_validate.rs
├── export/
│   ├── mod.rs
│   ├── jsonl.rs
│   ├── parquet.rs
│   ├── manifest.rs
│   └── hashes.rs
└── util/
    ├── paths.rs
    ├── ids.rs
    └── text.rs
```

## Core Rust Structs

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub entry_type: EntryType,
    pub messages: Vec<Message>,
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryType {
    ConceptQa,
    ApiQa,
    CodeCompletion,
    CodeGeneration,
    CodeRepair,
    Refactor,
    Explanation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub language: String,
    pub source: String,
    pub license: Option<String>,
    pub dataset: Option<String>,
    pub created_at: Option<String>,
    pub difficulty: Option<Difficulty>,
    pub topics: Vec<String>,
    pub quality_score: f32,
    pub validated: bool,

    pub api_item: Option<String>,
    pub crate_name: Option<String>,
    pub crate_version: Option<String>,
    pub file_path: Option<String>,

    pub cargo_check: Option<bool>,
    pub cargo_test: Option<bool>,
    pub clippy_clean: Option<bool>,
    pub error_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Beginner,
    Intermediate,
    Advanced,
}
```

## Quality Filters

Reject items when:

* license is missing or disallowed
* file is generated/vendor/minified
* file is too large
* file has binary/invalid UTF-8 content
* file contains likely secrets
* item is near-duplicate
* code cannot be parsed
* code repair answer fails validation
* content includes huge dependency-specific boilerplate
* content is mostly comments or mostly imports

Prefer items when:

* source is official documentation
* examples are small and complete
* code is under `examples/`, `tests/`, or `src/`
* crate has clear license metadata
* code compiles
* example demonstrates one concept cleanly

## Validation Strategy

For code examples:

1. Extract Rust code blocks.
2. Wrap snippets into minimal Cargo project when needed.
3. Run `cargo check`.
4. Optionally run `cargo clippy`.
5. Optionally run `cargo test`.
6. Store results in metadata.

Validation metadata:

```json
{
  "cargo_check": true,
  "cargo_test": false,
  "clippy_clean": true,
  "validated": true
}
```

## Manifest Format

Generate `corpus_manifest.toml`.

```toml
schema = "aptlantis.rust_corpus.v1"

[dataset]
id = "aptlantis-rust-tinyllama-sft-v1"
name = "Aptlantis Rust TinyLlama SFT Corpus"
language = "rust"
target_model = "TinyLlama-1.1B-Chat-v1.0"
created_at = "2026-04-30T00:00:00Z"

[outputs]
concepts = "rust_concepts_sft.jsonl"
api_qa = "rust_api_qa.jsonl"
code_completion = "rust_code_completion.jsonl"
code_repair = "rust_code_repair.jsonl"
parquet = "rust_corpus.parquet"

[quality]
requires_license = true
dedupe = true
secret_scan = true
cargo_check_for_code = true

[license_policy]
allowed = [
  "MIT",
  "Apache-2.0",
  "MIT OR Apache-2.0",
  "BSD-2-Clause",
  "BSD-3-Clause"
]
```

## Milestones

### Milestone 1 — Schema + JSONL Writer

* Implement structs.
* Implement JSONL writer.
* Add sample manually generated entries.
* Validate each line parses.

### Milestone 2 — mdBook Ingest

* Parse Markdown files from Rust Book-style repositories.
* Split into chunks by heading.
* Preserve source path and heading.
* Generate concept QA drafts.

### Milestone 3 — rustdoc JSON Ingest

* Parse rustdoc JSON.
* Extract item names, docs, signatures, modules.
* Generate API QA drafts.

### Milestone 4 — Code Ingest

* Walk local crate/source trees.
* Extract small `.rs` files.
* Reject generated/vendor files.
* Generate completion/explanation candidates.

### Milestone 5 — Validation

* Extract code blocks.
* Create temp Cargo projects.
* Run `cargo check`.
* Store validation result.

### Milestone 6 — Quality Report

* Count entries by type.
* Count licenses.
* Count validation pass/fail.
* Count topics.
* Export `quality_report.json`.

### Milestone 7 — Parquet + Manifest + Hashes

* Export JSONL to Parquet.
* Generate manifest TOML.
* Generate hash manifest.
* Leave room for AAMHS v2.0 integration later.

## Non-Goals For v1

* Do not train the model inside this tool.
* Do not scrape the whole internet.
* Do not generate massive raw-code pretraining dumps.
* Do not support every programming language yet.
* Do not build a GUI yet.

## Success Criteria

v1 is successful when it can produce:

* at least 1,000 valid concept/chat examples
* at least 1,000 API QA examples
* at least 1,000 validated code examples
* clean JSONL compatible with TinyLlama chat fine-tuning
* manifest and quality report
* reproducible output from local sources

```

TinyLlama’s model card specifically shows chat-template usage with `system` and `user` messages, so the `messages` schema is the right core format. :contentReference[oaicite:0]{index=0}

The Rust licensing/source choice is also solid: official Rust projects are generally MIT/Apache-2.0, and the Rust Book repo itself carries both licenses.
