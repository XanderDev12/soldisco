//! Typed boundaries for supervised background work.
//!
//! These modules intentionally define messages and configuration only. Live
//! tasks are introduced with their corresponding milestone, so this foundation
//! cannot accidentally report a collector as running.

pub mod collector;
pub mod projection;
pub mod raydium;
pub mod recovery;
pub mod retention;
pub mod screening;
