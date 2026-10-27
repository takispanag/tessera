//! The screen: the grid plus everything the escape stream can change.
//!
//! This is the layer that knows the specification. The parser hands it one
//! action at a time; the grid stores the result. Everything a program can do to
//! a terminal — printing, moving, erasing, scrolling, setting attributes,
//! switching to the alternate screen — is a method here.

use crate::cell::{Cell, Pen};
use crate::grid::Grid;
use crate::parser::{Action, Csi, Parser};

/// A position on the screen, counted from the top-left corner.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Point {
    pub x: u16,
    pub y: u16,
}

impl Point {
    pub fn new(x: u16, y: u16) -> Self {
        Point { x, y }
    }
}

/// A terminal screen.
pub struct Screen {
    grid: Grid,
    cursor: Point,
    pen: Pen,
    parser: Parser,
    scroll_top: u16,
    scroll_bottom: u16,
    autowrap: bool,
    wrap_pending: bool,
}

impl Screen {
    /// A blank screen of `cols` × `rows` with the cursor at the top left.
    pub fn new(cols: u16, rows: u16) -> Self {
        let rows = rows.max(1);
        let cols = cols.max(1);
        Screen {
            grid: Grid::new(cols, rows),
            cursor: Point::default(),
            pen: Pen::default(),
            parser: Parser::new(),
            scroll_top: 0,
            scroll_bottom: rows - 1,
            autowrap: true,
            wrap_pending: false,
        }
    }

    /// The number of columns.
    pub fn cols(&self) -> u16 {
        self.grid.cols()
    }

    /// The number of rows.
    pub fn rows(&self) -> u16 {
        self.grid.rows()
    }

    /// The cursor position.
    pub fn cursor(&self) -> Point {
        self.cursor
    }

    /// The current pen.
    pub fn pen(&self) -> Pen {
        self.pen
    }

    /// The grid, for reading cells back.
    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    /// One row as text, with trailing blanks removed.
    pub fn line(&self, y: u16) -> String {
        let mut out = String::new();
        for cell in self.grid.row(y) {
            if cell.is_continuation() {
                continue;
            }
            out.push(cell.ch);
        }
        out.trim_end().to_string()
    }

    /// Feeds text into the screen.
    pub fn feed(&mut self, input: &str) {
        for c in input.chars() {
            if let Some(action) = self.parser.advance(c) {
                self.apply(action);
            }
        }
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::Print(c) => self.print(c),
            Action::Execute(c) => self.control(c),
            Action::Csi(csi) => self.csi(csi),
            Action::Esc(_) => {}
            Action::Osc(_) => {}
        }
    }

    fn print(&mut self, c: char) {
        if self.wrap_pending {
            self.wrap_pending = false;
            if self.autowrap {
                self.cursor.x = 0;
                self.index();
            }
        }
        let width = u16::from(char_width(c));
        if width == 0 {
            return;
        }
        if self.cursor.x + width > self.cols() {
            self.cursor.x = 0;
            self.index();
        }
        let cell = Cell::new(c, self.pen, width as u8);
        self.grid.set(self.cursor.x, self.cursor.y, cell);
        if width == 2 {
            self.grid
                .set(self.cursor.x + 1, self.cursor.y, Cell::continuation(self.pen));
        }
        self.cursor.x += width;
        if self.cursor.x >= self.cols() {
            self.cursor.x = self.cols() - 1;
            self.wrap_pending = true;
        }
    }

    /// Moves the cursor down one line, scrolling the region at the bottom.
    fn index(&mut self) {
        if self.cursor.y == self.scroll_bottom {
            self.grid.scroll_up(
                self.scroll_top,
                self.scroll_bottom,
                1,
                Pen::default(),
            );
        } else if self.cursor.y + 1 < self.rows() {
            self.cursor.y += 1;
        }
    }

    fn control(&mut self, c: char) {
        match c {
            '\n' | '\x0b' | '\x0c' => {
                self.wrap_pending = false;
                self.index();
            }
            '\r' => {
                self.cursor.x = 0;
                self.wrap_pending = false;
            }
            '\x08' => {
                self.cursor.x = self.cursor.x.saturating_sub(1);
                self.wrap_pending = false;
            }
            '\t' => {
                self.wrap_pending = false;
                let next = (self.cursor.x / 8 + 1) * 8;
                self.cursor.x = next.min(self.cols() - 1);
            }
            _ => {}
        }
    }

    fn csi(&mut self, _csi: Csi) {
        // Control sequences arrive in the next commits.
    }
}

/// The number of columns a character occupies.
///
/// A minimal width table for now: the full Unicode east-asian width data
/// arrives with the wide-character support.
fn char_width(c: char) -> u8 {
    if c.is_control() {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Screen {
        Screen::new(10, 3)
    }

    #[test]
    fn printing_advances_the_cursor() {
        let mut screen = screen();
        screen.feed("ab");
        assert_eq!(screen.line(0), "ab");
        assert_eq!(screen.cursor(), Point::new(2, 0));
    }

    #[test]
    fn carriage_return_rewinds_the_column() {
        let mut screen = screen();
        screen.feed("abc\rX");
        assert_eq!(screen.line(0), "Xbc");
    }

    #[test]
    fn backspace_stops_at_the_margin() {
        let mut screen = screen();
        screen.feed("ab\x08\x08\x08X");
        assert_eq!(screen.line(0), "Xb");
        assert_eq!(screen.cursor().x, 1);
    }

    #[test]
    fn line_feed_keeps_the_column() {
        let mut screen = Screen::new(5, 3);
        screen.feed("ab\nc");
        assert_eq!(screen.line(0), "ab");
        assert_eq!(screen.line(1), "  c");
    }

    #[test]
    fn newline_moves_down_and_scrolls_at_the_bottom() {
        let mut screen = Screen::new(5, 2);
        screen.feed("a\r\nb\r\nc");
        assert_eq!(screen.line(0), "b");
        assert_eq!(screen.line(1), "c");
    }

    #[test]
    fn text_wraps_at_the_right_margin() {
        let mut screen = Screen::new(4, 3);
        screen.feed("abcdef");
        assert_eq!(screen.line(0), "abcd");
        assert_eq!(screen.line(1), "ef");
    }

    #[test]
    fn wrapping_scrolls_when_the_last_line_is_reached() {
        let mut screen = Screen::new(3, 2);
        screen.feed("abcdefg");
        assert_eq!(screen.line(0), "def");
        assert_eq!(screen.line(1), "g");
    }

    #[test]
    fn tab_moves_to_the_next_stop() {
        let mut screen = Screen::new(20, 1);
        screen.feed("a\tb");
        assert_eq!(screen.line(0), "a       b");
    }

    #[test]
    fn trailing_blanks_are_not_part_of_a_line() {
        let mut screen = screen();
        screen.feed("hi   ");
        assert_eq!(screen.line(0), "hi");
    }

    #[test]
    fn a_zero_sized_screen_is_coerced_to_one_cell() {
        let screen = Screen::new(0, 0);
        assert_eq!(screen.cols(), 1);
        assert_eq!(screen.rows(), 1);
    }
}
