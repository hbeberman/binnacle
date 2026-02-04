//! TUI Views module
//!
//! Contains different view implementations for the TUI.

mod log_panel;
mod node_detail;
mod recently_completed;
mod work;

pub use log_panel::{LogEntry, LogPanelView};
pub use node_detail::{EdgeInfo, NodeDetail, NodeDetailView};
pub use recently_completed::CompletedItem;
pub use recently_completed::RecentlyCompletedView;
pub use work::WorkItem;
pub use work::WorkView;
