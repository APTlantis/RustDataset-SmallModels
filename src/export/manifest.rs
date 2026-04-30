use std::{collections::BTreeMap, path::Path};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::generate::samples::{API_QA_FILE, COMPLETION_FILE, CONCEPTS_FILE, REPAIR_FILE};

#[derive(Debug, Serialize)]
struct Manifest {
    schema: String,
    dataset: DatasetSection,
    outputs: BTreeMap<String, String>,
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

pub fn write_manifest(_input: &Path, output: &Path) -> Result<()> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating manifest directory {}", parent.display()))?;
    }

    let manifest = Manifest {
        schema: "aptlantis.rust_corpus.v1".to_string(),
        dataset: DatasetSection {
            id: "aptlantis-rust-tinyllama-sft-v1".to_string(),
            name: "Aptlantis Rust TinyLlama SFT Corpus".to_string(),
            language: "rust".to_string(),
            target_model: "TinyLlama-1.1B-Chat-v1.0".to_string(),
            created_at: "2026-04-30T00:00:00Z".to_string(),
        },
        outputs: BTreeMap::from([
            ("concepts".to_string(), CONCEPTS_FILE.to_string()),
            ("api_qa".to_string(), API_QA_FILE.to_string()),
            ("code_completion".to_string(), COMPLETION_FILE.to_string()),
            ("code_repair".to_string(), REPAIR_FILE.to_string()),
            ("parquet".to_string(), "rust_corpus.parquet".to_string()),
        ]),
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
