use std::path::Path;

use anyhow::Result;

use crate::{
    export::jsonl::write_jsonl,
    ingest::crates_mirror::{CodeItem, read_code_items},
    schema::{DatasetEntry, Difficulty, EntryType, Message, Metadata, Role},
};

const SYSTEM_FIX: &str = "You fix Rust code errors and explain the correction briefly.";

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
    let broken = broken_variant(&item.code)?;
    let mut metadata = Metadata::sample("local-rust-source", &[], Difficulty::Beginner);
    metadata.topics = item.topics.clone();
    metadata.crate_name = item.crate_name.clone();
    metadata.file_path = Some(item.source_path.clone());
    metadata.quality_score = 0.87;
    metadata.validated = false;
    metadata.error_kind = Some("missing_closing_brace".to_string());

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
                    item.source_path, broken
                ),
            },
            Message {
                role: Role::Assistant,
                content: format!(
                    "The snippet is missing its final closing brace. Restoring the brace fixes the block structure.\n\n```rust\n{}\n```",
                    item.code
                ),
            },
        ],
        metadata,
    })
}

fn broken_variant(code: &str) -> Option<String> {
    let trimmed = code.trim_end();
    trimmed
        .strip_suffix('}')
        .map(|broken| broken.trim_end().to_string())
}

#[cfg(test)]
mod tests {
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
}
