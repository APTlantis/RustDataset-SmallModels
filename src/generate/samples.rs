use std::path::Path;

use anyhow::Result;

use crate::{
    export::jsonl::write_jsonl,
    schema::{DatasetEntry, Difficulty, EntryType, Message, Metadata, Role},
};

pub const CONCEPTS_FILE: &str = "rust_concepts_sft.jsonl";
pub const API_QA_FILE: &str = "rust_api_qa.jsonl";
pub const COMPLETION_FILE: &str = "rust_code_completion.jsonl";
pub const REPAIR_FILE: &str = "rust_code_repair.jsonl";

const SYSTEM_PRECISE: &str = "You are a precise and idiomatic Rust programming assistant.";
const SYSTEM_CONCEPTS: &str = "You explain Rust concepts clearly with small correct examples.";
const SYSTEM_FIX: &str = "You fix Rust code errors and explain the correction briefly.";
const SYSTEM_COMPLETE: &str = "You complete Rust code accurately and idiomatically.";

pub fn generate_samples(output: &Path) -> Result<()> {
    std::fs::create_dir_all(output)?;

    write_jsonl(&output.join(CONCEPTS_FILE), &concept_entries())?;
    write_jsonl(&output.join(API_QA_FILE), &api_entries())?;
    write_jsonl(&output.join(COMPLETION_FILE), &completion_entries())?;
    write_jsonl(&output.join(REPAIR_FILE), &repair_entries())?;

    Ok(())
}

pub fn concept_entries() -> Vec<DatasetEntry> {
    vec![
        entry(
            "rust-concept-ownership-0001",
            EntryType::ConceptQa,
            SYSTEM_CONCEPTS,
            "Explain ownership in Rust with a small example.",
            "Ownership means each value has one owner, and the value is dropped when that owner goes out of scope.\n\n```rust\nfn main() {\n    let name = String::from(\"Ferris\");\n    greet(name);\n}\n\nfn greet(name: String) {\n    println!(\"Hello, {name}\");\n}\n```\n\nThe `String` moves into `greet`, so `greet` becomes responsible for it.",
            Metadata::sample(
                "rust-book",
                &["ownership", "move", "memory"],
                Difficulty::Beginner,
            ),
        ),
        entry(
            "rust-concept-borrowing-0001",
            EntryType::ConceptQa,
            SYSTEM_CONCEPTS,
            "What is borrowing in Rust?",
            "Borrowing lets code use a value without taking ownership of it.\n\n```rust\nfn print_len(text: &String) {\n    println!(\"{}\", text.len());\n}\n\nfn main() {\n    let text = String::from(\"rust\");\n    print_len(&text);\n    println!(\"{}\", text);\n}\n```\n\nThe `&String` parameter borrows `text`, so `main` can still use it afterward.",
            Metadata::sample(
                "rust-book",
                &["borrowing", "references"],
                Difficulty::Beginner,
            ),
        ),
    ]
}

pub fn api_entries() -> Vec<DatasetEntry> {
    let mut map_meta = Metadata::sample(
        "rustdoc-json",
        &["iterator", "map", "closure"],
        Difficulty::Beginner,
    );
    map_meta.api_item = Some("Iterator::map".to_string());

    let mut option_meta = Metadata::sample(
        "rustdoc-json",
        &["option", "unwrap_or", "fallback"],
        Difficulty::Beginner,
    );
    option_meta.api_item = Some("Option::unwrap_or".to_string());

    let mut option_map_meta = Metadata::sample(
        "hand-authored-anchor",
        &["option", "map", "closure"],
        Difficulty::Beginner,
    );
    option_map_meta.api_item = Some("Option::map".to_string());

    let mut retain_meta = Metadata::sample(
        "hand-authored-anchor",
        &["vec", "retain", "predicate"],
        Difficulty::Beginner,
    );
    retain_meta.api_item = Some("Vec::retain".to_string());

    let mut as_str_meta = Metadata::sample(
        "hand-authored-anchor",
        &["string", "str", "borrow"],
        Difficulty::Beginner,
    );
    as_str_meta.api_item = Some("String::as_str".to_string());

    vec![
        entry(
            "rust-api-iterator-map-0001",
            EntryType::ApiQa,
            SYSTEM_PRECISE,
            "What does `Iterator::map` do in Rust?",
            "`Iterator::map` transforms each item from an iterator with a closure.\n\n```rust\nfn main() {\n    let nums = vec![1, 2, 3];\n    let doubled: Vec<i32> = nums.iter().map(|n| n * 2).collect();\n    println!(\"{:?}\", doubled);\n}\n```\n\nThis prints `[2, 4, 6]`.",
            map_meta,
        ),
        entry(
            "rust-api-option-unwrap-or-0001",
            EntryType::ApiQa,
            SYSTEM_PRECISE,
            "When should I use `Option::unwrap_or`?",
            "Use `Option::unwrap_or` when you want a default value if an `Option` is `None`.\n\n```rust\nfn main() {\n    let port: Option<u16> = None;\n    let port = port.unwrap_or(8080);\n    println!(\"{port}\");\n}\n```\n\nIf the option is `Some(value)`, that value is returned instead.",
            option_meta,
        ),
        entry(
            "rust-api-option-map-0001",
            EntryType::ApiQa,
            SYSTEM_PRECISE,
            "What does `Option::map` do in Rust?",
            "`Option::map` transforms the value inside `Some` with a closure and leaves `None` unchanged.\n\n```rust\nfn main() {\n    let name = Some(\"rust\");\n    let upper = name.map(|text| text.to_uppercase());\n    assert_eq!(upper, Some(String::from(\"RUST\")));\n}\n```\n\nUse it when the absence case should stay absent while the present value changes.",
            option_map_meta,
        ),
        entry(
            "rust-api-vec-retain-0001",
            EntryType::ApiQa,
            SYSTEM_PRECISE,
            "How does `Vec::retain` work?",
            "`Vec::retain` keeps only the elements for which its predicate returns `true`. It mutates the vector in place.\n\n```rust\nfn main() {\n    let mut values = vec![1, 2, 3, 4];\n    values.retain(|value| value % 2 == 0);\n    assert_eq!(values, vec![2, 4]);\n}\n```\n\nIt is different from `retain_mut`: `retain` lets the closure inspect each element, while `retain_mut` can also mutate it.",
            retain_meta,
        ),
        entry(
            "rust-api-string-as-str-0001",
            EntryType::ApiQa,
            SYSTEM_PRECISE,
            "When should I use `String::as_str`?",
            "`String::as_str` borrows a `String` as a string slice without allocating or moving the owned string.\n\n```rust\nfn print_label(label: &str) {\n    println!(\"{label}\");\n}\n\nfn main() {\n    let label = String::from(\"rust\");\n    print_label(label.as_str());\n    println!(\"{label}\");\n}\n```\n\nUse it when an API expects `&str` and you want to keep using the original `String` afterward.",
            as_str_meta,
        ),
    ]
}

pub fn completion_entries() -> Vec<DatasetEntry> {
    let mut first = Metadata::sample(
        "hand-authored-sample",
        &["result", "parse", "error-handling"],
        Difficulty::Beginner,
    );
    first.cargo_check = Some(true);

    let mut second = Metadata::sample(
        "hand-authored-sample",
        &["iterator", "filter", "collect"],
        Difficulty::Beginner,
    );
    second.cargo_check = Some(true);

    vec![
        entry(
            "rust-completion-parse-port-0001",
            EntryType::CodeCompletion,
            SYSTEM_COMPLETE,
            "Complete this function:\n\n```rust\nfn parse_port(input: &str) -> Result<u16, std::num::ParseIntError> {\n    // ...\n}\n```",
            "```rust\nfn parse_port(input: &str) -> Result<u16, std::num::ParseIntError> {\n    input.trim().parse::<u16>()\n}\n```\n\nThe function trims whitespace and returns the parser result directly.",
            first,
        ),
        entry(
            "rust-completion-even-numbers-0001",
            EntryType::CodeCompletion,
            SYSTEM_COMPLETE,
            "Complete a function that returns only even numbers from a slice.\n\n```rust\nfn even_numbers(values: &[i32]) -> Vec<i32> {\n    // ...\n}\n```",
            "```rust\nfn even_numbers(values: &[i32]) -> Vec<i32> {\n    values.iter().copied().filter(|value| value % 2 == 0).collect()\n}\n```\n\n`copied` turns `&i32` items into `i32` values before collection.",
            second,
        ),
    ]
}

pub fn repair_entries() -> Vec<DatasetEntry> {
    let mut moved = Metadata::sample(
        "hand-authored-sample",
        &["ownership", "move", "clone"],
        Difficulty::Beginner,
    );
    moved.cargo_check = Some(true);
    moved.error_kind = Some("move_after_move".to_string());

    let mut mutability = Metadata::sample(
        "hand-authored-sample",
        &["mutability", "binding"],
        Difficulty::Beginner,
    );
    mutability.cargo_check = Some(true);
    mutability.error_kind = Some("immutable_binding_mutated".to_string());

    vec![
        entry(
            "rust-repair-move-error-0001",
            EntryType::CodeRepair,
            SYSTEM_FIX,
            "Fix this Rust code:\n\n```rust\nfn main() {\n    let s = String::from(\"hello\");\n    let t = s;\n    println!(\"{}\", s);\n}\n```",
            "The value `s` is moved into `t`, so `s` cannot be used afterward. Clone it if both values are needed.\n\n```rust\nfn main() {\n    let s = String::from(\"hello\");\n    let t = s.clone();\n    println!(\"{}\", s);\n    println!(\"{}\", t);\n}\n```",
            moved,
        ),
        entry(
            "rust-repair-mutability-0001",
            EntryType::CodeRepair,
            SYSTEM_FIX,
            "Fix this Rust code:\n\n```rust\nfn main() {\n    let count = 0;\n    count += 1;\n    println!(\"{}\", count);\n}\n```",
            "The binding must be mutable before it can be changed.\n\n```rust\nfn main() {\n    let mut count = 0;\n    count += 1;\n    println!(\"{}\", count);\n}\n```",
            mutability,
        ),
    ]
}

fn entry(
    id: &str,
    entry_type: EntryType,
    system: &str,
    user: &str,
    assistant: &str,
    metadata: Metadata,
) -> DatasetEntry {
    DatasetEntry {
        id: id.to_string(),
        entry_type,
        messages: vec![
            Message {
                role: Role::System,
                content: system.to_string(),
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
