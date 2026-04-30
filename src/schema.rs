use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatasetEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub entry_type: EntryType,
    pub messages: Vec<Message>,
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryType {
    ConceptQa,
    ApiQa,
    CodeCompletion,
    CodeGeneration,
    CodeRepair,
    Refactor,
    Explanation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    pub language: String,
    pub source: String,
    pub license: Option<String>,
    pub dataset: Option<String>,
    pub created_at: Option<String>,
    pub difficulty: Option<Difficulty>,
    pub topics: Vec<String>,
    pub quality_score: f32,
    pub validated: bool,

    pub api_item: Option<String>,
    pub crate_name: Option<String>,
    pub crate_version: Option<String>,
    pub file_path: Option<String>,

    pub cargo_check: Option<bool>,
    pub cargo_test: Option<bool>,
    pub clippy_clean: Option<bool>,
    pub error_kind: Option<String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Beginner,
    Intermediate,
    Advanced,
}

impl Metadata {
    pub fn sample(source: &str, topics: &[&str], difficulty: Difficulty) -> Self {
        Self {
            language: "rust".to_string(),
            source: source.to_string(),
            license: Some("MIT OR Apache-2.0".to_string()),
            dataset: Some("aptlantis-rust-tinyllama-sft-v1".to_string()),
            created_at: Some("2026-04-30T00:00:00Z".to_string()),
            difficulty: Some(difficulty),
            topics: topics.iter().map(|topic| (*topic).to_string()).collect(),
            quality_score: 0.95,
            validated: true,
            api_item: None,
            crate_name: None,
            crate_version: None,
            file_path: None,
            cargo_check: None,
            cargo_test: None,
            clippy_clean: None,
            error_kind: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Difficulty, EntryType, Role};

    #[test]
    fn serializes_entry_type_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&EntryType::ConceptQa).unwrap(),
            "\"concept_qa\""
        );
    }

    #[test]
    fn serializes_role_as_lowercase() {
        assert_eq!(
            serde_json::to_string(&Role::Assistant).unwrap(),
            "\"assistant\""
        );
    }

    #[test]
    fn serializes_difficulty_as_lowercase() {
        assert_eq!(
            serde_json::to_string(&Difficulty::Beginner).unwrap(),
            "\"beginner\""
        );
    }
}
