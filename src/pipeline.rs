use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::{
    export::{hashes::write_hashes, manifest::write_manifest, parquet::export_parquet},
    generate::{
        api_qa::generate_api_qa,
        completion::generate_completion,
        concepts::generate_sft_from_chunks,
        repair::generate_repair,
        samples::{API_QA_FILE, COMPLETION_FILE, CONCEPTS_FILE, REPAIR_FILE, generate_samples},
    },
    ingest::{crates_mirror::ingest_crates, mdbook::ingest_mdbook, rustdoc_json::ingest_rustdoc},
    quality::{cargo_validate::validate_code_jsonl_with_code_items, report::validate_to_report},
};

#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub mdbook: Option<PathBuf>,
    pub rustdoc: Option<PathBuf>,
    pub crates: Option<PathBuf>,
    pub out: PathBuf,
    pub work: PathBuf,
    pub clean: bool,
    pub validate_code: bool,
}

pub fn run_pipeline(config: PipelineConfig) -> Result<()> {
    if config.clean {
        clean_generated(&config.out, &config.work)?;
    }

    std::fs::create_dir_all(&config.out)
        .with_context(|| format!("creating output directory {}", config.out.display()))?;
    std::fs::create_dir_all(&config.work)
        .with_context(|| format!("creating work directory {}", config.work.display()))?;

    generate_samples(&config.out)?;

    if let Some(mdbook) = &config.mdbook {
        let chunks = config.work.join("book_chunks.jsonl");
        ingest_mdbook(mdbook, &chunks)?;
        generate_sft_from_chunks(&chunks, &config.out.join(CONCEPTS_FILE))?;
    }

    if let Some(rustdoc) = &config.rustdoc {
        let api_items = config.work.join("api_items.jsonl");
        ingest_rustdoc(rustdoc, &api_items)?;
        generate_api_qa(&api_items, &config.out.join(API_QA_FILE))?;
    }

    if let Some(crates) = &config.crates {
        let code_items = config.work.join("code_items.jsonl");
        let completion_output = config.out.join(COMPLETION_FILE);
        let repair_output = config.out.join(REPAIR_FILE);
        ingest_crates(crates, &code_items)?;
        generate_completion(&code_items, &completion_output)?;
        generate_repair(&code_items, &repair_output)?;

        if config.validate_code {
            validate_code_jsonl_with_code_items(
                &completion_output,
                &completion_output,
                &config.work.join("cargo_validate"),
                &code_items,
            )?;
            validate_code_jsonl_with_code_items(
                &repair_output,
                &repair_output,
                &config.work.join("cargo_validate"),
                &code_items,
            )?;
            remove_path_if_exists(&config.work.join("cargo_validate"))?;
        }
    }

    validate_to_report(&config.out, &config.out.join("quality_report.json"))?;
    export_parquet(&config.out, &config.out.join("rust_corpus.parquet"))?;
    write_manifest(&config.out, &config.out.join("corpus_manifest.toml"))?;
    write_hashes(&config.out, &config.out.join("snapshot-hashes.txt"))?;

    Ok(())
}

pub fn clean_generated(out: &Path, work: &Path) -> Result<()> {
    for path in canonical_output_paths(out) {
        remove_path_if_exists(&path)?;
    }

    for path in legacy_output_paths(out) {
        remove_path_if_exists(&path)?;
    }

    for path in canonical_work_paths(work) {
        remove_path_if_exists(&path)?;
    }

    Ok(())
}

fn canonical_output_paths(out: &Path) -> Vec<PathBuf> {
    [
        CONCEPTS_FILE,
        API_QA_FILE,
        COMPLETION_FILE,
        "rust_code_repair.jsonl",
        "rust_corpus.parquet",
        "corpus_manifest.toml",
        "quality_report.json",
        "snapshot-hashes.txt",
    ]
    .into_iter()
    .map(|file| out.join(file))
    .collect()
}

fn legacy_output_paths(out: &Path) -> Vec<PathBuf> {
    [
        "rust_concepts_from_overview.jsonl",
        "rust_api_qa_from_rustdoc.jsonl",
        "rust_code_completion_from_sources.jsonl",
        "rust_code_completion_validated.jsonl",
    ]
    .into_iter()
    .map(|file| out.join(file))
    .collect()
}

fn canonical_work_paths(work: &Path) -> Vec<PathBuf> {
    [
        "book_chunks.jsonl",
        "api_items.jsonl",
        "code_items.jsonl",
        "cargo_validate",
    ]
    .into_iter()
    .map(|file| work.join(file))
    .collect()
}

fn remove_path_if_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    if path.is_dir() {
        std::fs::remove_dir_all(path).with_context(|| format!("removing {}", path.display()))?;
    } else {
        std::fs::remove_file(path).with_context(|| format!("removing {}", path.display()))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::pipeline::{PipelineConfig, clean_generated, run_pipeline};

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rust-corpus-forge-pipeline-{name}-{nanos}"))
    }

    #[test]
    fn clean_generated_removes_canonical_and_legacy_files() {
        let root = temp_dir("clean");
        let out = root.join("out");
        let work = root.join("work");
        fs::create_dir_all(&out).unwrap();
        fs::create_dir_all(&work).unwrap();
        fs::write(out.join("rust_code_completion_validated.jsonl"), "{}").unwrap();
        fs::write(out.join("quality_report.json"), "{}").unwrap();
        fs::write(work.join("code_items.jsonl"), "{}").unwrap();

        clean_generated(&out, &work).unwrap();

        assert!(!out.join("rust_code_completion_validated.jsonl").exists());
        assert!(!out.join("quality_report.json").exists());
        assert!(!work.join("code_items.jsonl").exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sample_only_pipeline_writes_canonical_outputs() {
        let root = temp_dir("samples");
        let out = root.join("out");
        let work = root.join("work");

        run_pipeline(PipelineConfig {
            mdbook: None,
            rustdoc: None,
            crates: None,
            out: out.clone(),
            work,
            clean: false,
            validate_code: false,
        })
        .unwrap();

        assert!(out.join("rust_concepts_sft.jsonl").exists());
        assert!(out.join("rust_api_qa.jsonl").exists());
        assert!(out.join("rust_code_completion.jsonl").exists());
        assert!(out.join("rust_code_repair.jsonl").exists());
        assert!(out.join("quality_report.json").exists());

        fs::remove_dir_all(root).unwrap();
    }
}
