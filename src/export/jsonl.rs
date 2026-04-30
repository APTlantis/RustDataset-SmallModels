use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    path::Path,
};

use anyhow::{Context, Result};

use crate::schema::DatasetEntry;

pub fn write_jsonl(path: &Path, entries: &[DatasetEntry]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating output directory {}", parent.display()))?;
    }

    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut writer = BufWriter::new(file);

    for entry in entries {
        serde_json::to_writer(&mut writer, entry)
            .with_context(|| format!("serializing entry {}", entry.id))?;
        writer
            .write_all(b"\n")
            .with_context(|| format!("writing newline to {}", path.display()))?;
    }

    writer
        .flush()
        .with_context(|| format!("flushing {}", path.display()))?;
    Ok(())
}

pub fn read_jsonl(path: &Path) -> Result<Vec<DatasetEntry>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();

    for (index, line) in reader.lines().enumerate() {
        let line =
            line.with_context(|| format!("reading line {} in {}", index + 1, path.display()))?;
        if line.trim().is_empty() {
            continue;
        }

        let entry = serde_json::from_str::<DatasetEntry>(&line)
            .with_context(|| format!("parsing line {} in {}", index + 1, path.display()))?;
        entries.push(entry);
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        export::jsonl::{read_jsonl, write_jsonl},
        generate::samples::concept_entries,
    };

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rust-corpus-forge-{name}-{nanos}.jsonl"))
    }

    #[test]
    fn writes_one_valid_json_object_per_line() {
        let path = temp_path("valid");
        let entries = concept_entries();

        write_jsonl(&path, &entries).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        let lines = raw.lines().collect::<Vec<_>>();

        assert_eq!(lines.len(), entries.len());
        for line in lines {
            serde_json::from_str::<serde_json::Value>(line).unwrap();
        }

        let read_entries = read_jsonl(&path).unwrap();
        assert_eq!(read_entries.len(), entries.len());

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn reading_invalid_jsonl_returns_error() {
        let path = temp_path("invalid");
        fs::write(&path, "{not-json}\n").unwrap();

        let err = read_jsonl(&path).unwrap_err().to_string();
        assert!(err.contains("parsing line 1"));

        fs::remove_file(path).unwrap();
    }
}
