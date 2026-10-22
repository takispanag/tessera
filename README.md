# Tessera

A terminal emulator engine written from scratch in Rust.

Tessera parses the VT/xterm byte stream and maintains the screen that a
terminal would show: cells, attributes, the cursor, scroll regions, the
alternate screen and scrollback. It does not open a pty and it does not paint
pixels — a front end feeds it text and reads the grid back, which keeps the
interesting part testable without a display.

The engine has no dependencies. The escape-sequence parser, the grid, the
attribute handling and the dump format are all in this crate.

## Status

Early. The repository is being built in dependency order, and each commit
compiles and passes its tests. The README grows with the engine; the honest
state of each subsystem is in the status table at the bottom.

## Layout

| Path | Responsibility |
|---|---|
| `src/cell.rs` | cells, colour and the pen |
| `src/grid.rs` | the cell grid, scrolling and resizing |
| `src/parser.rs` | the escape-sequence state machine |
| `src/screen.rs` | the screen: cursor, pen, modes, scrollback |
| `src/csi.rs` | control-sequence dispatch |
| `src/sgr.rs` | select-graphic-rendition |
| `src/osc.rs` | operating-system commands |
| `src/dump.rs` | rendering the grid back to text |
| `src/recording.rs` | replaying a `.vt` recording |

## Licence

MIT. See [LICENSE](LICENSE).
