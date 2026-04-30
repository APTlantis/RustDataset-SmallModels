use std::path::PathBuf;

use clap::{ArgAction, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "rust-corpus-forge")]
#[command(about = "Build small-model-friendly Rust fine-tuning datasets.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the canonical dataset pipeline end to end.
    Pipeline {
        #[arg(long)]
        mdbook: Option<PathBuf>,
        #[arg(long)]
        rustdoc: Option<PathBuf>,
        #[arg(long)]
        crates: Option<PathBuf>,
        #[arg(long, default_value = "out")]
        out: PathBuf,
        #[arg(long, default_value = "work")]
        work: PathBuf,
        #[arg(long, action = ArgAction::SetTrue)]
        clean: bool,
        #[arg(long = "no-validate-code", action = ArgAction::SetFalse, default_value_t = true)]
        validate_code: bool,
    },
    /// Remove canonical generated outputs and work files.
    Clean {
        #[arg(long, default_value = "out")]
        out: PathBuf,
        #[arg(long, default_value = "work")]
        work: PathBuf,
    },
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
