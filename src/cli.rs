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
    /// Generate deterministic hand-authored sample JSONL outputs.
    GenerateSamples {
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
