//! Rendering the grid back to text.
//!
//! A dump is the screen as plain text: one line per row, trailing blanks
//! removed, no attributes. It is the format the conformance tests compare,
//! because a grid of characters is the part of a terminal that can be checked
//! without a display.

use crate::screen::Screen;

/// Renders the visible screen as text, one line per row.
pub fn dump(screen: &Screen) -> String {
    let mut out = String::new();
    for y in 0..screen.rows() {
        if y > 0 {
            out.push('\n');
        }
        out.push_str(&screen.line(y));
    }
    out.trim_end_matches('\n').to_string()
}

/// Renders the scrollback above the visible screen.
pub fn dump_scrollback(screen: &Screen) -> Vec<String> {
    (0..screen.scrollback().len())
        .filter_map(|i| screen.scrollback_line(i))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_screen_dumps_as_empty_lines() {
        let screen = Screen::new(4, 3);
        assert_eq!(dump(&screen), "");
    }

    #[test]
    fn written_rows_appear_in_order() {
        let mut screen = Screen::new(6, 3);
        screen.feed("one\r\ntwo\r\nthree");
        assert_eq!(dump(&screen), "one\ntwo\nthree");
    }

    #[test]
    fn trailing_blank_rows_are_dropped() {
        let mut screen = Screen::new(4, 4);
        screen.feed("x");
        assert_eq!(dump(&screen), "x");
    }

    #[test]
    fn scrollback_dumps_oldest_first() {
        let mut screen = Screen::new(3, 1);
        screen.feed("a\r\nb\r\nc");
        assert_eq!(dump_scrollback(&screen), vec!["a", "b"]);
    }
}
