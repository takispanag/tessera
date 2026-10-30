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
    pub(crate) grid: Grid,
    pub(crate) cursor: Point,
    pub(crate) pen: Pen,
    pub(crate) parser: Parser,
    pub(crate) scroll_top: u16,
    pub(crate) scroll_bottom: u16,
    pub(crate) autowrap: bool,
    pub(crate) wrap_pending: bool,
    pub(crate) origin_mode: bool,
    saved_cursor: Option<(Point, Pen)>,
}

impl Screen {
    /// A blank screen of `cols` Γ— `rows` with the cursor at the top left.
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
            origin_mode: false,
            saved_cursor: None,
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

    /// The scroll region as inclusive zero-based rows.
    pub fn scroll_region(&self) -> (u16, u16) {
        (self.scroll_top, self.scroll_bottom)
    }

    /// Sets the scroll region from inclusive, one-based rows.
    ///
    /// A region that is degenerate — either end outside the screen, or the top
    /// at or below the bottom — is ignored, which is what xterm does rather
    /// than leaving the screen in a state where scrolling cannot happen.
    pub(crate) fn set_scroll_region(&mut self, top: u16, bottom: u16) {
        if top == 0 || bottom == 0 {
            return;
        }
        let top = top - 1;
        let bottom = bottom.min(self.rows()) - 1;
        if top >= bottom || bottom >= self.rows() {
            return;
        }
        self.scroll_top = top;
        self.scroll_bottom = bottom;
        self.move_to(0, 0);
    }

    /// The cursor moves up one line, scrolling the region down at the top.
    fn reverse_index(&mut self) {
        if self.cursor.y == self.scroll_top {
            self.grid
                .scroll_down(self.scroll_top, self.scroll_bottom, 1, Pen::default());
        } else {
            self.cursor.y = self.cursor.y.saturating_sub(1);
        }
    }

    /// Remembers the cursor and the pen.
    pub(crate) fn save_cursor(&mut self) {
        self.saved_cursor = Some((self.cursor, self.pen));
    }

    /// Restores the cursor and the pen, if anything was saved.
    pub(crate) fn restore_cursor(&mut self) {
        if let Some((point, pen)) = self.saved_cursor {
            self.cursor = point;
            self.pen = pen;
            self.wrap_pending = false;
        }
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
            Action::Esc(c) => self.escape(c),
            Action::Osc(_) => {}
        }
    }

    /// Handles the simple escape sequences.
    fn escape(&mut self, c: char) {
        match c {
            'D' => {
                self.wrap_pending = false;
                self.index();
            }
            'E' => {
                self.cursor.x = 0;
                self.wrap_pending = false;
                self.index();
            }
            'M' => {
                self.wrap_pending = false;
                self.reverse_index();
            }
            '7' => self.save_cursor(),
            '8' => self.restore_cursor(),
            'c' => self.reset(),
            _ => {}
        }
    }

    /// Returns the screen to its power-on state without changing its size.
    pub fn reset(&mut self) {
        self.grid.clear();
        self.cursor = Point::default();
        self.pen = Pen::default();
        self.scroll_top = 0;
        self.scroll_bottom = self.rows() - 1;
        self.autowrap = true;
        self.wrap_pending = false;
        self.origin_mode = false;
        self.saved_cursor = None;
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

    fn csi(&mut self, csi: Csi) {
        crate::csi::dispatch(self, csi);
    }

    /// Moves the cursor, clamping it to the screen.
    pub(crate) fn move_to(&mut self, x: i32, y: i32) {
        self.cursor.x = x.clamp(0, i32::from(self.cols()) - 1) as u16;
        self.cursor.y = y.clamp(0, i32::from(self.rows()) - 1) as u16;
        self.wrap_pending = false;
    }

    /// Erases `n` characters from the cursor without moving anything.
    pub(crate) fn erase_chars(&mut self, n: u16) {
        let (x, y) = (self.cursor.x, self.cursor.y);
        let last = x.saturating_add(n).saturating_sub(1);
        self.grid.erase_cells(y, x, last, Pen::default());
    }

    /// Opens a gap of `n` characters at the cursor.
    pub(crate) fn insert_chars(&mut self, n: u16) {
        let (x, y) = (self.cursor.x, self.cursor.y);
        self.grid.insert_cells(y, x, n, Pen::default());
    }

    /// Removes `n` characters at the cursor, pulling the rest left.
    pub(crate) fn delete_chars(&mut self, n: u16) {
        let (x, y) = (self.cursor.x, self.cursor.y);
        self.grid.delete_cells(y, x, n, Pen::default());
    }

    /// Inserts `n` blank lines at the cursor, inside the scroll region.
    pub(crate) fn insert_lines(&mut self, n: u16) {
        if self.cursor.y < self.scroll_top || self.cursor.y > self.scroll_bottom {
            return;
        }
        self.grid
            .scroll_down(self.cursor.y, self.scroll_bottom, n, Pen::default());
        self.cursor.x = 0;
        self.wrap_pending = false;
    }

    /// Deletes `n` lines at the cursor, pulling the rest of the region up.
    pub(crate) fn delete_lines(&mut self, n: u16) {
        if self.cursor.y < self.scroll_top || self.cursor.y > self.scroll_bottom {
            return;
        }
        self.grid
            .scroll_up(self.cursor.y, self.scroll_bottom, n, Pen::default());
        self.cursor.x = 0;
        self.wrap_pending = false;
    }

    /// Scrolls the scroll region up by `n` lines.
    pub(crate) fn scroll_up(&mut self, n: u16) {
        self.grid
            .scroll_up(self.scroll_top, self.scroll_bottom, n, Pen::default());
    }

    /// Scrolls the scroll region down by `n` lines.
    pub(crate) fn scroll_down(&mut self, n: u16) {
        self.grid
            .scroll_down(self.scroll_top, self.scroll_bottom, n, Pen::default());
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

#[cfg(test)]
mod region_tests {
    use crate::screen::Screen;

    #[test]
    fn the_scroll_region_defaults_to_the_whole_screen() {
        let screen = Screen::new(4, 5);
        assert_eq!(screen.scroll_region(), (0, 4));
    }

    #[test]
    fn setting_the_region_homes_the_cursor() {
        let mut screen = Screen::new(6, 6);
        screen.feed("abc\x1b[4;2H\x1b[2;4r");
        assert_eq!(screen.scroll_region(), (1, 3));
        assert_eq!(screen.cursor(), crate::screen::Point::new(0, 0));
    }

    #[test]
    fn a_degenerate_region_is_ignored() {
        let mut screen = Screen::new(6, 6);
        screen.feed("\x1b[4;2r");
        assert_eq!(screen.scroll_region(), (0, 5));
        screen.feed("\x1b[9;9r");
        assert_eq!(screen.scroll_region(), (0, 5));
    }

    #[test]
    fn line_feed_scrolls_only_the_region() {
        let mut screen = Screen::new(3, 5);
        screen.feed("a\r\nb\r\nc\r\nd\r\ne");
        // Rows 2 to 4 in one-based terms: the middle three rows scroll.
        screen.feed("\x1b[2;4r");
        screen.feed("\x1b[4;1H\n");
        assert_eq!(screen.line(0), "a");
        assert_eq!(screen.line(1), "c");
        assert_eq!(screen.line(2), "d");
        assert_eq!(screen.line(3), "");
        assert_eq!(screen.line(4), "e");
    }

    #[test]
    fn reverse_index_scrolls_down_at_the_top_of_the_region() {
        let mut screen = Screen::new(3, 4);
        screen.feed("a\r\nb\r\nc\r\nd");
        screen.feed("\x1b[2;3r\x1b[2;1H");
        screen.feed("\x1bM");
        assert_eq!(screen.line(0), "a");
        assert_eq!(screen.line(1), "");
        assert_eq!(screen.line(2), "b");
        assert_eq!(screen.line(3), "d");
    }

    #[test]
    fn next_line_resets_the_column() {
        let mut screen = Screen::new(6, 3);
        screen.feed("abc\x1bE");
        assert_eq!(screen.cursor(), crate::screen::Point::new(0, 1));
    }

    #[test]
    fn saving_and_restoring_restores_the_pen_too() {
        let mut screen = Screen::new(6, 3);
        screen.feed("\x1b[31m\x1b7");
        screen.feed("\x1b[0m\x1b[3;4H");
        assert_eq!(screen.cursor(), crate::screen::Point::new(3, 2));
        screen.feed("\x1b8");
        assert_eq!(screen.cursor(), crate::screen::Point::new(0, 0));
        assert_eq!(screen.pen().fg, crate::cell::Color::Indexed(1));
    }

    #[test]
    fn the_csi_form_saves_and_restores_the_same_way() {
        let mut screen = Screen::new(6, 3);
        screen.feed("\x1b[3;4H\x1b[s\x1b[1;1H\x1b[u");
        assert_eq!(screen.cursor(), crate::screen::Point::new(3, 2));
    }

    #[test]
    fn a_full_reset_clears_everything() {
        let mut screen = Screen::new(5, 3);
        screen.feed("\x1b[31mabc\x1b[2;5r\x1bc");
        assert_eq!(screen.line(0), "");
        assert_eq!(screen.scroll_region(), (0, 2));
        assert_eq!(screen.pen(), crate::cell::Pen::default());
        assert_eq!(screen.cursor(), crate::screen::Point::new(0, 0));
    }
}
