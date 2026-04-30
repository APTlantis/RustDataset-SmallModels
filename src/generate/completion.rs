use std::path::Path;

use anyhow::Result;

use crate::{
    export::jsonl::write_jsonl,
    ingest::crates_mirror::{CodeItem, read_code_items},
    schema::{DatasetEntry, Difficulty, EntryType, Message, Metadata, Role},
};

const SYSTEM_COMPLETE: &str = "You complete Rust code accurately and idiomatically.";

pub fn generate_completion(input: &Path, output: &Path) -> Result<Vec<DatasetEntry>> {
    let items = read_code_items(input)?;
    let entries = completion_entries_from_items(&items);
    write_jsonl(output, &entries)?;
    Ok(entries)
}

pub fn completion_entries_from_items(items: &[CodeItem]) -> Vec<DatasetEntry> {
    items.iter().map(completion_entry_from_item).collect()
}

fn completion_entry_from_item(item: &CodeItem) -> DatasetEntry {
    let mut metadata = Metadata::sample("local-rust-source", &[], Difficulty::Beginner);
    metadata.topics = item.topics.clone();
    metadata.crate_name = item.crate_name.clone();
    metadata.file_path = Some(item.source_path.clone());
    metadata.quality_score = 0.88;
    metadata.validated = false;
    metadata.cargo_check = Some(false);

    DatasetEntry {
        id: item.id.replace("code-item", "rust-completion-source"),
        entry_type: EntryType::CodeCompletion,
        messages: vec![
            Message {
                role: Role::System,
                content: SYSTEM_COMPLETE.to_string(),
            },
            Message {
                role: Role::User,
                content: format!(
                    "Complete this Rust code from `{}`:\n\n```rust\n{}\n```",
                    item.source_path,
                    partial_code(&item.code)
                ),
            },
            Message {
                role: Role::Assistant,
                content: format!(
                    "```rust\n{}\n```\n\nThe completion preserves the original logic and keeps the snippet self-contained.",
                    item.code
                ),
            },
        ],
        metadata,
    }
}

fn partial_code(code: &str) -> String {
    let lines = code.lines().collect::<Vec<_>>();
    if lines.len() <= 4 {
        return format!("{}\n    // TODO: complete the implementation", lines[0]);
    }

    let keep = (lines.len() / 2).clamp(2, 8);
    let mut partial = lines[..keep].join("\n");
    partial.push_str("\n    // TODO: complete the implementation");
    partial
}

#[cfg(test)]
mod tests {
    use crate::{
        generate::completion::completion_entries_from_items,
        ingest::crates_mirror::{CodeItem, CodeItemKind},
        quality::report::validate_entry,
    };

    #[test]
    fn generates_valid_completion_entry_from_code_item() {
        let item = CodeItem {
            id: "code-item-000001".to_string(),
            source_path: "src/lib.rs".to_string(),
            crate_name: Some("sample_crate".to_string()),
            item_kind: CodeItemKind::Function,
            code: "pub fn add(a: i32, b: i32) -> i32 {\n    let total = a + b;\n    total\n}"
                .to_string(),
            topics: vec!["sample".to_string(), "result".to_string()],
            line_count: 4,
            has_tests: false,
            has_main: false,
        };

        let entries = completion_entries_from_items(&[item]);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "rust-completion-source-000001");
        assert!(validate_entry(&entries[0]).is_empty());
    }
}
