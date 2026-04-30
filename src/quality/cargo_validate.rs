use std::{path::Path, process::Command};

use anyhow::{Context, Result};

use crate::{
    export::jsonl::{read_jsonl, write_jsonl},
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
