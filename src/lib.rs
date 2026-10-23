//! Tessera — a terminal emulator engine written from scratch.
//!
//! A terminal is a small program with a large specification: a byte stream
//! arrives, escape sequences are parsed out of it, and the result is a grid of
//! cells that something else paints on a screen. This crate is the engine half
//! of that: the parser, the screen model, the cursor and the scrollback.
//!
//! Nothing here depends on another terminal library. The parser, the grid, the
//! attribute handling and the dump format are all implemented in this crate,
//! and it has no dependencies at all.
//!
//! # Scope
//!
//! The engine models the screen; it does not open a pty or paint pixels. A
//! front end feeds it text and reads the grid back, which keeps the
//! interesting part testable without a display.

pub mod cell;

pub use cell::{Attributes, Cell, Color, Pen};

/// Engine version, taken from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
