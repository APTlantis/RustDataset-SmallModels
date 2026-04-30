use anyhow::Result;
use clap::Parser;
use rust_corpus_forge::cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::IngestMdbook { input, output } => {
            rust_corpus_forge::ingest::mdbook::ingest_mdbook(&input, &output)?;
        }
        Command::GenerateSamples { output } => {
            rust_corpus_forge::generate::samples::generate_samples(&output)?;
        }
        Command::GenerateSft { input, output } => {
            rust_corpus_forge::generate::concepts::generate_sft_from_chunks(&input, &output)?;
        }
        Command::Validate { input, report } => {
            rust_corpus_forge::quality::report::validate_to_report(&input, &report)?;
        }
        Command::Manifest { input, output } => {
            rust_corpus_forge::export::manifest::write_manifest(&input, &output)?;
        }
        Command::Hashes { input, output } => {
            rust_corpus_forge::export::hashes::write_hashes(&input, &output)?;
        }
    }

    Ok(())
}
