//! Supervised background-work boundaries for the modular server.
//!
//! Collector, pipeline, discovery, normalization, and maintenance are active.
//! The remaining small modules reserve typed boundaries for later milestones.

pub mod collector;
pub mod discovery;
pub mod discovery_rpc;
pub mod intake;
pub mod maintenance;
pub mod normalization;
pub mod pending_activity;
pub mod pipeline;
pub mod projection;
pub mod pump_swap_pair;
pub mod qualification;
pub mod raydium;
pub mod recovery;
pub mod screening;
