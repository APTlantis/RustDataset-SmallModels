use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::Serialize;
use walkdir::WalkDir;

use crate::{
    export::jsonl::read_jsonl,
    schema::{DatasetEntry, EntryType, Role},
};

#[derive(Debug, Clone, Serialize)]
pub struct QualityReport {
    pub input: String,
    pub total_entries: usize,
    pub valid_entries: usize,
    pub invalid_entries: usize,
    pub files: BTreeMap<String, FileReport>,
    pub counts_by_type: BTreeMap<String, usize>,
    pub counts_by_validation_status: BTreeMap<String, usize>,
    pub counts_by_cargo_check: BTreeMap<String, usize>,
    pub counts_by_topic: BTreeMap<String, usize>,
    pub counts_by_license: BTreeMap<String, usize>,
    pub errors: Vec<ValidationError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileReport {
    pub entries: usize,
    pub valid_entries: usize,
    pub invalid_entries: usize,
    pub cargo_check_passed: usize,
    pub cargo_check_failed: usize,
    pub cargo_check_not_run: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationError {
    pub file: String,
    pub id: Option<String>,
    pub message: String,
}

pub fn validate_to_report(input: &Path, report: &Path) -> Result<QualityReport> {
    let quality_report = validate_input(input)?;

    if let Some(parent) = report.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating report directory {}", parent.display()))?;
    }

    let json = serde_json::to_string_pretty(&quality_report)?;
    std::fs::write(report, format!("{json}\n"))
        .with_context(|| format!("writing quality report {}", report.display()))?;

    Ok(quality_report)
}

pub fn validate_input(input: &Path) -> Result<QualityReport> {
    let files = jsonl_files(input)?;
    let mut report = QualityReport {
        input: input.display().to_string(),
        total_entries: 0,
        valid_entries: 0,
        invalid_entries: 0,
        files: BTreeMap::new(),
        counts_by_type: BTreeMap::new(),
        counts_by_validation_status: BTreeMap::new(),
        counts_by_cargo_check: BTreeMap::new(),
        counts_by_topic: BTreeMap::new(),
        counts_by_license: BTreeMap::new(),
        errors: Vec::new(),
    };

    for file in files {
        let file_key = display_name(input, &file);
        match read_jsonl(&file) {
            Ok(entries) => {
                let mut file_report = FileReport {
                    entries: entries.len(),
                    valid_entries: 0,
                    invalid_entries: 0,
                    cargo_check_passed: 0,
                    cargo_check_failed: 0,
                    cargo_check_not_run: 0,
                };

                for entry in entries {
                    report.total_entries += 1;
                    increment(
                        &mut report.counts_by_type,
                        entry_type_name(entry.entry_type),
                    );
                    increment(
                        &mut report.counts_by_validation_status,
                        if entry.metadata.validated {
                            "validated"
                        } else {
                            "not_validated"
                        },
                    );
                    for topic in &entry.metadata.topics {
                        increment(&mut report.counts_by_topic, topic);
                    }
                    match entry.metadata.cargo_check {
                        Some(true) => {
                            file_report.cargo_check_passed += 1;
                            increment(&mut report.counts_by_cargo_check, "passed");
                        }
                        Some(false) => {
                            file_report.cargo_check_failed += 1;
                            increment(&mut report.counts_by_cargo_check, "failed");
                        }
                        None => {
                            file_report.cargo_check_not_run += 1;
                            increment(&mut report.counts_by_cargo_check, "not_run");
                        }
                    }
                    increment(
                        &mut report.counts_by_license,
                        entry.metadata.license.as_deref().unwrap_or("missing"),
                    );

                    let errors = validate_entry(&entry);
                    if errors.is_empty() {
                        report.valid_entries += 1;
                        file_report.valid_entries += 1;
                    } else {
                        report.invalid_entries += 1;
                        file_report.invalid_entries += 1;
                        for message in errors {
                            report.errors.push(ValidationError {
                                file: file_key.clone(),
                                id: Some(entry.id.clone()),
                                message,
                            });
                        }
                    }
                }

                report.files.insert(file_key, file_report);
            }
            Err(error) => {
                report.errors.push(ValidationError {
                    file: file_key.clone(),
                    id: None,
                    message: error.to_string(),
                });
                report.files.insert(
                    file_key,
                    FileReport {
                        entries: 0,
                        valid_entries: 0,
                        invalid_entries: 1,
                        cargo_check_passed: 0,
                        cargo_check_failed: 0,
                        cargo_check_not_run: 0,
                    },
                );
                report.invalid_entries += 1;
            }
        }
    }

    Ok(report)
}

pub fn validate_entry(entry: &DatasetEntry) -> Vec<String> {
    let mut errors = Vec::new();

    for role in [Role::System, Role::User, Role::Assistant] {
        if !entry.messages.iter().any(|message| message.role == role) {
            errors.push(format!("missing required {role:?} message"));
        }
    }

    if entry.metadata.language != "rust" {
        errors.push("metadata.language must be rust".to_string());
    }

    if !(0.0..=1.0).contains(&entry.metadata.quality_score) {
        errors.push("metadata.quality_score must be between 0.0 and 1.0".to_string());
    }

    for message in entry
        .messages
        .iter()
        .filter(|message| matches!(message.role, Role::User | Role::Assistant))
    {
        if let Err(error) = validate_fenced_code(&message.content) {
            errors.push(error);
        }
    }

    errors
}

fn validate_fenced_code(content: &str) -> std::result::Result<(), String> {
    let mut in_fence = false;

    for line in content.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("```") {
            continue;
        }

        if in_fence {
            in_fence = false;
            continue;
        }

        if trimmed.trim_end() != "```rust" {
            return Err("Rust code fences must open with ```rust".to_string());
        }
        in_fence = true;
    }

    if in_fence {
        return Err("unclosed Rust code fence".to_string());
    }

    Ok(())
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

    files.sort();
    Ok(files)
}

fn display_name(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .display()
        .to_string()
        .replace('\\', "/")
}

fn entry_type_name(entry_type: EntryType) -> &'static str {
    match entry_type {
        EntryType::ConceptQa => "concept_qa",
        EntryType::ApiQa => "api_qa",
        EntryType::CodeCompletion => "code_completion",
        EntryType::CodeGeneration => "code_generation",
        EntryType::CodeRepair => "code_repair",
        EntryType::Refactor => "refactor",
        EntryType::Explanation => "explanation",
    }
}

fn increment(map: &mut BTreeMap<String, usize>, key: &str) {
    *map.entry(key.to_string()).or_insert(0) += 1;
}

#[cfg(test)]
mod tests {
    use crate::{
        generate::samples::concept_entries, quality::report::validate_entry, schema::Role,
    };

    #[test]
    fn valid_sample_entries_pass() {
        for entry in concept_entries() {
            assert!(validate_entry(&entry).is_empty());
        }
    }

    #[test]
    fn missing_assistant_message_fails() {
        let mut entry = concept_entries().remove(0);
        entry
            .messages
            .retain(|message| message.role != Role::Assistant);

        let errors = validate_entry(&entry);

        assert!(errors.iter().any(|error| error.contains("Assistant")));
    }

    #[test]
    fn invalid_quality_score_fails() {
        let mut entry = concept_entries().remove(0);
        entry.metadata.quality_score = 1.5;

        let errors = validate_entry(&entry);

        assert!(errors.iter().any(|error| error.contains("quality_score")));
    }

    #[test]
    fn unlabeled_rust_code_fence_fails() {
        let mut entry = concept_entries().remove(0);
        entry.messages[1].content = "Please review:\n\n```\nfn main() {}\n```".to_string();

        let errors = validate_entry(&entry);

        assert!(errors.iter().any(|error| error.contains("```rust")));
    }
}
