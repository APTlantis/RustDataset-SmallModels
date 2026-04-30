use std::path::Path;

use anyhow::Result;

use crate::{
    export::jsonl::write_jsonl,
    ingest::rustdoc_json::{ApiItem, read_api_items},
    schema::{DatasetEntry, Difficulty, EntryType, Message, Metadata, Role},
};

const SYSTEM_PRECISE: &str = "You are a precise and idiomatic Rust programming assistant.";

pub fn generate_api_qa(input: &Path, output: &Path) -> Result<Vec<DatasetEntry>> {
    let items = read_api_items(input)?;
    let entries = api_qa_entries_from_items(&items);
    write_jsonl(output, &entries)?;
    Ok(entries)
}

pub fn api_qa_entries_from_items(items: &[ApiItem]) -> Vec<DatasetEntry> {
    items.iter().map(api_qa_entry_from_item).collect()
}

fn api_qa_entry_from_item(item: &ApiItem) -> DatasetEntry {
    let mut metadata = Metadata::sample("rustdoc-json", &[], Difficulty::Beginner);
    metadata.topics = item.topics.clone();
    metadata.api_item = Some(item.path.clone());
    metadata.crate_name = item.crate_name.clone();
    metadata.crate_version = item.crate_version.clone();
    metadata.file_path = Some(item.source_path.clone());
    metadata.quality_score = 0.90;
    metadata.validated = false;

    DatasetEntry {
        id: item.id.replace("rustdoc-api-item", "rust-api"),
        entry_type: EntryType::ApiQa,
        messages: vec![
            Message {
                role: Role::System,
                content: SYSTEM_PRECISE.to_string(),
            },
            Message {
                role: Role::User,
                content: format!("What does `{}` do in Rust?", item.path),
            },
            Message {
                role: Role::Assistant,
                content: assistant_content(item),
            },
        ],
        metadata,
    }
}

fn assistant_content(item: &ApiItem) -> String {
    let docs = first_sentence(&item.docs).unwrap_or_else(|| {
        format!(
            "`{}` is a Rust {} item documented by rustdoc.",
            item.path, item.kind
        )
    });
    let signature = item.signature.as_deref().unwrap_or(&item.path);

    format!(
        "{docs}\n\n```rust\n// {signature}\n```\n\nUse this API according to its rustdoc contract and surrounding type context."
    )
}

fn first_sentence(docs: &str) -> Option<String> {
    let compact = docs.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return None;
    }

    if let Some(index) = compact.find(". ") {
        Some(compact[..=index].to_string())
    } else {
        Some(compact)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        generate::api_qa::api_qa_entries_from_items, ingest::rustdoc_json::ApiItem,
        quality::report::validate_entry,
    };

    #[test]
    fn generates_valid_api_qa_entry_from_item() {
        let item = ApiItem {
            id: "rustdoc-api-item-000001".to_string(),
            crate_name: Some("sample".to_string()),
            crate_version: Some("1.0.0".to_string()),
            name: "map".to_string(),
            path: "sample::Iterator::map".to_string(),
            kind: "function".to_string(),
            docs: "Transforms each item with a closure. Returns a lazy iterator.".to_string(),
            signature: Some("fn map(self, f)".to_string()),
            source_path: "sample.json".to_string(),
            topics: vec!["iterator".to_string(), "map".to_string()],
        };

        let entries = api_qa_entries_from_items(&[item]);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "rust-api-000001");
        assert!(validate_entry(&entries[0]).is_empty());
    }
}
