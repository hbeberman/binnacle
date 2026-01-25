//! Complexity detection heuristics for task descriptions.
//!
//! This module provides functions to detect potentially complex task descriptions
//! that might be better suited as ideas (for exploration) rather than tasks
//! (for execution). Used by buddy to soft-gate complex items.
//!
//! # Heuristics
//!
//! The complexity detection uses several heuristics:
//! - **Length-based**: Long titles/descriptions suggest multiple concerns
//! - **Multiple concerns**: Conjunctions and multiple action verbs
//! - **Vague scope**: Uncertainty markers and exploratory language
//! - **Structural**: Lists, open-ended markers, etc.
//!
//! # Example
//!
//! ```
//! use binnacle::models::complexity::{analyze_complexity, ComplexityScore};
//!
//! let score = analyze_complexity(
//!     "Add authentication and fix database and improve logging",
//!     Some("We need to do various things"),
//! );
//!
//! assert!(score.is_complex());
//! assert!(!score.reasons.is_empty());
//! ```

use serde::{Deserialize, Serialize};

/// Thresholds for complexity detection.
pub mod thresholds {
    /// Maximum recommended title length (characters).
    pub const MAX_TITLE_LENGTH: usize = 80;

    /// Maximum recommended description length (characters).
    pub const MAX_DESCRIPTION_LENGTH: usize = 500;

    /// Score threshold above which a task is considered complex.
    pub const COMPLEXITY_THRESHOLD: u8 = 3;
}

/// Result of complexity analysis.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComplexityScore {
    /// Numeric complexity score (higher = more complex).
    pub score: u8,

    /// Human-readable reasons for the complexity assessment.
    pub reasons: Vec<String>,
}

impl ComplexityScore {
    /// Create a new empty score.
    pub fn new() -> Self {
        Self {
            score: 0,
            reasons: Vec::new(),
        }
    }

    /// Add a complexity indicator with a reason.
    pub fn add(&mut self, points: u8, reason: impl Into<String>) {
        self.score = self.score.saturating_add(points);
        self.reasons.push(reason.into());
    }

    /// Returns true if the score indicates a complex task.
    pub fn is_complex(&self) -> bool {
        self.score >= thresholds::COMPLEXITY_THRESHOLD
    }

    /// Returns a summary suitable for display to the user.
    pub fn summary(&self) -> String {
        if self.reasons.is_empty() {
            "Task appears well-scoped.".to_string()
        } else {
            format!(
                "Complexity indicators (score {}): {}",
                self.score,
                self.reasons.join("; ")
            )
        }
    }

    /// Returns a soft-gate suggestion prompt for buddy to use when complexity is detected.
    ///
    /// This generates a friendly, conversational message that suggests filing as an idea
    /// rather than a task. Returns `None` if the task is not complex enough to warrant
    /// the suggestion.
    ///
    /// # Example
    ///
    /// ```
    /// use binnacle::models::complexity::analyze_complexity;
    ///
    /// let score = analyze_complexity("Explore caching options and investigate patterns", None);
    /// if let Some(suggestion) = score.soft_gate_suggestion() {
    ///     // buddy would display this suggestion to the user
    ///     println!("{}", suggestion);
    /// }
    /// ```
    pub fn soft_gate_suggestion(&self) -> Option<String> {
        if !self.is_complex() {
            return None;
        }

        let mut suggestion = String::new();

        // Opening line - acknowledge what they're trying to do
        suggestion
            .push_str("This sounds like it might be better as an **idea** rather than a task.\n\n");

        // Explain why (use the reasons we detected)
        suggestion.push_str("Here's what I noticed:\n");
        for reason in &self.reasons {
            suggestion.push_str(&format!("- {}\n", simplify_reason(reason)));
        }

        // Explain the difference between ideas and tasks
        suggestion.push_str("\n**Ideas** are great for:\n");
        suggestion.push_str("- Exploratory concepts that need discussion\n");
        suggestion.push_str("- \"What if\" thoughts that aren't fully formed\n");
        suggestion.push_str("- Work that needs decomposition before it's actionable\n");

        // Offer the choice
        suggestion.push_str("\n**What would you like to do?**\n");
        suggestion
            .push_str("1. File as an **idea** (recommended) - can be promoted to tasks later\n");
        suggestion.push_str("2. File as a **task** anyway - you know best what's needed\n");
        suggestion.push_str("3. Let me help you **break it down** into smaller tasks\n");

        Some(suggestion)
    }
}

/// Simplify technical reason strings into user-friendly language.
fn simplify_reason(reason: &str) -> String {
    // Convert technical reasons into friendlier language
    if reason.contains("conjunctions") {
        "Contains multiple concerns (might be several tasks combined)".to_string()
    } else if reason.contains("action verbs") {
        "Multiple action items detected (could be split up)".to_string()
    } else if reason.contains("exploratory") {
        "Uses exploratory language (suggests research/investigation needed)".to_string()
    } else if reason.contains("uncertainty") {
        "Contains uncertainty markers (might need more clarity first)".to_string()
    } else if reason.contains("vague scope") {
        "Scope seems broad or undefined".to_string()
    } else if reason.contains("open-ended") {
        "Has open-ended elements (suggests incomplete definition)".to_string()
    } else if reason.contains("list items") {
        "Contains a list of items (might be multiple tasks)".to_string()
    } else if reason.contains("Title is") {
        "Title is quite long (might benefit from breaking down)".to_string()
    } else if reason.contains("Description is") {
        "Description is quite long (might cover multiple concerns)".to_string()
    } else {
        reason.to_string()
    }
}

impl Default for ComplexityScore {
    fn default() -> Self {
        Self::new()
    }
}

/// Analyze a task title and optional description for complexity.
///
/// Returns a `ComplexityScore` with detected issues.
pub fn analyze_complexity(title: &str, description: Option<&str>) -> ComplexityScore {
    let mut score = ComplexityScore::new();

    // Length-based checks
    check_title_length(title, &mut score);
    if let Some(desc) = description {
        check_description_length(desc, &mut score);
    }

    // Multiple concerns in title
    check_multiple_concerns(title, &mut score);

    // Vague scope indicators
    check_vague_scope(title, description, &mut score);

    // Exploratory language
    check_exploratory_language(title, description, &mut score);

    // Structural indicators in description
    if let Some(desc) = description {
        check_structural_indicators(desc, &mut score);
    }

    score
}

/// Check if title is too long.
fn check_title_length(title: &str, score: &mut ComplexityScore) {
    if title.len() > thresholds::MAX_TITLE_LENGTH {
        score.add(
            1,
            format!(
                "Title is {} chars (max recommended: {})",
                title.len(),
                thresholds::MAX_TITLE_LENGTH
            ),
        );
    }
}

/// Check if description is too long.
fn check_description_length(description: &str, score: &mut ComplexityScore) {
    if description.len() > thresholds::MAX_DESCRIPTION_LENGTH {
        score.add(
            1,
            format!(
                "Description is {} chars (max recommended: {})",
                description.len(),
                thresholds::MAX_DESCRIPTION_LENGTH
            ),
        );
    }
}

/// Check for multiple concerns indicated by conjunctions or multiple verbs.
fn check_multiple_concerns(title: &str, score: &mut ComplexityScore) {
    let title_lower = title.to_lowercase();

    // Check for conjunctions that suggest multiple tasks
    let conjunctions = [" and ", " also ", " plus ", " & ", " as well as "];
    let found_conjunctions: Vec<&str> = conjunctions
        .iter()
        .filter(|c| title_lower.contains(*c))
        .copied()
        .collect();

    if !found_conjunctions.is_empty() {
        score.add(
            2,
            format!(
                "Title contains conjunctions suggesting multiple concerns: {:?}",
                found_conjunctions
            ),
        );
    }

    // Check for multiple action verbs at word boundaries
    let action_verbs = [
        "add",
        "fix",
        "update",
        "remove",
        "create",
        "delete",
        "implement",
        "refactor",
        "improve",
        "change",
        "modify",
        "build",
        "setup",
        "configure",
        "migrate",
        "integrate",
        "test",
    ];

    let words: Vec<&str> = title_lower.split_whitespace().collect();
    let verb_count = words
        .iter()
        .filter(|w| action_verbs.contains(&w.trim_matches(|c: char| !c.is_alphabetic())))
        .count();

    if verb_count > 1 {
        score.add(
            1,
            format!(
                "Title contains {} action verbs (suggesting multiple tasks)",
                verb_count
            ),
        );
    }
}

/// Check for vague scope indicators.
fn check_vague_scope(title: &str, description: Option<&str>, score: &mut ComplexityScore) {
    let text = match description {
        Some(desc) => format!("{} {}", title, desc),
        None => title.to_string(),
    };
    let text_lower = text.to_lowercase();

    let vague_words = [
        "various",
        "several",
        "multiple",
        "all the",
        "everything",
        "a lot of",
        "many",
        "numerous",
        "a bunch of",
    ];

    let found_vague: Vec<&str> = vague_words
        .iter()
        .filter(|w| text_lower.contains(*w))
        .copied()
        .collect();

    if !found_vague.is_empty() {
        score.add(1, format!("Contains vague scope words: {:?}", found_vague));
    }

    // Uncertainty markers
    let uncertainty_words = [
        "maybe", "might", "possibly", "probably", "perhaps", "could be",
    ];
    let found_uncertainty: Vec<&str> = uncertainty_words
        .iter()
        .filter(|w| text_lower.contains(*w))
        .copied()
        .collect();

    if !found_uncertainty.is_empty() {
        score.add(
            2,
            format!(
                "Contains uncertainty markers (consider idea instead): {:?}",
                found_uncertainty
            ),
        );
    }
}

/// Check for exploratory language that suggests an idea rather than a task.
fn check_exploratory_language(title: &str, description: Option<&str>, score: &mut ComplexityScore) {
    let text = match description {
        Some(desc) => format!("{} {}", title, desc),
        None => title.to_string(),
    };
    let text_lower = text.to_lowercase();

    let exploratory_words = [
        "explore",
        "investigate",
        "research",
        "figure out",
        "look into",
        "consider",
        "think about",
        "brainstorm",
        "prototype",
        "experiment",
        "try out",
        "see if",
        "what if",
    ];

    let found_exploratory: Vec<&str> = exploratory_words
        .iter()
        .filter(|w| text_lower.contains(*w))
        .copied()
        .collect();

    if !found_exploratory.is_empty() {
        score.add(
            2,
            format!(
                "Contains exploratory language (better as idea): {:?}",
                found_exploratory
            ),
        );
    }
}

/// Check for structural indicators of complexity in description.
fn check_structural_indicators(description: &str, score: &mut ComplexityScore) {
    // Check for open-ended markers
    let open_ended = ["etc.", "etc", "and so on", "and more", "..."];
    let text_lower = description.to_lowercase();

    let found_open: Vec<&str> = open_ended
        .iter()
        .filter(|w| text_lower.contains(*w))
        .copied()
        .collect();

    if !found_open.is_empty() {
        score.add(1, format!("Contains open-ended markers: {:?}", found_open));
    }

    // Check for list items (numbered or bulleted)
    let list_indicators = description
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            // Numbered: "1. ", "2) ", etc.
            trimmed
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit())
                && (trimmed.contains(". ") || trimmed.contains(") "))
            // Bulleted: "- ", "* ", "• "
            || trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || trimmed.starts_with("• ")
        })
        .count();

    if list_indicators >= 3 {
        score.add(
            2,
            format!(
                "Description has {} list items (consider decomposing into subtasks)",
                list_indicators
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_task_not_complex() {
        let score = analyze_complexity("Fix typo in README", None);
        assert!(!score.is_complex());
        assert!(score.reasons.is_empty());
    }

    #[test]
    fn test_long_title_adds_score() {
        let long_title = "A".repeat(100);
        let score = analyze_complexity(&long_title, None);
        assert!(score.score >= 1);
        assert!(score.reasons.iter().any(|r| r.contains("Title is")));
    }

    #[test]
    fn test_long_description_adds_score() {
        let long_desc = "A".repeat(600);
        let score = analyze_complexity("Short title", Some(&long_desc));
        assert!(score.score >= 1);
        assert!(score.reasons.iter().any(|r| r.contains("Description is")));
    }

    #[test]
    fn test_conjunction_detected() {
        let score = analyze_complexity("Add feature and fix bug", None);
        assert!(score.score >= 2);
        assert!(score.reasons.iter().any(|r| r.contains("conjunctions")));
    }

    #[test]
    fn test_multiple_verbs_detected() {
        let score = analyze_complexity("Implement add remove features", None);
        assert!(score.score >= 1);
        assert!(score.reasons.iter().any(|r| r.contains("action verbs")));
    }

    #[test]
    fn test_vague_scope_detected() {
        let score = analyze_complexity("Update various files", None);
        assert!(score.score >= 1);
        assert!(score.reasons.iter().any(|r| r.contains("vague scope")));
    }

    #[test]
    fn test_uncertainty_detected() {
        let score = analyze_complexity("Maybe add caching", None);
        assert!(score.score >= 2);
        assert!(score.reasons.iter().any(|r| r.contains("uncertainty")));
    }

    #[test]
    fn test_exploratory_language_detected() {
        let score = analyze_complexity("Explore caching options", None);
        assert!(score.score >= 2);
        assert!(score.reasons.iter().any(|r| r.contains("exploratory")));
    }

    #[test]
    fn test_open_ended_markers_detected() {
        let score = analyze_complexity("Update files", Some("Fix these: a, b, c, etc."));
        assert!(score.score >= 1);
        assert!(score.reasons.iter().any(|r| r.contains("open-ended")));
    }

    #[test]
    fn test_list_items_detected() {
        let desc = "Steps:\n1. Do A\n2. Do B\n3. Do C\n4. Do D";
        let score = analyze_complexity("Multi-step task", Some(desc));
        assert!(score.score >= 2);
        assert!(score.reasons.iter().any(|r| r.contains("list items")));
    }

    #[test]
    fn test_bulleted_list_detected() {
        let desc = "Steps:\n- Do A\n- Do B\n- Do C";
        let score = analyze_complexity("Multi-step task", Some(desc));
        assert!(score.score >= 2);
        assert!(score.reasons.iter().any(|r| r.contains("list items")));
    }

    #[test]
    fn test_highly_complex_task() {
        let title = "Add authentication and fix database and improve logging and refactor API";
        let desc = "We need to investigate various things, etc.\n1. Do A\n2. Do B\n3. Do C";
        let score = analyze_complexity(title, Some(desc));
        assert!(score.is_complex());
        assert!(score.score >= thresholds::COMPLEXITY_THRESHOLD);
    }

    #[test]
    fn test_summary_empty_for_simple() {
        let score = analyze_complexity("Fix typo", None);
        assert_eq!(score.summary(), "Task appears well-scoped.");
    }

    #[test]
    fn test_summary_shows_reasons() {
        let score = analyze_complexity("Add X and fix Y", None);
        let summary = score.summary();
        assert!(summary.contains("Complexity indicators"));
        assert!(summary.contains("conjunctions"));
    }

    #[test]
    fn test_default_impl() {
        let score = ComplexityScore::default();
        assert_eq!(score.score, 0);
        assert!(score.reasons.is_empty());
    }

    #[test]
    fn test_soft_gate_suggestion_none_for_simple() {
        let score = analyze_complexity("Fix typo in README", None);
        assert!(score.soft_gate_suggestion().is_none());
    }

    #[test]
    fn test_soft_gate_suggestion_some_for_complex() {
        let score = analyze_complexity("Explore caching options and investigate patterns", None);
        assert!(score.is_complex());
        let suggestion = score.soft_gate_suggestion();
        assert!(suggestion.is_some());
    }

    #[test]
    fn test_soft_gate_suggestion_structure() {
        let score = analyze_complexity("Add auth and fix database and improve logging", None);
        let suggestion = score.soft_gate_suggestion().unwrap();

        // Check key components are present
        assert!(suggestion.contains("better as an **idea**"));
        assert!(suggestion.contains("Here's what I noticed"));
        assert!(suggestion.contains("Ideas** are great for"));
        assert!(suggestion.contains("What would you like to do?"));
        assert!(suggestion.contains("File as an **idea**"));
        assert!(suggestion.contains("File as a **task** anyway"));
        assert!(suggestion.contains("break it down"));
    }

    #[test]
    fn test_soft_gate_suggestion_friendly_reasons() {
        // Test conjunction detection produces friendly message
        let score = analyze_complexity("Add feature and fix bug", None);
        if let Some(suggestion) = score.soft_gate_suggestion() {
            assert!(suggestion.contains("multiple concerns"));
        }

        // Test exploratory language produces friendly message
        let score = analyze_complexity("Explore caching options deeply", None);
        if let Some(suggestion) = score.soft_gate_suggestion() {
            assert!(suggestion.contains("exploratory"));
        }
    }

    #[test]
    fn test_simplify_reason() {
        assert_eq!(
            super::simplify_reason("Contains conjunctions suggesting multiple concerns"),
            "Contains multiple concerns (might be several tasks combined)"
        );
        assert_eq!(
            super::simplify_reason("Contains exploratory language (better as idea)"),
            "Uses exploratory language (suggests research/investigation needed)"
        );
        assert_eq!(
            super::simplify_reason("Something unknown"),
            "Something unknown"
        );
    }
}
