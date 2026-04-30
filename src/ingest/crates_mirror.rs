use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

const MAX_SOURCE_BYTES: u64 = 24 * 1024;
const MIN_SNIPPET_LINES: usize = 3;
const MAX_SNIPPET_LINES: usize = 80;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodeItem {
    pub id: String,
    pub source_path: String,
    pub crate_name: Option<String>,
    pub item_kind: CodeItemKind,
    pub code: String,
    pub topics: Vec<String>,
    pub line_count: usize,
    pub has_tests: bool,
    pub has_main: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeItemKind {
    File,
    Function,
}

pub fn ingest_crates(input: &Path, output: &Path) -> Result<Vec<CodeItem>> {
    let items = collect_code_items(input)?;
    write_code_items(output, &items)?;
    Ok(items)
}

pub fn collect_code_items(input: &Path) -> Result<Vec<CodeItem>> {
    let mut files = rust_files(input)?;
    files.sort();

    let mut items = Vec::new();
    for file in files {
        if !is_candidate_file(&file)? {
            continue;
        }

        let raw = std::fs::read_to_string(&file)
            .with_context(|| format!("reading Rust source {}", file.display()))?;
        if !is_candidate_source(&raw) {
            continue;
        }

        let crate_name = crate_name_for_file(input, &file);
        let source_path = display_name(input, &file);
        let mut snippets = function_snippets(&raw);
        if snippets.is_empty() && is_snippet_sized(&raw) {
            snippets.push((CodeItemKind::File, raw.trim().to_string()));
        }

        for (item_kind, code) in snippets {
            let line_count = code.lines().count();
            if !(MIN_SNIPPET_LINES..=MAX_SNIPPET_LINES).contains(&line_count) {
                continue;
            }

            items.push(CodeItem {
                id: String::new(),
                source_path: source_path.clone(),
                crate_name: crate_name.clone(),
                item_kind,
                topics: topics_from_path(&source_path, &code),
                has_tests: code.contains("#[test]")
                    || source_path.starts_with("tests/")
                    || source_path.contains("/tests/"),
                has_main: code.contains("fn main("),
                line_count,
                code,
            });
        }
    }

    items.sort_by(|left, right| {
        left.source_path
            .cmp(&right.source_path)
            .then(left.item_kind.cmp(&right.item_kind))
            .then(left.code.cmp(&right.code))
    });

    for (index, item) in items.iter_mut().enumerate() {
        item.id = format!("code-item-{index:06}");
    }

    Ok(items)
}

pub fn write_code_items(path: &Path, items: &[CodeItem]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating code item output directory {}", parent.display()))?;
    }

    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    for item in items {
        serde_json::to_writer(&mut writer, item)
            .with_context(|| format!("serializing code item {}", item.id))?;
        writer
            .write_all(b"\n")
            .with_context(|| format!("writing code item line to {}", path.display()))?;
    }
    writer.flush()?;
    Ok(())
}

pub fn read_code_items(path: &Path) -> Result<Vec<CodeItem>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut items = Vec::new();

    for (index, line) in reader.lines().enumerate() {
        let line =
            line.with_context(|| format!("reading line {} in {}", index + 1, path.display()))?;
        if line.trim().is_empty() {
            continue;
        }

        let item = serde_json::from_str::<CodeItem>(&line)
            .with_context(|| format!("parsing line {} in {}", index + 1, path.display()))?;
        items.push(item);
    }

    Ok(items)
}

pub fn rust_files(input: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if input.is_file() {
        if input.extension().is_some_and(|extension| extension == "rs") {
            files.push(input.to_path_buf());
        }
    } else {
        for entry in WalkDir::new(input).into_iter().filter_entry(|entry| {
            entry.depth() == 0
                || !is_ignored_component(entry.file_name().to_string_lossy().as_ref())
        }) {
            let entry = entry.with_context(|| format!("walking {}", input.display()))?;
            if entry.file_type().is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "rs")
                && !is_generated_path(entry.path())
            {
                files.push(entry.path().to_path_buf());
            }
        }
    }

    files.sort();
    Ok(files)
}

pub fn is_candidate_source(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() || contains_likely_secret(trimmed) {
        return false;
    }

    let mut useful = 0usize;
    let mut comments_or_imports = 0usize;
    for line in trimmed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if line.starts_with("//")
            || line.starts_with("/*")
            || line.starts_with('*')
            || line.starts_with("use ")
            || line.starts_with("extern crate ")
        {
            comments_or_imports += 1;
        } else {
            useful += 1;
        }
    }

    useful >= 3 && useful >= comments_or_imports
}

pub fn function_snippets(raw: &str) -> Vec<(CodeItemKind, String)> {
    let lines = raw.lines().collect::<Vec<_>>();
    let mut snippets = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        let trimmed = lines[index].trim_start();
        if !starts_function(trimmed) {
            index += 1;
            continue;
        }

        let start = index;
        let mut brace_depth = 0isize;
        let mut saw_open = false;
        while index < lines.len() {
            for character in lines[index].chars() {
                match character {
                    '{' => {
                        saw_open = true;
                        brace_depth += 1;
                    }
                    '}' if saw_open => brace_depth -= 1,
                    _ => {}
                }
            }

            if saw_open && brace_depth == 0 {
                let code = lines[start..=index].join("\n").trim().to_string();
                if is_snippet_sized(&code) && is_candidate_source(&code) {
                    snippets.push((CodeItemKind::Function, code));
                }
                break;
            }

            index += 1;
        }

        index += 1;
    }

    snippets
}

fn starts_function(trimmed: &str) -> bool {
    trimmed.starts_with("fn ")
        || trimmed.starts_with("pub fn ")
        || trimmed.starts_with("pub(crate) fn ")
        || trimmed.starts_with("pub(super) fn ")
        || trimmed.starts_with("async fn ")
        || trimmed.starts_with("pub async fn ")
}

fn is_candidate_file(path: &Path) -> Result<bool> {
    let metadata = std::fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    Ok(metadata.len() <= MAX_SOURCE_BYTES && !is_generated_path(path))
}

fn is_snippet_sized(raw: &str) -> bool {
    let line_count = raw.lines().count();
    (MIN_SNIPPET_LINES..=MAX_SNIPPET_LINES).contains(&line_count)
}

fn contains_likely_secret(raw: &str) -> bool {
    let lowered = raw.to_ascii_lowercase();
    lowered.contains("api_key")
        || lowered.contains("secret_key")
        || lowered.contains("access_token")
        || lowered.contains("private_key")
        || lowered.contains("-----begin")
}

fn is_generated_path(path: &Path) -> bool {
    let lowered = path.display().to_string().to_ascii_lowercase();
    lowered.contains(".generated.")
        || lowered.contains("_generated.")
        || lowered.contains("/generated/")
        || lowered.contains("\\generated\\")
        || lowered.ends_with(".pb.rs")
}

fn is_ignored_component(component: &str) -> bool {
    matches!(
        component,
        ".git" | "target" | "vendor" | ".cargo" | "node_modules"
    )
}

fn topics_from_path(source_path: &str, code: &str) -> Vec<String> {
    let mut topics = Vec::new();
    for segment in source_path
        .split(['/', '\\', '.', '_', '-'])
        .map(str::to_ascii_lowercase)
        .filter(|segment| segment.len() > 1 && segment != "rs" && segment != "src")
    {
        if !topics.contains(&segment) {
            topics.push(segment);
        }
    }

    for keyword in [
        "iterator", "result", "option", "string", "vec", "async", "test",
    ] {
        if code.contains(keyword) && !topics.iter().any(|topic| topic == keyword) {
            topics.push(keyword.to_string());
        }
    }

    if topics.is_empty() {
        topics.push("rust".to_string());
    }

    topics
}

fn crate_name_for_file(root: &Path, file: &Path) -> Option<String> {
    if root.is_file() {
        return None;
    }

    let manifest = file.ancestors().find_map(|ancestor| {
        let manifest = ancestor.join("Cargo.toml");
        manifest.exists().then_some(manifest)
    })?;

    let raw = std::fs::read_to_string(manifest).ok()?;
    raw.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("name = "))
        .map(|name| name.trim_matches('"').to_string())
}

fn display_name(root: &Path, file: &Path) -> String {
    if root.is_file() {
        return file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
    }

    file.strip_prefix(root)
        .unwrap_or(file)
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{collect_code_items, function_snippets, is_candidate_source, rust_files};

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rust-corpus-forge-code-{nanos}"))
    }

    #[test]
    fn rust_file_discovery_skips_ignored_directories_and_non_rust_files() {
        let root = temp_dir();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("target/debug")).unwrap();
        fs::write(
            root.join("src/lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
        )
        .unwrap();
        fs::write(root.join("src/readme.txt"), "not rust").unwrap();
        fs::write(root.join("target/debug/build.rs"), "fn main() {}\n").unwrap();

        let files = rust_files(&root).unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with(Path::new("src/lib.rs")));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn candidate_source_rejects_comments_imports_and_secrets() {
        assert!(!is_candidate_source(
            "// comment\nuse std::fmt;\nuse std::io;"
        ));
        assert!(!is_candidate_source(
            "const API_KEY: &str = \"abc\";\nfn main() {\n    println!(\"x\");\n}"
        ));
        assert!(is_candidate_source(
            "pub fn add(a: i32, b: i32) -> i32 {\n    let total = a + b;\n    total\n}"
        ));
    }

    #[test]
    fn function_extraction_handles_balanced_braces() {
        let raw = "pub fn add(a: i32, b: i32) -> i32 {\n    let total = a + b;\n    total\n}\n\npub struct Skip;";
        let snippets = function_snippets(raw);

        assert_eq!(snippets.len(), 1);
        assert!(snippets[0].1.contains("let total = a + b;"));
    }

    #[test]
    fn collect_code_items_prefers_function_snippets() {
        let root = temp_dir();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"sample_crate\"\n",
        )
        .unwrap();
        fs::write(
            root.join("src/lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 {\n    let total = a + b;\n    total\n}\n",
        )
        .unwrap();

        let items = collect_code_items(&root).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "code-item-000000");
        assert_eq!(items[0].crate_name.as_deref(), Some("sample_crate"));

        fs::remove_dir_all(root).unwrap();
    }
}
