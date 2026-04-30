use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiItem {
    pub id: String,
    pub crate_name: Option<String>,
    pub crate_version: Option<String>,
    pub name: String,
    pub path: String,
    pub kind: String,
    pub docs: String,
    pub signature: Option<String>,
    pub source_path: String,
    pub topics: Vec<String>,
}

pub fn ingest_rustdoc(input: &Path, output: &Path) -> Result<Vec<ApiItem>> {
    let items = collect_api_items(input)?;
    write_api_items(output, &items)?;
    Ok(items)
}

pub fn collect_api_items(input: &Path) -> Result<Vec<ApiItem>> {
    let mut files = rustdoc_files(input)?;
    files.sort();

    let mut items = Vec::new();
    for file in files {
        let raw = std::fs::read_to_string(&file)
            .with_context(|| format!("reading rustdoc JSON {}", file.display()))?;
        let value = serde_json::from_str::<Value>(&raw)
            .with_context(|| format!("parsing rustdoc JSON {}", file.display()))?;
        items.extend(api_items_from_value(input, &file, &value));
    }

    items.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.kind.cmp(&right.kind))
            .then(left.name.cmp(&right.name))
    });

    for (index, item) in items.iter_mut().enumerate() {
        item.id = format!("rustdoc-api-item-{index:06}");
    }

    Ok(items)
}

pub fn write_api_items(path: &Path, items: &[ApiItem]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating API item output directory {}", parent.display()))?;
    }

    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    for item in items {
        serde_json::to_writer(&mut writer, item)
            .with_context(|| format!("serializing API item {}", item.id))?;
        writer
            .write_all(b"\n")
            .with_context(|| format!("writing API item line to {}", path.display()))?;
    }
    writer.flush()?;
    Ok(())
}

pub fn read_api_items(path: &Path) -> Result<Vec<ApiItem>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut items = Vec::new();

    for (index, line) in reader.lines().enumerate() {
        let line =
            line.with_context(|| format!("reading line {} in {}", index + 1, path.display()))?;
        if line.trim().is_empty() {
            continue;
        }

        let item = serde_json::from_str::<ApiItem>(&line)
            .with_context(|| format!("parsing line {} in {}", index + 1, path.display()))?;
        items.push(item);
    }

    Ok(items)
}

fn rustdoc_files(input: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if input.is_file() {
        files.push(input.to_path_buf());
    } else {
        for entry in WalkDir::new(input) {
            let entry = entry.with_context(|| format!("walking {}", input.display()))?;
            if entry.file_type().is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "json")
            {
                files.push(entry.path().to_path_buf());
            }
        }
    }

    Ok(files)
}

fn api_items_from_value(root: &Path, file: &Path, value: &Value) -> Vec<ApiItem> {
    let crate_name = value
        .pointer("/crate_version")
        .and_then(Value::as_str)
        .and_then(|_| value.pointer("/root"))
        .and_then(Value::as_str)
        .and_then(|root_id| value.pointer(&format!("/index/{root_id}/name")))
        .and_then(Value::as_str)
        .map(str::to_string);
    let crate_version = value
        .pointer("/crate_version")
        .and_then(Value::as_str)
        .map(str::to_string);

    let Some(index) = value.get("index").and_then(Value::as_object) else {
        return Vec::new();
    };

    let mut items = Vec::new();
    for item in index.values() {
        let Some(name) = item.get("name").and_then(Value::as_str) else {
            continue;
        };
        let kind = item_kind(item);
        if !is_supported_kind(&kind) {
            continue;
        }

        let docs = item
            .get("docs")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        let signature = item_signature(item);
        if docs.is_empty() && signature.is_none() {
            continue;
        }

        let path = item_path(item, name);
        items.push(ApiItem {
            id: String::new(),
            crate_name: crate_name.clone(),
            crate_version: crate_version.clone(),
            name: name.to_string(),
            path: path.clone(),
            kind,
            docs,
            signature,
            source_path: display_name(root, file),
            topics: topics_from_api_path(&path),
        });
    }

    items
}

fn item_kind(item: &Value) -> String {
    item.get("inner")
        .and_then(Value::as_object)
        .and_then(|inner| inner.keys().next().map(String::as_str))
        .or_else(|| item.get("kind").and_then(Value::as_str))
        .unwrap_or("unknown")
        .to_string()
}

fn item_path(item: &Value, name: &str) -> String {
    item.get("path")
        .and_then(Value::as_array)
        .map(|segments| {
            segments
                .iter()
                .filter_map(Value::as_str)
                .chain(std::iter::once(name))
                .collect::<Vec<_>>()
                .join("::")
        })
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| name.to_string())
}

fn item_signature(item: &Value) -> Option<String> {
    let inner = item.get("inner")?;
    let kind = item_kind(item);

    match kind.as_str() {
        "function" => Some(function_signature(item.get("name")?.as_str()?, inner)),
        "struct" => Some(format!("struct {}", item.get("name")?.as_str()?)),
        "enum" => Some(format!("enum {}", item.get("name")?.as_str()?)),
        "trait" => Some(format!("trait {}", item.get("name")?.as_str()?)),
        "type_alias" => Some(format!("type {}", item.get("name")?.as_str()?)),
        "constant" => Some(format!("const {}", item.get("name")?.as_str()?)),
        "module" => Some(format!("mod {}", item.get("name")?.as_str()?)),
        _ => None,
    }
}

fn function_signature(name: &str, inner: &Value) -> String {
    let inputs = inner
        .get("function")
        .and_then(|function| function.get("decl"))
        .and_then(|decl| decl.get("inputs"))
        .and_then(Value::as_array)
        .map(|inputs| {
            inputs
                .iter()
                .filter_map(|input| input.get(0).and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|inputs| !inputs.is_empty())
        .unwrap_or_default();

    format!("fn {name}({inputs})")
}

fn is_supported_kind(kind: &str) -> bool {
    matches!(
        kind,
        "module" | "struct" | "enum" | "trait" | "function" | "type_alias" | "constant"
    )
}

pub fn topics_from_api_path(path: &str) -> Vec<String> {
    let mut topics = Vec::new();
    for segment in path
        .split("::")
        .flat_map(|segment| segment.split('_'))
        .map(str::to_ascii_lowercase)
        .filter(|segment| segment.len() > 1)
    {
        if !topics.contains(&segment) {
            topics.push(segment);
        }
    }

    if topics.is_empty() {
        topics.push("api".to_string());
    }

    topics
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
        .display()
        .to_string()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{api_items_from_value, topics_from_api_path};

    #[test]
    fn extracts_documented_api_items_from_rustdoc_index() {
        let value = json!({
            "crate_version": "1.0.0",
            "index": {
                "0:0": {
                    "name": "map",
                    "path": ["sample", "Iterator"],
                    "docs": "Transforms each item with a closure.",
                    "inner": {
                        "function": {
                            "decl": {
                                "inputs": [["self", {"borrowed_ref": {}}], ["f", {"generic": "F"}]]
                            }
                        }
                    }
                },
                "0:1": {
                    "name": "hidden",
                    "path": ["sample"],
                    "docs": "",
                    "inner": { "import": {} }
                }
            }
        });

        let items = api_items_from_value(
            std::path::Path::new("docs"),
            std::path::Path::new("docs/sample.json"),
            &value,
        );

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].path, "sample::Iterator::map");
        assert_eq!(items[0].kind, "function");
        assert_eq!(items[0].signature.as_deref(), Some("fn map(self, f)"));
    }

    #[test]
    fn derives_topics_from_api_path() {
        assert_eq!(
            topics_from_api_path("std::option::Option::unwrap_or"),
            vec!["std", "option", "unwrap", "or"]
        );
    }
}
