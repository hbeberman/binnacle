//! Theme constants for binnacle GUI
//!
//! Defines the color scheme and CSS variables as Rust constants.
//! These values match the CSS custom properties in index.html.

/// Background colors
pub mod background {
    /// Primary background color (darkest)
    pub const PRIMARY: &str = "#1a2332";
    /// Secondary background color
    pub const SECONDARY: &str = "#243447";
    /// Tertiary background color
    pub const TERTIARY: &str = "#2d4059";
    /// Overlay background with transparency
    pub const OVERLAY: &str = "rgba(0, 0, 0, 0.85)";
}

/// Text colors
pub mod text {
    /// Primary text color
    pub const PRIMARY: &str = "#e8edf3";
    /// Secondary text color (muted)
    pub const SECONDARY: &str = "#b8c5d6";
}

/// Accent colors
pub mod accent {
    /// Blue accent color
    pub const BLUE: &str = "#4a90e2";
    /// Light blue accent color
    pub const LIGHT: &str = "#6aa8f0";
}

/// Border colors
pub mod border {
    /// Default border color
    pub const DEFAULT: &str = "#3a4d66";
}

/// Status colors
pub mod status {
    /// Success color (green)
    pub const SUCCESS: &str = "#5cb85c";
    /// Warning color (orange)
    pub const WARNING: &str = "#f0ad4e";
    /// Danger color (red)
    pub const DANGER: &str = "#d9534f";
    /// Info color (light blue)
    pub const INFO: &str = "#5bc0de";
}

/// Edge type colors
pub mod edge {
    /// Blocking/depends_on edges
    pub const BLOCKING: &str = "#e85d5d";
    /// Informational edges (related_to)
    pub const INFORMATIONAL: &str = "#7a8fa3";
    /// Fixes edges
    pub const FIXES: &str = "#5cb85c";
    /// Hierarchy edges (child_of)
    pub const HIERARCHY: &str = "#9b6ed8";
    /// Default edge color
    pub const DEFAULT: &str = "#3a4d66";
    /// Queued edges
    pub const QUEUED: &str = "#20b2aa";
    /// Agent working_on edges (yellow)
    pub const AGENT: &str = "#f0c040";
    /// Agent worked_on edges (historical)
    pub const AGENT_PAST: &str = "#6b7a8a";
    /// Pinned edges
    pub const PINNED: &str = "#5cb85c";
    /// Documents edges
    pub const DOCUMENTS: &str = "#4a90e2";
    /// Impacts edges (bug impacts)
    pub const IMPACTS: &str = "#e85d5d";
}

/// Task node colors by status
pub mod task {
    /// Pending task color
    pub const PENDING: &str = "#4a6fa5";
    /// In-progress task color
    pub const IN_PROGRESS: &str = "#4a90e2";
    /// Blocked task color
    pub const BLOCKED: &str = "#8b5a2b";
    /// Done task color
    pub const DONE: &str = "#5cb85c";
}

/// Bug node colors by status
pub mod bug {
    /// Pending bug color
    pub const PENDING: &str = "#e07878";
    /// In-progress bug color
    pub const IN_PROGRESS: &str = "#d95050";
    /// Blocked bug color
    pub const BLOCKED: &str = "#b33a3a";
    /// Done bug color
    pub const DONE: &str = "#8fbc8f";
}

/// Idea node colors by status
pub mod idea {
    /// Pending (seed) idea color
    pub const PENDING: &str = "#8b5fc9";
    /// In-progress (exploring) idea color
    pub const IN_PROGRESS: &str = "#7a4db8";
    /// Blocked idea color
    pub const BLOCKED: &str = "#5c3a8a";
    /// Done (promoted/archived) idea color
    pub const DONE: &str = "#8fbc8f";
}

/// Queue node color
pub mod queue {
    /// Queue node color (teal)
    pub const COLOR: &str = "#20b2aa";
    /// Light queue color
    pub const LIGHT: &str = "#40d0c8";
}

/// Agent node colors by status
pub mod agent {
    /// Active agent color (bright cyan)
    pub const ACTIVE: &str = "#00d4ff";
    /// Idle agent color
    pub const IDLE: &str = "#6bb3c9";
    /// Stale agent color
    pub const STALE: &str = "#4a6670";
}

/// Doc node colors by type
pub mod doc {
    /// PRD document color (blue)
    pub const PRD: &str = "#4a90e2";
    /// Note document color (gold)
    pub const NOTE: &str = "#e8b84a";
    /// Handoff document color (orange)
    pub const HANDOFF: &str = "#e87d4a";
}

/// Get CSS color for a node based on entity type and status
pub fn node_color(entity_type: &str, status: &str, doc_type: Option<&str>) -> &'static str {
    match entity_type {
        "task" | "milestone" => match status {
            "pending" => task::PENDING,
            "in_progress" | "inprogress" => task::IN_PROGRESS,
            "blocked" => task::BLOCKED,
            "done" | "cancelled" => task::DONE,
            _ => task::PENDING,
        },
        "bug" => match status {
            "pending" => bug::PENDING,
            "in_progress" | "inprogress" => bug::IN_PROGRESS,
            "blocked" => bug::BLOCKED,
            "done" | "cancelled" => bug::DONE,
            _ => bug::PENDING,
        },
        "idea" => match status {
            "seed" | "pending" => idea::PENDING,
            "exploring" | "in_progress" | "inprogress" => idea::IN_PROGRESS,
            "blocked" => idea::BLOCKED,
            "promoted" | "archived" | "done" => idea::DONE,
            _ => idea::PENDING,
        },
        "queue" => queue::COLOR,
        "agent" => match status {
            "active" => agent::ACTIVE,
            "idle" => agent::IDLE,
            "stale" => agent::STALE,
            _ => agent::IDLE,
        },
        "doc" => match doc_type {
            Some("prd") => doc::PRD,
            Some("note") => doc::NOTE,
            Some("handoff") => doc::HANDOFF,
            _ => doc::NOTE,
        },
        _ => background::TERTIARY,
    }
}

/// Get CSS color for an edge based on edge type
pub fn edge_color(edge_type: &str) -> &'static str {
    match edge_type {
        "depends_on" | "blocks" => edge::BLOCKING,
        "related_to" => edge::INFORMATIONAL,
        "fixes" => edge::FIXES,
        "child_of" | "parent_of" => edge::HIERARCHY,
        "queued" => edge::QUEUED,
        "working_on" => edge::AGENT,
        "worked_on" => edge::AGENT_PAST,
        "pinned" => edge::PINNED,
        "documents" | "attached_to" => edge::DOCUMENTS,
        "impacts" => edge::IMPACTS,
        _ => edge::DEFAULT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_color_task() {
        assert_eq!(node_color("task", "pending", None), task::PENDING);
        assert_eq!(node_color("task", "in_progress", None), task::IN_PROGRESS);
        assert_eq!(node_color("task", "blocked", None), task::BLOCKED);
        assert_eq!(node_color("task", "done", None), task::DONE);
    }

    #[test]
    fn test_node_color_bug() {
        assert_eq!(node_color("bug", "pending", None), bug::PENDING);
        assert_eq!(node_color("bug", "in_progress", None), bug::IN_PROGRESS);
    }

    #[test]
    fn test_node_color_doc() {
        assert_eq!(node_color("doc", "pending", Some("prd")), doc::PRD);
        assert_eq!(node_color("doc", "pending", Some("note")), doc::NOTE);
        assert_eq!(node_color("doc", "pending", Some("handoff")), doc::HANDOFF);
    }

    #[test]
    fn test_edge_color() {
        assert_eq!(edge_color("depends_on"), edge::BLOCKING);
        assert_eq!(edge_color("child_of"), edge::HIERARCHY);
        assert_eq!(edge_color("queued"), edge::QUEUED);
        assert_eq!(edge_color("unknown"), edge::DEFAULT);
    }
}
