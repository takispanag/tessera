//! Control-sequence dispatch.
//!
//! The parser hands over one sequence at a time; this module decides what it
//! means. Sequences are grouped the way ECMA-48 groups them: cursor movement,
//! editing, attributes, modes, scrolling and device control.

use crate::cell::Pen;
use crate::parser::Csi;
use crate::screen::Screen;

/// Applies a control sequence to the screen.
pub(crate) fn dispatch(screen: &mut Screen, csi: Csi) {
    // Modes are the one group that reads the private marker.
    if let Some('?') = csi.private {
        match csi.final_byte {
            'h' => set_modes(screen, &csi, true),
            'l' => set_modes(screen, &csi, false),
            _ => {}
        }
        return;
    }
    if csi.private.is_some() || csi.intermediate.is_some() {
        return;
    }
    let n = i32::from(csi.param(0, 1));
    let (x, y) = {
        let c = screen.cursor;
        (i32::from(c.x), i32::from(c.y))
    };
    match csi.final_byte {
        // Cursor up, down, right, left.
        'A' => screen.move_to(x, y - n),
        'B' => screen.move_to(x, y + n),
        'C' => screen.move_to(x + n, y),
        'D' => screen.move_to(x - n, y),
        // Next and previous line: move vertically and return to column 0.
        'E' => screen.move_to(0, y + n),
        'F' => screen.move_to(0, y - n),
        // Absolute column and absolute row.
        'G' | '`' => screen.move_to(n - 1, y),
        'd' => {
            let base = screen.origin_base();
            screen.move_to(x, base + n - 1);
        }
        // Cursor position, row first, counted from the origin.
        'H' | 'f' => {
            let base = screen.origin_base();
            let row = i32::from(csi.param(0, 1));
            let col = i32::from(csi.param(1, 1));
            screen.move_to(col - 1, base + row - 1);
        }
        // Tab stops.
        'I' => screen.tab_forward(csi.param(0, 1)),
        'Z' => screen.tab_backward(csi.param(0, 1)),
        'g' => screen.clear_tab_stops(csi.param(0, 0)),
        // Scroll region, from one-based rows with the bottom defaulting to the
        // last row. Setting it homes the cursor.
        'r' => {
            let bottom = csi.param(1, u16::MAX);
            let bottom = if bottom == u16::MAX { screen.rows() } else { bottom };
            screen.set_scroll_region(csi.param(0, 1), bottom);
        }
        // Save and restore the cursor and the pen.
        's' => screen.save_cursor(),
        'u' => screen.restore_cursor(),
        // Select graphic rendition: colour and attributes.
        'm' => crate::sgr::apply(&mut screen.pen, &csi.params),
        // Erase in display: 0 to the end, 1 from the start, 2 the whole screen.
        'J' => erase_in_display(screen, csi.param(0, 0)),
        // Erase in line: 0 to the end, 1 from the start, 2 the whole line.
        'K' => erase_in_line(screen, csi.param(0, 0)),
        // Erase, insert and delete characters.
        'X' => screen.erase_chars(csi.param(0, 1)),
        '@' => screen.insert_chars(csi.param(0, 1)),
        'P' => screen.delete_chars(csi.param(0, 1)),
        // Insert and delete lines, and scroll the region.
        'L' => screen.insert_lines(csi.param(0, 1)),
        'M' => screen.delete_lines(csi.param(0, 1)),
        'S' => screen.scroll_up(csi.param(0, 1)),
        'T' => screen.scroll_down(csi.param(0, 1)),
        _ => {}
    }
}

fn set_modes(screen: &mut Screen, csi: &Csi, enable: bool) {
    for param in &csi.params {
        if let Some(mode) = param {
            screen.set_mode(*mode, enable);
        }
    }
}

fn erase_in_display(screen: &mut Screen, mode: u16) {
    let pen = Pen::default();
    let (cx, cy) = (screen.cursor.x, screen.cursor.y);
    let last_row = screen.rows() - 1;
    let last_col = screen.cols() - 1;
    match mode {
        1 => {
            if cy > 0 {
                screen.grid.clear_rows(0, cy - 1, pen);
            }
            screen.grid.erase_cells(cy, 0, cx, pen);
        }
        2 | 3 => screen.grid.clear(),
        _ => {
            screen.grid.erase_cells(cy, cx, last_col, pen);
            if cy < last_row {
                screen.grid.clear_rows(cy + 1, last_row, pen);
            }
        }
    }
}

fn erase_in_line(screen: &mut Screen, mode: u16) {
    let pen = Pen::default();
    let (cx, cy) = (screen.cursor.x, screen.cursor.y);
    let last_col = screen.cols() - 1;
    match mode {
        1 => screen.grid.erase_cells(cy, 0, cx, pen),
        2 => screen.grid.erase_cells(cy, 0, last_col, pen),
        _ => screen.grid.erase_cells(cy, cx, last_col, pen),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::Point;

    fn at(input: &str) -> Point {
        let mut screen = Screen::new(10, 5);
        screen.feed(input);
        screen.cursor()
    }

    fn shown(input: &str) -> Vec<String> {
        let mut screen = Screen::new(8, 3);
        screen.feed(input);
        (0..3).map(|y| screen.line(y)).collect()
    }

    #[test]
    fn absolute_positioning_is_one_based() {
        assert_eq!(at("\x1b[2;3H"), Point::new(2, 1));
        assert_eq!(at("\x1b[H"), Point::new(0, 0));
    }

    #[test]
    fn relative_movement_clamps_at_the_edges() {
        assert_eq!(at("\x1b[9A"), Point::new(0, 0));
        assert_eq!(at("\x1b[99B"), Point::new(0, 4));
        assert_eq!(at("\x1b[99C"), Point::new(9, 0));
        assert_eq!(at("\x1b[99D"), Point::new(0, 0));
    }

    #[test]
    fn next_and_previous_line_return_to_the_first_column() {
        assert_eq!(at("abc\x1b[E"), Point::new(0, 1));
        assert_eq!(at("\r\n\r\nabc\x1b[F"), Point::new(0, 1));
    }

    #[test]
    fn absolute_column_and_row() {
        assert_eq!(at("\x1b[4G"), Point::new(3, 0));
        assert_eq!(at("\x1b[3d"), Point::new(0, 2));
    }

    #[test]
    fn movement_clears_a_pending_wrap() {
        let mut screen = Screen::new(4, 2);
        screen.feed("abcd");
        screen.feed("\x1b[2G");
        screen.feed("X");
        assert_eq!(screen.line(0), "aXcd");
    }

    #[test]
    fn unknown_sequences_are_ignored() {
        assert_eq!(at("\x1b[99z"), Point::new(0, 0));
    }

    #[test]
    fn erasing_the_display_from_the_cursor() {
        let out = shown("aaaaaaaa\r\nbbbbbbbb\r\ncccccccc\x1b[2;3H\x1b[J");
        assert_eq!(out, vec!["aaaaaaaa", "bb", ""]);
    }

    #[test]
    fn erasing_the_display_to_the_cursor() {
        let out = shown("aaaaaaaa\r\nbbbbbbbb\r\ncccccccc\x1b[2;3H\x1b[1J");
        assert_eq!(out, vec!["", "   bbbbb", "cccccccc"]);
    }

    #[test]
    fn erasing_the_whole_display() {
        let out = shown("aaaa\x1b[2J");
        assert_eq!(out, vec!["", "", ""]);
    }

    #[test]
    fn erasing_to_the_end_of_a_line() {
        let out = shown("abcdefgh\x1b[1;3H\x1b[K");
        assert_eq!(out[0], "ab");
    }

    #[test]
    fn erasing_to_the_start_of_a_line() {
        let out = shown("abcdefgh\x1b[1;3H\x1b[1K");
        assert_eq!(out[0], "   defgh");
    }

    #[test]
    fn erasing_characters_leaves_the_rest_in_place() {
        let out = shown("abcdefgh\x1b[1;2H\x1b[2X");
        assert_eq!(out[0], "a  defgh");
    }

    #[test]
    fn inserting_and_deleting_characters_shift_the_line() {
        assert_eq!(shown("abcdefgh\x1b[1;3H\x1b[2@")[0], "ab  cdef");
        assert_eq!(shown("abcdefgh\x1b[1;3H\x1b[2P")[0], "abefgh");
    }

    #[test]
    fn inserting_and_deleting_lines_scroll_the_region() {
        assert_eq!(
            shown("a\r\nb\r\nc\x1b[1;1H\x1b[L"),
            vec!["", "a", "b"]
        );
        assert_eq!(
            shown("a\r\nb\r\nc\x1b[1;1H\x1b[M"),
            vec!["b", "c", ""]
        );
    }

    #[test]
    fn scrolling_the_region() {
        assert_eq!(shown("a\r\nb\r\nc\x1b[S"), vec!["b", "c", ""]);
        assert_eq!(shown("a\r\nb\r\nc\x1b[T"), vec!["", "a", "b"]);
    }
}
