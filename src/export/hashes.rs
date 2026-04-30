use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use walkdir::WalkDir;

pub fn write_hashes(input: &Path, output: &Path) -> Result<()> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating hash output directory {}", parent.display()))?;
    }

    let mut files = hashable_files(input, output)?;
    files.sort();

    let mut lines = Vec::new();
    for file in files {
        let hash = blake3_file(&file)?;
        let name = display_name(input, &file);
        lines.push(format!("{hash}  {name}"));
    }

    let mut rendered = lines.join("\n");
    if !rendered.is_empty() {
        rendered.push('\n');
    }

    std::fs::write(output, rendered)
        .with_context(|| format!("writing hash snapshot {}", output.display()))?;
    Ok(())
}

fn hashable_files(input: &Path, output: &Path) -> Result<Vec<PathBuf>> {
    let output = output
        .canonicalize()
        .unwrap_or_else(|_| output.to_path_buf());
    let mut files = Vec::new();

    if input.is_file() {
        files.push(input.to_path_buf());
    } else {
        for entry in WalkDir::new(input).min_depth(1).max_depth(1) {
            let entry = entry.with_context(|| format!("walking {}", input.display()))?;
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path().to_path_buf();
            let comparable = path.canonicalize().unwrap_or_else(|_| path.clone());
            if comparable != output {
                files.push(path);
            }
        }
    }

    Ok(files)
}

fn blake3_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0; 8192];

    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("reading {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(hasher.finalize().to_hex().to_string())
}

fn display_name(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .display()
        .to_string()
        .replace('\\', "/")
}
