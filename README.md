# Tessera

A terminal emulator engine written from scratch in Rust.

Tessera takes the VT/xterm byte stream and keeps the screen a terminal would
show: cells, attributes, the cursor, scroll regions, tab stops, the alternate
screen and scrollback. It does not open a pty and it does not paint pixels — a
front end feeds it text and reads the grid back, which keeps the interesting
part testable without a display.

The engine has no dependencies. The parser, the grid, the attribute handling
and the dump format are all in this crate.

```
┌──────────────────────────────┐
│  tessera (library)           │   zero dependencies
│  parser · screen · dump      │
└──────────┬───────────────────┘
           │
    ┌──────┴───────┐
    │              │
┌───▼──────┐  ┌────▼─────────────┐
│ tessera- │  │ a front end      │
│ dump     │  │ (not written yet)│
└──────────┘  └──────────────────┘
```

## Status

This is the engine, not a terminal you can run a shell in. What exists is
tested; what does not exist is in [ROADMAP.md](ROADMAP.md).

| Area | State |
|---|---|
| Escape-sequence parser | done: ECMA-48 states, CSI parameters, OSC, DCS swallow |
| Cursor movement and addressing | done: CUP, HVP, CUU/CUD/CUF/CUB, CNL/CPL, CHA/VPA |
| Erasing and editing | done: ED, EL, ECH, ICH, DCH, IL, DL, SU, SD |
| Attributes and colour | done: 16, 256 and true colour, the eight attributes |
| Scroll region and origin mode | done: DECSTBM, DECOM, IND, RI, NEL |
| Private modes | done: DECAWM, DECTCEM, DECOM, alternate screen |
| Tab stops | done: HTS, TBC, CHT, CBT |
| Alternate screen and scrollback | done |
| Window title | done for OSC 0 and 2; the rest of OSC is ignored |
| Wide characters and combining marks | not started: every character is one column |
| Pty, input encoding, `SIGWINCH` | not started |
| Rendering, selection, clipboard | not started |
| Resize reflow | not started |
| `vttest` / `esctest` conformance | harness exists, corpus does not |

## Try it

Requires a recent stable Rust.

```bash
# Play the example recording and print the resulting screen
cargo run --bin tessera-dump -- --size 40x8 examples/hello.vt
```

Recordings are plain text so they can be reviewed in a diff. `\e` is the escape
byte, `\r` and `\n` are themselves, and anything else can be written as a hex
escape:

```
\e[1;32mTessera\e[0m is a terminal engine.\r\n
```

## Using the engine

```rust
use tessera::{dump, Screen};

let mut screen = Screen::new(20, 3);
screen.feed("hello\r\nworld");
assert_eq!(dump(&screen), "hello\nworld");
```

A front end keeps a `Screen`, calls `feed` with whatever arrived from the pty,
and reads `grid()`, `cursor()`, `title()` and `scrollback()` when painting.

## Layout

| Path | Responsibility |
|---|---|
| `src/cell.rs` | cells, colour and the pen |
| `src/grid.rs` | the cell grid, scrolling, erasing and resizing |
| `src/parser.rs` | the escape-sequence state machine |
| `src/screen.rs` | the screen: cursor, pen, modes, buffers, scrollback |
| `src/csi.rs` | control-sequence dispatch |
| `src/sgr.rs` | select graphic rendition |
| `src/dump.rs` | rendering the grid back to text |
| `src/recording.rs` | replaying a `.vt` recording |
| `src/bin/tessera-dump.rs` | the command-line harness |

## Decisions worth knowing about

- **The cursor parks at the right margin.** Printing the last character of a
  line does not wrap; the wrap happens when the next character arrives. A line
  that exactly fills the screen therefore does not scroll.
- **Scrolling snapshots its region.** Moving rows up or down overlaps, so the
  region is copied before it is written; scrolling by more than the region
  clears it rather than wrapping.
- **The engine does not trust its input.** Coordinates outside the grid read as
  blanks and writes are ignored, and a parameter list that overflows saturates
  instead of wrapping. A hostile or broken stream cannot take the engine down.
- **The alternate screen is a second grid.** Lines that leave the top of the
  main screen go to a bounded scrollback; the alternate screen never writes to
  it, because a full-screen program redrawing itself is not history.

## Testing

```bash
cargo test --workspace
```

The suite covers the parser state machine (including interrupted sequences and
saturating parameters), grid geometry and scrolling, cursor addressing and
clamping, every erase and edit operation, the attribute forms, scroll regions,
modes and tab stops, the alternate screen and scrollback, and the recording
harness end to end.

## Licence

MIT. See [LICENSE](LICENSE).
