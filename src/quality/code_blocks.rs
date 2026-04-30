use crate::schema::{DatasetEntry, Role};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustCodeBlock {
    pub role: Role,
    pub code: String,
}

pub fn assistant_rust_blocks(entry: &DatasetEntry) -> Vec<String> {
    rust_blocks(entry)
        .into_iter()
        .filter(|block| block.role == Role::Assistant)
        .map(|block| block.code)
        .collect()
}

pub fn rust_blocks(entry: &DatasetEntry) -> Vec<RustCodeBlock> {
    entry
        .messages
        .iter()
        .flat_map(|message| {
            extract_rust_blocks(&message.content)
                .into_iter()
                .map(|code| RustCodeBlock {
                    role: message.role,
                    code,
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

pub fn extract_rust_blocks(content: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut in_rust = false;
    let mut lines = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim_start();
        if !in_rust
            && (trimmed == "```rust" || trimmed == "```rust," || trimmed.starts_with("```rust "))
        {
            in_rust = true;
            lines.clear();
            continue;
        }

        if in_rust && trimmed == "```" {
            let code = lines.join("\n").trim().to_string();
            if !code.is_empty() {
                blocks.push(code);
            }
            in_rust = false;
            lines.clear();
            continue;
        }

        if in_rust {
            lines.push(line.to_string());
        }
    }

    blocks
}

#[cfg(test)]
mod tests {
    use crate::quality::code_blocks::extract_rust_blocks;

    #[test]
    fn extracts_labeled_rust_blocks() {
        let blocks = extract_rust_blocks("Text\n```rust\nfn main() {}\n```\nMore");

        assert_eq!(blocks, vec!["fn main() {}"]);
    }

    #[test]
    fn ignores_unlabeled_blocks() {
        let blocks = extract_rust_blocks("```\nfn main() {}\n```");

        assert!(blocks.is_empty());
    }
}
