//! Framework-agnostic document engine for Strop.
//!
//! Nothing in this crate may depend on the UI shell. The hot editing path is a
//! rope (`buffer`); durable history is a Loro document (`store`, to come). The
//! shell talks to this crate through snapshots and transactions only.

pub mod buffer;
pub mod document;
pub mod store;
pub mod typograph;

pub use buffer::Buffer;
pub use store::Store;
