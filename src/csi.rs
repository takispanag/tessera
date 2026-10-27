//! Control-sequence dispatch.
//!
//! The parser hands over one sequence at a time; this module decides what it
//! means. Sequences are grouped the way ECMA-48 groups them: cursor movement,
//! editing, attributes, modes, scrolling and device control. This commit covers
//! the cursor group.

use crate::parser::Csi;
use crate::screen::Screen;

/// Applies a control sequence to the screen.
pub(crate) fn dispatch(screen: &mut Screen, csi: Csi) {
    // Private sequences belong to the mode group, which is handled later.
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
        'd' => screen.move_to(x, n - 1),
        // Cursor position, row first.
        'H' | 'f' => {
            let row = i32::from(csi.param(0, 1));
            let col = i32::from(csi.param(1, 1));
            screen.move_to(col - 1, row - 1);
        }
        _ => {}
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
        assert_eq!(at("\n\nabc\x1b[F"), Point::new(0, 1));
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
}
