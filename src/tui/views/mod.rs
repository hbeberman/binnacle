//! TUI Views module
//!
//! Contains different view implementations for the TUI.

mod node_detail;
mod queue_ready;
mod recently_completed;

pub use node_detail::{EdgeInfo, NodeDetail, NodeDetailView};
pub use queue_ready::QueueReadyView;
pub use queue_ready::WorkItem;
pub use recently_completed::CompletedItem;
pub use recently_completed::RecentlyCompletedView;
