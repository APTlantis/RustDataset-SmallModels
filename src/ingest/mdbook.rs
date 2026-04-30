use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MdBookChunk {
    pub id: String,
    pub source_path: String,
    pub heading: String,
    pub heading_level: u8,
    pub content: String,
    pub topics: Vec<String>,
}

pub fn ingest_mdbook(input: &Path, output: &Path) -> Result<Vec<MdBookChunk>> {
    let chunks = collect_chunks(input)?;
    write_chunks(output, &chunks)?;
    Ok(chunks)
}

pub fn collect_chunks(input: &Path) -> Result<Vec<MdBookChunk>> {
    let mut files = markdown_files(input)?;
    files.sort();

    let mut chunks = Vec::new();
    for file in files {
        let raw = std::fs::read_to_string(&file)
            .with_context(|| format!("reading Markdown source {}", file.display()))?;
        chunks.extend(split_markdown(input, &file, &raw));
    }

    for (index, chunk) in chunks.iter_mut().enumerate() {
        chunk.id = format!("mdbook-chunk-{index:06}");
    }

    Ok(chunks)
}

pub fn write_chunks(path: &Path, chunks: &[MdBookChunk]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating chunk output directory {}", parent.display()))?;
    }

    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    for chunk in chunks {
        serde_json::to_writer(&mut writer, chunk)
            .with_context(|| format!("serializing chunk {}", chunk.id))?;
        writer
            .write_all(b"\n")
            .with_context(|| format!("writing chunk line to {}", path.display()))?;
    }
    writer.flush()?;
    Ok(())
}

pub fn read_chunks(path: &Path) -> Result<Vec<MdBookChunk>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut chunks = Vec::new();

    for (index, line) in reader.lines().enumerate() {
        let line =
            line.with_context(|| format!("reading line {} in {}", index + 1, path.display()))?;
        if line.trim().is_empty() {
            continue;
        }

        let chunk = serde_json::from_str::<MdBookChunk>(&line)
            .with_context(|| format!("parsing line {} in {}", index + 1, path.display()))?;
        chunks.push(chunk);
    }

    Ok(chunks)
}

fn markdown_files(input: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if input.is_file() {
        files.push(input.to_path_buf());
    } else {
        for entry in WalkDir::new(input) {
            let entry = entry.with_context(|| format!("walking {}", input.display()))?;
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            let is_markdown = path.extension().is_some_and(|extension| extension == "md");
            let is_summary = path
                .file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("SUMMARY.md"));

            if is_markdown && !is_summary {
                files.push(path.to_path_buf());
            }
        }
    }

    Ok(files)
}

fn split_markdown(root: &Path, file: &Path, raw: &str) -> Vec<MdBookChunk> {
    let mut chunks = Vec::new();
    let mut current_heading = String::new();
    let mut current_level = 1;
    let mut current_lines = Vec::new();

    for line in raw.lines() {
        if let Some((level, heading)) = parse_heading(line) {
            push_chunk(
                &mut chunks,
                root,
                file,
                &current_heading,
                current_level,
                &current_lines,
            );
            current_heading = heading;
            current_level = level;
            current_lines.clear();
        } else {
            current_lines.push(line.to_string());
        }
    }

    push_chunk(
        &mut chunks,
        root,
        file,
        &current_heading,
        current_level,
        &current_lines,
    );

    chunks
}

fn push_chunk(
    chunks: &mut Vec<MdBookChunk>,
    root: &Path,
    file: &Path,
    heading: &str,
    level: u8,
    lines: &[String],
) {
    let content = lines.join("\n").trim().to_string();
    if heading.trim().is_empty() || content.len() < 40 {
        return;
    }

    chunks.push(MdBookChunk {
        id: String::new(),
        source_path: display_name(root, file),
        heading: heading.trim().to_string(),
        heading_level: level,
        topics: topics_from_heading(heading),
        content,
    });
}

fn parse_heading(line: &str) -> Option<(u8, String)> {
    let trimmed = line.trim_start();
    let hashes = trimmed
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if !(1..=6).contains(&hashes) {
        return None;
    }

    let rest = trimmed.get(hashes..)?;
    if !rest.starts_with(' ') {
        return None;
    }

    let heading = rest.trim().trim_matches('#').trim().to_string();
    if heading.is_empty() {
        None
    } else {
        Some((hashes as u8, heading))
    }
}

pub fn topics_from_heading(heading: &str) -> Vec<String> {
    let stopwords = ["a", "an", "and", "in", "of", "the", "to", "with"];
    let mut topics = Vec::new();

    for word in heading
        .split(|character: char| !character.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .filter(|word| word.len() > 1 && !stopwords.contains(&word.as_str()))
    {
        if !topics.contains(&word) {
            topics.push(word);
        }
    }

    if topics.is_empty() {
        topics.push("rust".to_string());
    }

    topics
}

fn display_name(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .display()
        .to_string()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::{collect_chunks, split_markdown, topics_from_heading};

    #[test]
    fn splits_markdown_by_headings() {
        let raw = "# Ownership\nRust ownership keeps memory safe.\n\n```rust\nfn main() {}\n```\n\n## Moves\nMoving transfers ownership to another binding.";
        let chunks = split_markdown(
            std::path::Path::new("book"),
            std::path::Path::new("book/src/chapter.md"),
            raw,
        );

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].heading, "Ownership");
        assert_eq!(chunks[0].heading_level, 1);
        assert_eq!(chunks[0].source_path, "src/chapter.md");
        assert_eq!(chunks[1].heading, "Moves");
    }

    #[test]
    fn derives_topics_from_heading() {
        assert_eq!(
            topics_from_heading("Sharing State with Threads"),
            vec!["sharing", "state", "threads"]
        );
    }

    #[test]
    fn collect_chunks_assigns_stable_ids() {
        let chunks = collect_chunks(std::path::Path::new("OVERVIEW-RustCorpus.md")).unwrap();

        assert!(chunks.len() > 1);
        assert_eq!(chunks[0].id, "mdbook-chunk-000000");
    }
}
