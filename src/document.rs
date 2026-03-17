use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub title: String,
    pub theme: Theme,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Block {
    Step {
        id: usize,
        title: String,
        equations: Vec<String>,
        notes: Vec<String>,
        is_result: bool,
    },
    Prose {
        content: String,
    },
    Divider,
}

impl Document {
    pub fn new(title: &str) -> Self {
        Document {
            title: title.to_string(),
            theme: Theme::default(),
            blocks: Vec::new(),
        }
    }

    pub fn step_count(&self) -> usize {
        self.blocks.iter().filter(|b| matches!(b, Block::Step { .. })).count()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Selection {
    pub step_id: usize,
    pub title: String,
    pub latex: String,
    pub unicode: String,
    pub formatted: String,
    pub notes: Vec<String>,
    pub selected_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_new_defaults() {
        let doc = Document::new("My Title");
        assert_eq!(doc.title, "My Title");
        assert_eq!(doc.theme, Theme::Dark);
        assert!(doc.blocks.is_empty());
    }

    #[test]
    fn test_document_new_untitled() {
        let doc = Document::new("Untitled");
        assert_eq!(doc.title, "Untitled");
    }

    #[test]
    fn test_step_count_empty() {
        let doc = Document::new("Test");
        assert_eq!(doc.step_count(), 0);
    }

    #[test]
    fn test_step_count_only_steps() {
        let doc = Document {
            title: "T".to_string(),
            theme: Theme::Dark,
            blocks: vec![
                Block::Step {
                    id: 1,
                    title: "A".to_string(),
                    equations: vec![],
                    notes: vec![],
                    is_result: false,
                },
                Block::Step {
                    id: 2,
                    title: "B".to_string(),
                    equations: vec![],
                    notes: vec![],
                    is_result: false,
                },
            ],
        };
        assert_eq!(doc.step_count(), 2);
    }

    #[test]
    fn test_step_count_mixed_blocks() {
        let doc = Document {
            title: "T".to_string(),
            theme: Theme::Dark,
            blocks: vec![
                Block::Prose {
                    content: "intro".to_string(),
                },
                Block::Step {
                    id: 1,
                    title: "S1".to_string(),
                    equations: vec![],
                    notes: vec![],
                    is_result: false,
                },
                Block::Divider,
                Block::Step {
                    id: 2,
                    title: "S2".to_string(),
                    equations: vec![],
                    notes: vec![],
                    is_result: true,
                },
                Block::Prose {
                    content: "outro".to_string(),
                },
            ],
        };
        assert_eq!(doc.step_count(), 2);
    }

    #[test]
    fn test_theme_default() {
        assert_eq!(Theme::default(), Theme::Dark);
    }

    #[test]
    fn test_theme_equality() {
        assert_eq!(Theme::Dark, Theme::Dark);
        assert_eq!(Theme::Light, Theme::Light);
        assert_ne!(Theme::Dark, Theme::Light);
    }
}
