//! Alpaca Markets trading library.
//!
//! Provides an async REST client, real-time WebSocket stream abstractions, and
//! all supporting domain types for building Alpaca trading applications in Rust.

#![deny(missing_docs)]

pub mod client;
pub mod commands;
pub mod config;
pub mod events;
pub mod logging;
pub mod stream;
pub mod types;
