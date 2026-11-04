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
//! # Example
//!
//! ```
//! use tessera::{dump, Screen};
//!
//! let mut screen = Screen::new(20, 3);
//! screen.feed("hello\r\nworld");
//! assert_eq!(screen.line(0), "hello");
//! assert_eq!(dump(&screen), "hello\nworld");
//! ```
//!
//! # Layout
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`cell`] | cells, colour and the pen |
//! | [`grid`] | the cell grid, scrolling, erasing and resizing |
//! | [`parser`] | the escape-sequence state machine |
//! | [`screen`] | the screen: cursor, pen, modes, buffers, scrollback |
//! | [`dump`] | rendering the grid back to text |
//! | [`recording`] | replaying a `.vt` recording |
//!
//! # Scope
//!
//! The engine models the screen; it does not open a pty or paint pixels. A
//! front end feeds it text and reads the grid back, which keeps the interesting
//! part testable without a display. The work still ahead is listed in
//! `ROADMAP.md`.

mod csi;
mod sgr;

pub mod cell;
pub mod dump;
pub mod grid;
pub mod parser;
pub mod recording;
pub mod screen;

pub use cell::{Attributes, Cell, Color, Pen};
pub use dump::dump;
pub use grid::Grid;
pub use parser::{Action, Csi, Parser};
pub use screen::{Point, Screen};

/// Engine version, taken from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
