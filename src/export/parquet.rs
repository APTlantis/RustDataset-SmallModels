use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use arrow_array::{ArrayRef, RecordBatch, StringArray, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use parquet::{arrow::ArrowWriter, file::properties::WriterProperties};
use walkdir::WalkDir;

use crate::export::jsonl::read_jsonl;

pub fn export_parquet(input: &Path, output: &Path) -> Result<usize> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating Parquet output directory {}", parent.display()))?;
    }

    let mut ids = Vec::new();
    let mut entry_types = Vec::new();
    let mut sources = Vec::new();
    let mut topics = Vec::new();
    let mut messages = Vec::new();
    let mut metadata = Vec::new();
    let mut files = Vec::new();
    let mut row_numbers = Vec::new();

    for file in jsonl_files(input)? {
        let file_name = display_name(input, &file);
        for (index, entry) in read_jsonl(&file)?.into_iter().enumerate() {
            ids.push(entry.id.clone());
            entry_types.push(entry_type_name(&entry));
            sources.push(entry.metadata.source.clone());
            topics.push(serde_json::to_string(&entry.metadata.topics)?);
            messages.push(serde_json::to_string(&entry.messages)?);
            metadata.push(serde_json::to_string(&entry.metadata)?);
            files.push(file_name.clone());
            row_numbers.push(index as u64);
        }
    }

    let row_count = ids.len();
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("type", DataType::Utf8, false),
        Field::new("source", DataType::Utf8, false),
        Field::new("topics_json", DataType::Utf8, false),
        Field::new("messages_json", DataType::Utf8, false),
        Field::new("metadata_json", DataType::Utf8, false),
        Field::new("source_file", DataType::Utf8, false),
        Field::new("source_row", DataType::UInt64, false),
    ]));

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(StringArray::from(ids)) as ArrayRef,
            Arc::new(StringArray::from(entry_types)) as ArrayRef,
            Arc::new(StringArray::from(sources)) as ArrayRef,
            Arc::new(StringArray::from(topics)) as ArrayRef,
            Arc::new(StringArray::from(messages)) as ArrayRef,
            Arc::new(StringArray::from(metadata)) as ArrayRef,
            Arc::new(StringArray::from(files)) as ArrayRef,
            Arc::new(UInt64Array::from(row_numbers)) as ArrayRef,
        ],
    )?;

    let file = std::fs::File::create(output)
        .with_context(|| format!("creating Parquet file {}", output.display()))?;
    let props = WriterProperties::builder().build();
    let mut writer = ArrowWriter::try_new(file, schema, Some(props))?;
    writer.write(&batch)?;
    writer.close()?;

    Ok(row_count)
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

fn entry_type_name(entry: &crate::schema::DatasetEntry) -> String {
    serde_json::to_value(entry.entry_type)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{export::parquet::export_parquet, generate::samples::generate_samples};

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rust-corpus-forge-parquet-{name}-{nanos}"))
    }

    #[test]
    fn exports_jsonl_outputs_to_parquet() {
        let root = temp_dir("export");
        let out = root.join("out");
        generate_samples(&out).unwrap();

        let rows = export_parquet(&out, &out.join("rust_corpus.parquet")).unwrap();

        assert_eq!(rows, 8);
        assert!(out.join("rust_corpus.parquet").exists());

        std::fs::remove_dir_all(root).unwrap();
    }
}
