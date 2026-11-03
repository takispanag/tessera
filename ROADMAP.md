# Roadmap

The engine half of a terminal is here. The work below is the part that makes it
a terminal you would actually use, roughly in the order I intend to take it.

## 1. A front end

- A window that paints the grid, with a real font and a cell size derived from
  its metrics.
- Cursor rendering, including the blink state and the shape requested with
  `DECSCUSR`.
- Selection, copy, and the clipboard.
- Scrollback navigation with a scrollbar, and a search over the scrollback.

*Roughly 15 hours.*

## 2. A pty and input

- Spawning a shell on a pty, and pumping its output into the engine.
- Encoding keys into the byte stream: the escape sequences for arrows, function
  keys and modifiers, and the application-cursor mode.
- Bracketed paste and focus reporting.
- `SIGWINCH` on resize, so the shell learns the new size.

*Roughly 12 hours.*

## 3. Unicode

- The east-asian width tables, so wide characters occupy two columns.
- Combining marks and zero-width joiners, so a character is not split across
  the margin.
- Grapheme clusters, so a backspace deletes a character rather than a code
  point.

*Roughly 10 hours.*

## 4. Resize with reflow

The grid currently drops what no longer fits. Reflow means re-breaking wrapped
lines at the new width, which needs wrap points stored per line and a rule for
what to do with a line the program wrote without wrapping.

*Roughly 8 hours.*

## 5. Conformance

- A corpus of recordings under `tests/corpus/`, each with its expected dump.
- Runs of `vttest` and `esctest` against the engine, with a pass count in the
  README.
- Differential testing against xterm on a generated set of sequences.

The harness for this already exists: `recording::play` and `dump::dump`.

*Roughly 10 hours.*

## 6. Performance

- Dirty-line rendering driven by the grid's dirty flags.
- Damage tracking so a repaint touches only the rows that changed.
- A benchmark that cats a large file and measures the frame time.

*Roughly 6 hours.*

## 7. The rest of the protocol

- OSC 8 hyperlinks, kept per cell.
- OSC 4 and 10/11 palette queries.
- Mouse reporting, in both the X10 and SGR encodings.
- `DECALN`, `DECSTR` and the remaining mode resets.

*Roughly 8 hours.*
