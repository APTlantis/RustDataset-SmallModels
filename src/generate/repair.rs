use std::path::Path;

use anyhow::Result;

use crate::{
    export::jsonl::write_jsonl,
    ingest::crates_mirror::{CodeItem, read_code_items},
    schema::{DatasetEntry, Difficulty, EntryType, Message, Metadata, Role},
};

const SYSTEM_FIX: &str = "You fix Rust code errors and explain the correction briefly.";

#[derive(Debug, Clone)]
struct BrokenRepair {
    code: String,
    error_kind: &'static str,
    explanation: &'static str,
}

pub fn generate_repair(input: &Path, output: &Path) -> Result<Vec<DatasetEntry>> {
    let items = read_code_items(input)?;
    let entries = repair_entries_from_items(&items);
    write_jsonl(output, &entries)?;
    Ok(entries)
}

pub fn repair_entries_from_items(items: &[CodeItem]) -> Vec<DatasetEntry> {
    items
        .iter()
        .filter_map(repair_entry_from_item)
        .collect::<Vec<_>>()
}

fn repair_entry_from_item(item: &CodeItem) -> Option<DatasetEntry> {
    let broken = broken_variant(item)?;
    let mut metadata = Metadata::sample("local-rust-source", &[], Difficulty::Beginner);
    metadata.topics = item.topics.clone();
    metadata.crate_name = item.crate_name.clone();
    metadata.file_path = Some(item.source_path.clone());
    metadata.quality_score = 0.87;
    metadata.validated = false;
    metadata.error_kind = Some(broken.error_kind.to_string());

    Some(DatasetEntry {
        id: item.id.replace("code-item", "rust-repair-source"),
        entry_type: EntryType::CodeRepair,
        messages: vec![
            Message {
                role: Role::System,
                content: SYSTEM_FIX.to_string(),
            },
            Message {
                role: Role::User,
                content: format!(
                    "Fix this Rust code from `{}`:\n\n```rust\n{}\n```",
                    item.source_path, broken.code
                ),
            },
            Message {
                role: Role::Assistant,
                content: format!("{}\n\n```rust\n{}\n```", broken.explanation, item.code),
            },
        ],
        metadata,
    })
}

fn broken_variant(item: &CodeItem) -> Option<BrokenRepair> {
    let mut variants = Vec::new();
    if let Some(variant) = missing_closing_brace(&item.code) {
        variants.push(variant);
    }
    if let Some(variant) = immutable_binding_mutated(&item.code) {
        variants.push(variant);
    }
    if let Some(variant) = wrong_collect_type(&item.code) {
        variants.push(variant);
    }
    if let Some(variant) = missing_question_mark(&item.code) {
        variants.push(variant);
    }

    if variants.is_empty() {
        return None;
    }

    let index = numeric_suffix(&item.id).unwrap_or(0) % variants.len();
    Some(variants.remove(index))
}

fn missing_closing_brace(code: &str) -> Option<BrokenRepair> {
    let trimmed = code.trim_end();
    trimmed.strip_suffix('}').map(|broken| BrokenRepair {
        code: broken.trim_end().to_string(),
        error_kind: "missing_closing_brace",
        explanation: "The snippet is missing its final closing brace. Restoring the brace fixes the block structure.",
    })
}

fn immutable_binding_mutated(code: &str) -> Option<BrokenRepair> {
    code.contains("let mut ").then(|| BrokenRepair {
        code: code.replacen("let mut ", "let ", 1),
        error_kind: "immutable_binding_mutated",
        explanation: "The binding is changed later, so it must be declared with `mut`.",
    })
}

fn wrong_collect_type(code: &str) -> Option<BrokenRepair> {
    (code.contains(".collect()") && code.contains("Vec")).then(|| BrokenRepair {
        code: code.replacen(".collect()", ".collect::<i32>()", 1),
        error_kind: "wrong_collect_type",
        explanation: "`collect` should build the function's collection type. Collecting into `i32` is the wrong target type here.",
    })
}

fn missing_question_mark(code: &str) -> Option<BrokenRepair> {
    (code.contains("Result<") && code.contains('?')).then(|| BrokenRepair {
        code: code.replacen('?', "", 1),
        error_kind: "missing_question_mark",
        explanation: "The fallible operation needs `?` so errors are returned early and successful values are unwrapped.",
    })
}

fn numeric_suffix(id: &str) -> Option<usize> {
    id.rsplit('-').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::broken_variant;
    use crate::{
        generate::repair::repair_entries_from_items,
        ingest::crates_mirror::{CodeItem, CodeItemKind},
        quality::report::validate_entry,
    };

    #[test]
    fn generates_valid_repair_entry_from_code_item() {
        let item = CodeItem {
            id: "code-item-000001".to_string(),
            source_path: "src/lib.rs".to_string(),
            source_root: None,
            crate_name: Some("sample_crate".to_string()),
            item_kind: CodeItemKind::Function,
            code: "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}".to_string(),
            topics: vec!["lib".to_string()],
            line_count: 3,
            has_tests: false,
            has_main: false,
        };

        let entries = repair_entries_from_items(&[item]);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "rust-repair-source-000001");
        assert!(validate_entry(&entries[0]).is_empty());
    }

    #[test]
    fn repair_generation_can_create_mutability_variant() {
        let item = CodeItem {
            id: "code-item-000001".to_string(),
            source_path: "src/lib.rs".to_string(),
            source_root: None,
            crate_name: Some("sample_crate".to_string()),
            item_kind: CodeItemKind::Function,
            code: "pub fn push_value(values: &mut Vec<i32>, value: i32) {\n    let mut next = value;\n    next += 1;\n    values.push(next);\n}"
                .to_string(),
            topics: vec!["vec".to_string()],
            line_count: 5,
            has_tests: false,
            has_main: false,
        };

        let broken = broken_variant(&item).unwrap();

        assert_eq!(broken.error_kind, "immutable_binding_mutated");
        assert!(broken.code.contains("let next = value;"));
    }
}
