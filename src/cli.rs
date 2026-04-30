use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "rust-corpus-forge")]
#[command(about = "Build small-model-friendly Rust fine-tuning datasets.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Split Rust Book-style Markdown into source chunks.
    IngestMdbook {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Extract documented API items from rustdoc JSON.
    IngestRustdoc {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Extract small Rust code items from local crates or source trees.
    IngestCrates {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Generate deterministic hand-authored sample JSONL outputs.
    GenerateSamples {
        #[arg(long)]
        output: PathBuf,
    },
    /// Generate concept SFT JSONL from mdBook chunks.
    GenerateSft {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Generate API QA JSONL from ingested rustdoc API items.
    GenerateApiQa {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Generate code completion JSONL from ingested Rust code items.
    GenerateCompletion {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Validate JSONL outputs and write a quality report.
    Validate {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        report: PathBuf,
    },
    /// Run cargo check on assistant Rust code blocks and write updated JSONL.
    ValidateCode {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value = "work/cargo_validate")]
        work_dir: PathBuf,
    },
    /// Write a corpus manifest TOML file.
    Manifest {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Write BLAKE3 hashes for generated output files.
    Hashes {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}
