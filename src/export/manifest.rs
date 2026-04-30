use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::Serialize;
use walkdir::WalkDir;

use crate::{export::jsonl::read_jsonl, quality::report::validate_input};

#[derive(Debug, Serialize)]
struct Manifest {
    schema: String,
    dataset: DatasetSection,
    summary: SummarySection,
    outputs: BTreeMap<String, OutputFile>,
    quality: QualitySection,
    license_policy: LicensePolicy,
}

#[derive(Debug, Serialize)]
struct DatasetSection {
    id: String,
    name: String,
    language: String,
    target_model: String,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct SummarySection {
    generated_at: String,
    total_entries: usize,
    schema_valid_entries: usize,
    schema_invalid_entries: usize,
    validated_entries: usize,
    not_validated_entries: usize,
    cargo_check_passed: usize,
    cargo_check_failed: usize,
    cargo_check_not_run: usize,
}

#[derive(Debug, Serialize)]
struct OutputFile {
    path: String,
    bytes: u64,
    blake3: String,
    entries: usize,
    schema_valid_entries: usize,
    schema_invalid_entries: usize,
    cargo_check_passed: usize,
    cargo_check_failed: usize,
    cargo_check_not_run: usize,
}

#[derive(Debug, Serialize)]
struct QualitySection {
    requires_license: bool,
    dedupe: bool,
    secret_scan: bool,
    cargo_check_for_code: bool,
}

#[derive(Debug, Serialize)]
struct LicensePolicy {
    allowed: Vec<String>,
}

pub fn write_manifest(input: &Path, output: &Path) -> Result<()> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating manifest directory {}", parent.display()))?;
    }

    let report = validate_input(input)?;
    let outputs = output_files(input, &report)?;
    let summary = SummarySection {
        generated_at: chrono::Utc::now().to_rfc3339(),
        total_entries: report.total_entries,
        schema_valid_entries: report.valid_entries,
        schema_invalid_entries: report.invalid_entries,
        validated_entries: *report
            .counts_by_validation_status
            .get("validated")
            .unwrap_or(&0),
        not_validated_entries: *report
            .counts_by_validation_status
            .get("not_validated")
            .unwrap_or(&0),
        cargo_check_passed: *report.counts_by_cargo_check.get("passed").unwrap_or(&0),
        cargo_check_failed: *report.counts_by_cargo_check.get("failed").unwrap_or(&0),
        cargo_check_not_run: *report.counts_by_cargo_check.get("not_run").unwrap_or(&0),
    };

    let manifest = Manifest {
        schema: "aptlantis.rust_corpus.v1".to_string(),
        dataset: DatasetSection {
            id: "aptlantis-rust-tinyllama-sft-v1".to_string(),
            name: "Aptlantis Rust TinyLlama SFT Corpus".to_string(),
            language: "rust".to_string(),
            target_model: "TinyLlama-1.1B-Chat-v1.0".to_string(),
            created_at: "2026-04-30T00:00:00Z".to_string(),
        },
        summary,
        outputs,
        quality: QualitySection {
            requires_license: true,
            dedupe: true,
            secret_scan: true,
            cargo_check_for_code: true,
        },
        license_policy: LicensePolicy {
            allowed: vec![
                "MIT".to_string(),
                "Apache-2.0".to_string(),
                "MIT OR Apache-2.0".to_string(),
                "BSD-2-Clause".to_string(),
                "BSD-3-Clause".to_string(),
            ],
        },
    };

    let toml = toml::to_string_pretty(&manifest)?;
    std::fs::write(output, toml)
        .with_context(|| format!("writing manifest {}", output.display()))?;
    Ok(())
}

fn output_files(
    input: &Path,
    report: &crate::quality::report::QualityReport,
) -> Result<BTreeMap<String, OutputFile>> {
    let mut files = jsonl_files(input)?;
    files.sort();

    let mut outputs = BTreeMap::new();
    for file in files {
        let name = display_name(input, &file);
        let entries = read_jsonl(&file)?;
        let mut cargo_check_passed = 0usize;
        let mut cargo_check_failed = 0usize;
        let mut cargo_check_not_run = 0usize;

        for entry in &entries {
            match entry.metadata.cargo_check {
                Some(true) => cargo_check_passed += 1,
                Some(false) => cargo_check_failed += 1,
                None => cargo_check_not_run += 1,
            }
        }

        outputs.insert(
            manifest_key(&name),
            OutputFile {
                path: name,
                bytes: std::fs::metadata(&file)
                    .with_context(|| format!("stat {}", file.display()))?
                    .len(),
                blake3: blake3_file(&file)?,
                entries: entries.len(),
                schema_valid_entries: entries.len(),
                schema_invalid_entries: 0,
                cargo_check_passed,
                cargo_check_failed,
                cargo_check_not_run,
            },
        );
    }

    let parquet = input.join("rust_corpus.parquet");
    if parquet.exists() {
        outputs.insert(
            "rust_corpus_parquet".to_string(),
            OutputFile {
                path: "rust_corpus.parquet".to_string(),
                bytes: std::fs::metadata(&parquet)
                    .with_context(|| format!("stat {}", parquet.display()))?
                    .len(),
                blake3: blake3_file(&parquet)?,
                entries: report.total_entries,
                schema_valid_entries: report.valid_entries,
                schema_invalid_entries: report.invalid_entries,
                cargo_check_passed: *report.counts_by_cargo_check.get("passed").unwrap_or(&0),
                cargo_check_failed: *report.counts_by_cargo_check.get("failed").unwrap_or(&0),
                cargo_check_not_run: *report.counts_by_cargo_check.get("not_run").unwrap_or(&0),
            },
        );
    }

    Ok(outputs)
}

fn jsonl_files(input: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if input.is_file() {
        files.push(input.to_path_buf());
    } else {
        for entry in WalkDir::new(input).min_depth(1).max_depth(1) {
            let entry = entry.with_context(|| format!("walking {}", input.display()))?;
            if entry.file_type().is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "jsonl")
            {
                files.push(entry.path().to_path_buf());
            }
        }
    }

    Ok(files)
}

fn blake3_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0; 8192];

    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("reading {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(hasher.finalize().to_hex().to_string())
}

fn display_name(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .display()
        .to_string()
        .replace('\\', "/")
}

fn manifest_key(path: &str) -> String {
    path.trim_end_matches(".jsonl")
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect()
}
