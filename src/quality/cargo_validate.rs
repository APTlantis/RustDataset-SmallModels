use std::{collections::BTreeMap, path::Path, process::Command};

use anyhow::{Context, Result};

use crate::{
    export::jsonl::{read_jsonl, write_jsonl},
    ingest::crates_mirror::read_code_items,
    quality::code_blocks::assistant_rust_blocks,
    schema::DatasetEntry,
};

pub fn validate_code_jsonl(
    input: &Path,
    output: &Path,
    work_dir: &Path,
) -> Result<Vec<DatasetEntry>> {
    let mut entries = read_jsonl(input)?;
    validate_entries(&mut entries, work_dir)?;
    write_jsonl(output, &entries)?;
    Ok(entries)
}

pub fn validate_code_jsonl_with_source_context(
    input: &Path,
    output: &Path,
    work_dir: &Path,
    source_root: Option<&Path>,
) -> Result<Vec<DatasetEntry>> {
    let mut entries = read_jsonl(input)?;
    validate_entries_with_source_context(&mut entries, work_dir, source_root)?;
    write_jsonl(output, &entries)?;
    Ok(entries)
}

pub fn validate_code_jsonl_with_code_items(
    input: &Path,
    output: &Path,
    work_dir: &Path,
    code_items: &Path,
) -> Result<Vec<DatasetEntry>> {
    let mut entries = read_jsonl(input)?;
    let items = read_code_items(code_items)?;
    let source_roots = items
        .into_iter()
        .filter_map(|item| item.source_root.map(|root| (item.id, root)))
        .collect::<BTreeMap<_, _>>();
    let mut crate_results = BTreeMap::<String, bool>::new();

    for entry in &mut entries {
        let item_id = source_item_id(&entry.id);
        let source_root = item_id
            .as_ref()
            .and_then(|id| source_roots.get(id))
            .map(Path::new);

        if let Some(source_root) = source_root {
            let target_dir = work_dir
                .join("source_crate_targets")
                .join(safe_key(&source_root.display().to_string()));
            let result = if let Some(result) = crate_results.get(&source_root.display().to_string())
            {
                *result
            } else {
                let result = source_root.join("Cargo.toml").exists()
                    && cargo_check_crate_with_target(source_root, &target_dir)?;
                crate_results.insert(source_root.display().to_string(), result);
                result
            };

            if result {
                entry.metadata.cargo_check = Some(true);
                entry.metadata.validated = true;
                continue;
            }
        }

        validate_entries(std::slice::from_mut(entry), work_dir)?;
    }

    write_jsonl(output, &entries)?;
    Ok(entries)
}

pub fn validate_entries(entries: &mut [DatasetEntry], work_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(work_dir)
        .with_context(|| format!("creating cargo validation work dir {}", work_dir.display()))?;

    for (index, entry) in entries.iter_mut().enumerate() {
        let blocks = assistant_rust_blocks(entry);
        if blocks.is_empty() {
            entry.metadata.cargo_check = Some(false);
            entry.metadata.validated = false;
            continue;
        }

        let mut all_passed = true;
        for (block_index, block) in blocks.iter().enumerate() {
            let project_dir = work_dir.join(format!("entry-{index:06}-block-{block_index:02}"));
            let result = cargo_check_snippet(block, &project_dir)?;
            all_passed &= result;
        }

        entry.metadata.cargo_check = Some(all_passed);
        entry.metadata.validated = all_passed;
    }

    Ok(())
}

pub fn validate_entries_with_source_context(
    entries: &mut [DatasetEntry],
    work_dir: &Path,
    source_root: Option<&Path>,
) -> Result<()> {
    if let Some(source_root) = source_root
        && source_root.join("Cargo.toml").exists()
        && cargo_check_crate(source_root)?
    {
        for entry in entries {
            entry.metadata.cargo_check = Some(true);
            entry.metadata.validated = true;
        }
        return Ok(());
    }

    validate_entries(entries, work_dir)
}

pub fn cargo_check_crate(crate_root: &Path) -> Result<bool> {
    let target_dir = std::env::temp_dir().join(format!(
        "rust-corpus-forge-crate-target-{}",
        safe_key(&crate_root.display().to_string())
    ));
    let result = cargo_check_crate_with_target(crate_root, &target_dir);
    if target_dir.exists() {
        std::fs::remove_dir_all(&target_dir)
            .with_context(|| format!("removing {}", target_dir.display()))?;
    }
    result
}

fn cargo_check_crate_with_target(crate_root: &Path, target_dir: &Path) -> Result<bool> {
    let lockfile = crate_root.join("Cargo.lock");
    let lockfile_existed = lockfile.exists();
    let target_dir = if target_dir.is_absolute() {
        target_dir.to_path_buf()
    } else {
        std::env::current_dir()?.join(target_dir)
    };

    let output = Command::new("cargo")
        .arg("check")
        .arg("--quiet")
        .current_dir(crate_root)
        .env("CARGO_TARGET_DIR", &target_dir)
        .output()
        .with_context(|| {
            format!(
                "running cargo check in source crate {}",
                crate_root.display()
            )
        })?;

    if !lockfile_existed && lockfile.exists() {
        std::fs::remove_file(&lockfile)
            .with_context(|| format!("removing generated {}", lockfile.display()))?;
    }

    Ok(output.status.success())
}

fn safe_key(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn source_item_id(entry_id: &str) -> Option<String> {
    entry_id
        .strip_prefix("rust-completion-source-")
        .or_else(|| entry_id.strip_prefix("rust-repair-source-"))
        .map(|suffix| format!("code-item-{suffix}"))
}

pub fn cargo_check_snippet(code: &str, project_dir: &Path) -> Result<bool> {
    prepare_project(code, project_dir)?;

    let target_dir = project_dir.join("target");
    let output = Command::new("cargo")
        .arg("check")
        .arg("--quiet")
        .current_dir(project_dir)
        .env("CARGO_TARGET_DIR", &target_dir)
        .output()
        .with_context(|| format!("running cargo check in {}", project_dir.display()))?;

    Ok(output.status.success())
}

fn prepare_project(code: &str, project_dir: &Path) -> Result<()> {
    if project_dir.exists() {
        std::fs::remove_dir_all(project_dir)
            .with_context(|| format!("clearing {}", project_dir.display()))?;
    }
    std::fs::create_dir_all(project_dir.join("src"))
        .with_context(|| format!("creating {}", project_dir.join("src").display()))?;

    std::fs::write(
        project_dir.join("Cargo.toml"),
        "[package]\nname = \"snippet_check\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n",
    )
    .with_context(|| format!("writing {}", project_dir.join("Cargo.toml").display()))?;

    let source_path = if has_main_function(code) {
        project_dir.join("src/main.rs")
    } else {
        project_dir.join("src/lib.rs")
    };

    std::fs::write(&source_path, code)
        .with_context(|| format!("writing snippet source {}", source_path.display()))?;
    Ok(())
}

fn has_main_function(code: &str) -> bool {
    code.lines()
        .map(str::trim_start)
        .any(|line| line.starts_with("fn main(") || line.starts_with("pub fn main("))
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        generate::samples::completion_entries,
        quality::cargo_validate::{cargo_check_snippet, validate_entries},
    };

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rust-corpus-forge-{name}-{nanos}"))
    }

    #[test]
    fn cargo_check_accepts_simple_function_snippet() {
        let dir = temp_dir("cargo-pass");
        let passed =
            cargo_check_snippet("pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}", &dir).unwrap();

        assert!(passed);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cargo_check_rejects_broken_snippet() {
        let dir = temp_dir("cargo-fail");
        let passed = cargo_check_snippet("pub fn broken() {\n    missing();\n}", &dir).unwrap();

        assert!(!passed);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn validate_entries_updates_metadata() {
        let dir = temp_dir("entries");
        let mut entries = completion_entries();

        validate_entries(&mut entries, &dir).unwrap();

        assert!(
            entries
                .iter()
                .all(|entry| entry.metadata.cargo_check == Some(true))
        );
        assert!(entries.iter().all(|entry| entry.metadata.validated));

        std::fs::remove_dir_all(dir).unwrap();
    }
}
