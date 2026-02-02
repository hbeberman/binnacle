//! TUI Views module
//!
//! Contains different view implementations for the TUI.

mod log_panel;
mod node_detail;
mod queue_ready;
mod recent_changes;
mod recently_completed;

pub use log_panel::{LogEntry, LogPanelView};
pub use node_detail::{EdgeInfo, NodeDetail, NodeDetailView};
pub use queue_ready::QueueReadyView;
pub use queue_ready::WorkItem;
pub use recent_changes::RecentChangesView;
pub use recently_completed::CompletedItem;
pub use recently_completed::RecentlyCompletedView;
