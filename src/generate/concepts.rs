use std::path::Path;

use anyhow::Result;

use crate::{
    export::jsonl::write_jsonl,
    ingest::mdbook::{MdBookChunk, read_chunks},
    schema::{DatasetEntry, Difficulty, EntryType, Message, Metadata, Role},
};

const SYSTEM_CONCEPTS: &str = "You explain Rust concepts clearly with small correct examples.";

pub fn generate_sft_from_chunks(input: &Path, output: &Path) -> Result<Vec<DatasetEntry>> {
    let chunks = read_chunks(input)?;
    let entries = concept_entries_from_chunks(&chunks);
    write_jsonl(output, &entries)?;
    Ok(entries)
}

pub fn concept_entries_from_chunks(chunks: &[MdBookChunk]) -> Vec<DatasetEntry> {
    let mut entries = concept_anchor_entries();
    entries.extend(chunks.iter().map(concept_entry_from_chunk));
    entries
}

fn concept_entry_from_chunk(chunk: &MdBookChunk) -> DatasetEntry {
    let mut metadata = Metadata::sample("mdbook", &[], Difficulty::Beginner);
    metadata.topics = chunk.topics.clone();
    metadata.file_path = Some(chunk.source_path.clone());
    metadata.quality_score = 0.90;
    metadata.validated = false;

    DatasetEntry {
        id: chunk.id.replace("mdbook-chunk", "rust-concept-mdbook"),
        entry_type: EntryType::ConceptQa,
        messages: vec![
            Message {
                role: Role::System,
                content: SYSTEM_CONCEPTS.to_string(),
            },
            Message {
                role: Role::User,
                content: format!("Explain `{}` in Rust.", chunk.heading),
            },
            Message {
                role: Role::Assistant,
                content: assistant_content(chunk),
            },
        ],
        metadata,
    }
}

fn assistant_content(chunk: &MdBookChunk) -> String {
    let summary = first_paragraph(&chunk.content)
        .unwrap_or_else(|| format!("`{}` is an important Rust topic.", chunk.heading));

    if let Some(code) = first_rust_code_block(&chunk.content) {
        format!(
            "{summary}\n\n```rust\n{code}\n```\n\nThis example is intentionally small so the concept stays focused."
        )
    } else {
        summary
    }
}

fn concept_anchor_entries() -> Vec<DatasetEntry> {
    vec![
        concept_anchor_entry(
            "rust-concept-anchor-question-mark-0001",
            "Explain the `?` operator in Rust.",
            "The `?` operator is used inside a function that returns `Result` or `Option`. It unwraps the success value and returns early if there is an error.\n\n```rust\nfn parse_port(input: &str) -> Result<u16, std::num::ParseIntError> {\n    let port = input.trim().parse::<u16>()?;\n    Ok(port)\n}\n```\n\nHere `?` turns `Ok(value)` into `value`; if parsing fails, the `ParseIntError` is returned from `parse_port`.",
            &["result", "question-mark", "error-handling"],
        ),
        concept_anchor_entry(
            "rust-concept-anchor-iterator-filter-map-0001",
            "How do `filter` and `map` work together on iterators?",
            "`filter` keeps only items that match a predicate, and `map` transforms each remaining item. They are lazy until a consumer such as `collect` runs.\n\n```rust\nfn doubled_even(values: &[i32]) -> Vec<i32> {\n    values.iter().copied().filter(|n| n % 2 == 0).map(|n| n * 2).collect()\n}\n```\n\nThis copies values out of the slice, keeps even numbers, doubles them, and collects the result into a `Vec<i32>`.",
            &["iterator", "filter", "map", "collect"],
        ),
        concept_anchor_entry(
            "rust-concept-anchor-display-bound-0001",
            "What does a `Display` trait bound mean?",
            "A `Display` bound means a value can be formatted with `{}`. Generic functions use the bound when they need human-readable formatting.\n\n```rust\nuse std::fmt::Display;\n\nfn label<T: Display>(name: T) -> String {\n    format!(\"name: {name}\")\n}\n```\n\nWithout `T: Display`, the compiler cannot know that `name` supports `{}` formatting.",
            &["trait", "display", "generics"],
        ),
    ]
}

fn concept_anchor_entry(id: &str, user: &str, assistant: &str, topics: &[&str]) -> DatasetEntry {
    let mut metadata = Metadata::sample("hand-authored-anchor", topics, Difficulty::Beginner);
    metadata.quality_score = 0.95;
    metadata.validated = true;
    metadata.cargo_check = Some(true);

    DatasetEntry {
        id: id.to_string(),
        entry_type: EntryType::ConceptQa,
        messages: vec![
            Message {
                role: Role::System,
                content: SYSTEM_CONCEPTS.to_string(),
            },
            Message {
                role: Role::User,
                content: user.to_string(),
            },
            Message {
                role: Role::Assistant,
                content: assistant.to_string(),
            },
        ],
        metadata,
    }
}

fn first_paragraph(content: &str) -> Option<String> {
    content
        .split("\n\n")
        .map(str::trim)
        .find(|paragraph| {
            !paragraph.is_empty()
                && !paragraph.starts_with("```")
                && !paragraph.starts_with('>')
                && !paragraph.starts_with('|')
        })
        .map(strip_markdown_links)
}

fn first_rust_code_block(content: &str) -> Option<String> {
    let mut in_rust = false;
    let mut lines = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed == "```rust" || trimmed == "```rust," || trimmed.starts_with("```rust ") {
            in_rust = true;
            lines.clear();
            continue;
        }

        if in_rust && trimmed == "```" {
            let code = lines.join("\n").trim().to_string();
            return (!code.is_empty()).then_some(code);
        }

        if in_rust {
            lines.push(line.to_string());
        }
    }

    None
}

fn strip_markdown_links(paragraph: &str) -> String {
    let mut output = String::new();
    let mut chars = paragraph.chars().peekable();

    while let Some(character) = chars.next() {
        if character == '[' {
            let mut label = String::new();
            for next in chars.by_ref() {
                if next == ']' {
                    break;
                }
                label.push(next);
            }

            if chars.peek() == Some(&'(') {
                chars.next();
                for next in chars.by_ref() {
                    if next == ')' {
                        break;
                    }
                }
                output.push_str(&label);
            } else {
                output.push('[');
                output.push_str(&label);
            }
        } else {
            output.push(character);
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use crate::{
        generate::concepts::concept_entries_from_chunks, ingest::mdbook::MdBookChunk,
        quality::report::validate_entry,
    };

    #[test]
    fn generates_valid_concept_entry_from_chunk() {
        let chunk = MdBookChunk {
            id: "mdbook-chunk-000001".to_string(),
            source_path: "src/chapter.md".to_string(),
            heading: "Ownership".to_string(),
            heading_level: 1,
            content: "Ownership controls when values are dropped.\n\n```rust\nfn main() {\n    let value = String::from(\"rust\");\n    println!(\"{value}\");\n}\n```"
                .to_string(),
            topics: vec!["ownership".to_string()],
        };

        let entries = concept_entries_from_chunks(&[chunk]);

        let generated = entries
            .iter()
            .find(|entry| entry.id == "rust-concept-mdbook-000001")
            .unwrap();
        assert!(validate_entry(generated).is_empty());
    }

    #[test]
    fn concept_without_code_does_not_use_placeholder_hello_world() {
        let chunk = MdBookChunk {
            id: "mdbook-chunk-000002".to_string(),
            source_path: "src/chapter.md".to_string(),
            heading: "Borrowing".to_string(),
            heading_level: 1,
            content: "Borrowing lets code use a value without taking ownership.".to_string(),
            topics: vec!["borrowing".to_string()],
        };

        let entries = concept_entries_from_chunks(&[chunk]);

        let generated = entries
            .iter()
            .find(|entry| entry.id == "rust-concept-mdbook-000002")
            .unwrap();
        assert!(!generated.messages[2].content.contains("Hello, Rust"));
        assert!(!generated.messages[2].content.contains("The key idea is"));
        assert!(validate_entry(generated).is_empty());
    }
}
