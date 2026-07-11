//! Framework-agnostic document engine for Strop.
//!
//! Nothing in this crate may depend on the UI shell. The hot editing path is a
//! rope (`buffer`); durable history is a Loro document (`store`, to come). The
//! shell talks to this crate through snapshots and transactions only.

pub mod buffer;
pub mod diagnose;
pub mod diff;
pub mod document;
pub mod images;
pub mod journal;
pub mod llm;
pub mod markdown;
pub mod store;
pub mod typograph;
pub mod voice;

pub use buffer::Buffer;
pub use store::Store;
