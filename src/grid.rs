//! The cell grid.
//!
//! The grid owns the rectangle of cells and nothing else: no cursor, no
//! attributes, no rules about what a control sequence means. Keeping it that
//! dumb makes it cheap to copy, resize and test, and it means the screen layer
//! above it is the only place that has to know about the specification.
//!
//! Scrolling and erasing live here because they are pure geometry: which cells
//! move, and which become blank. Whether a sequence should scroll a region is
//! the screen's decision, not the grid's.

use crate::cell::{Cell, Pen};

/// A rectangle of [`Cell`]s addressed by column and row.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Grid {
    cols: u16,
    rows: u16,
    cells: Vec<Cell>,
    dirty: Vec<bool>,
}

impl Grid {
    /// A grid of `cols` Ξ“β€” `rows` blank cells.
    pub fn new(cols: u16, rows: u16) -> Self {
        Grid {
            cols,
            rows,
            cells: vec![Cell::default(); usize::from(cols) * usize::from(rows)],
            dirty: vec![true; usize::from(rows)],
        }
    }

    /// The number of columns.
    pub fn cols(&self) -> u16 {
        self.cols
    }

    /// The number of rows.
    pub fn rows(&self) -> u16 {
        self.rows
    }

    fn index(&self, x: u16, y: u16) -> Option<usize> {
        if x >= self.cols || y >= self.rows {
            return None;
        }
        Some(usize::from(y) * usize::from(self.cols) + usize::from(x))
    }

    /// The cell at (`x`, `y`).
    ///
    /// A coordinate outside the grid reads as a blank cell rather than
    /// panicking, so a malformed escape sequence cannot take the engine down.
    pub fn cell(&self, x: u16, y: u16) -> Cell {
        self.index(x, y)
            .map_or_else(Cell::default, |i| self.cells[i])
    }

    /// Writes `cell` at (`x`, `y`). Coordinates outside the grid are ignored.
    pub fn set(&mut self, x: u16, y: u16, cell: Cell) {
        if let Some(i) = self.index(x, y) {
            self.cells[i] = cell;
            self.dirty[usize::from(y)] = true;
        }
    }

    /// One row as a slice, or an empty slice when `y` is outside the grid.
    pub fn row(&self, y: u16) -> &[Cell] {
        if y >= self.rows {
            return &[];
        }
        let start = usize::from(y) * usize::from(self.cols);
        &self.cells[start..start + usize::from(self.cols)]
    }

    /// One row as a mutable slice, or an empty slice when `y` is out of range.
    pub fn row_mut(&mut self, y: u16) -> &mut [Cell] {
        if y >= self.rows {
            return &mut [];
        }
        self.dirty[usize::from(y)] = true;
        let start = usize::from(y) * usize::from(self.cols);
        &mut self.cells[start..start + usize::from(self.cols)]
    }

    /// Fills the whole grid with blanks in the default pen.
    pub fn clear(&mut self) {
        self.fill(Pen::default());
    }

    /// Fills the whole grid with blanks in `pen`.
    pub fn fill(&mut self, pen: Pen) {
        let blank = Cell::blank(pen);
        for cell in &mut self.cells {
            *cell = blank;
        }
        self.mark_all_dirty();
    }

    /// Fills rows `top..=bottom` with blanks in `pen`.
    pub fn clear_rows(&mut self, top: u16, bottom: u16, pen: Pen) {
        let blank = Cell::blank(pen);
        for y in top..=bottom.min(self.rows.saturating_sub(1)) {
            if y >= self.rows {
                break;
            }
            let start = usize::from(y) * usize::from(self.cols);
            let end = start + usize::from(self.cols);
            for cell in &mut self.cells[start..end] {
                *cell = blank;
            }
            self.dirty[usize::from(y)] = true;
        }
    }

    /// Blanks the cells of row `y` in columns `from..=to`, inclusive.
    pub fn erase_cells(&mut self, y: u16, from: u16, to: u16, pen: Pen) {
        let blank = Cell::blank(pen);
        let last = to.min(self.cols.saturating_sub(1));
        for x in from..=last {
            self.set(x, y, blank);
        }
    }

    /// Scrolls rows `top..=bottom` up by `count`, blanking what comes in at the
    /// bottom. Rows outside the region do not move.
    pub fn scroll_up(&mut self, top: u16, bottom: u16, count: u16, pen: Pen) {
        self.scroll(top, bottom, count, pen, true);
    }

    /// Scrolls rows `top..=bottom` down by `count`, blanking what comes in at
    /// the top.
    pub fn scroll_down(&mut self, top: u16, bottom: u16, count: u16, pen: Pen) {
        self.scroll(top, bottom, count, pen, false);
    }

    fn scroll(&mut self, top: u16, bottom: u16, count: u16, pen: Pen, up: bool) {
        let top = top.min(self.rows.saturating_sub(1));
        let bottom = bottom.min(self.rows.saturating_sub(1));
        if top > bottom || count == 0 {
            return;
        }
        let height = bottom - top + 1;
        let count = count.min(height);
        let cols = usize::from(self.cols);

        // Snapshot the region first: the copy overlaps, so reading while
        // writing would blank rows that have not been read yet.
        let snapshot: Vec<Cell> = (top..=bottom)
            .flat_map(|y| self.row(y).iter().copied())
            .collect();

        for y in top..=bottom {
            let i = (y - top) as isize;
            let src = if up {
                i + count as isize
            } else {
                i - count as isize
            };
            for x in 0..cols {
                let cell = if src < 0 || src >= height as isize {
                    Cell::blank(pen)
                } else {
                    snapshot[src as usize * cols + x]
                };
                self.cells[usize::from(y) * cols + x] = cell;
            }
            self.dirty[usize::from(y)] = true;
        }
    }

    /// Marks every row as needing a repaint.
    pub fn mark_all_dirty(&mut self) {
        for flag in &mut self.dirty {
            *flag = true;
        }
    }

    /// Whether row `y` has changed since the last repaint.
    pub fn is_dirty(&self, y: u16) -> bool {
        self.dirty.get(usize::from(y)).copied().unwrap_or(false)
    }

    /// Whether any row has changed since the last repaint.
    pub fn any_dirty(&self) -> bool {
        self.dirty.iter().any(|d| *d)
    }

    /// Clears the dirty flags, as a front end does once it has repainted.
    pub fn clear_dirty(&mut self) {
        for flag in &mut self.dirty {
            *flag = false;
        }
    }

    /// Resizes the grid, keeping the top-left corner.
    ///
    /// Content that no longer fits is dropped and new space is blank. This is
    /// the behaviour of a terminal that is resized without reflow; reflowing
    /// wrapped lines is a separate problem and is not attempted here.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        if cols == self.cols && rows == self.rows {
            return;
        }
        let mut next = Grid::new(cols, rows);
        let keep_cols = cols.min(self.cols);
        let keep_rows = rows.min(self.rows);
        for y in 0..keep_rows {
            for x in 0..keep_cols {
                next.set(x, y, self.cell(x, y));
            }
        }
        *self = next;
    }
}

impl Grid {
    /// Opens a gap of `count` cells at (`x`, `y`), shifting the rest right.
    pub fn insert_cells(&mut self, y: u16, x: u16, count: u16, pen: Pen) {
        if y >= self.rows || x >= self.cols || count == 0 {
            return;
        }
        let cols = self.cols;
        let count = count.min(cols - x);
        let blank = Cell::blank(pen);
        let mut row: Vec<Cell> = self.row(y).to_vec();
        for i in 0..count {
            row.insert(usize::from(x + i), blank);
        }
        row.truncate(usize::from(cols));
        self.row_mut(y).copy_from_slice(&row);
    }

    /// Removes `count` cells at (`x`, `y`), shifting the rest left.
    pub fn delete_cells(&mut self, y: u16, x: u16, count: u16, pen: Pen) {
        if y >= self.rows || x >= self.cols || count == 0 {
            return;
        }
        let cols = self.cols;
        let count = count.min(cols - x);
        let blank = Cell::blank(pen);
        let mut row: Vec<Cell> = self.row(y).to_vec();
        for _ in 0..count {
            row.remove(usize::from(x));
        }
        row.resize(usize::from(cols), blank);
        self.row_mut(y).copy_from_slice(&row);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Color;

    fn xs(row: &[Cell]) -> String {
        row.iter().map(|c| c.ch).collect()
    }

    #[test]
    fn a_new_grid_is_all_blanks() {
        let grid = Grid::new(4, 2);
        assert_eq!(grid.cols(), 4);
        assert_eq!(grid.rows(), 2);
        for y in 0..2 {
            for x in 0..4 {
                assert!(grid.cell(x, y).is_blank());
            }
        }
        assert!(grid.any_dirty());
    }

    #[test]
    fn cells_round_trip() {
        let mut grid = Grid::new(3, 3);
        let pen = Pen {
            fg: Color::Indexed(1),
            ..Pen::default()
        };
        grid.set(2, 1, Cell::new('x', pen, 1));
        let cell = grid.cell(2, 1);
        assert_eq!(cell.ch, 'x');
        assert_eq!(cell.pen, pen);
    }

    #[test]
    fn out_of_range_coordinates_are_harmless() {
        let mut grid = Grid::new(2, 2);
        assert!(grid.cell(99, 99).is_blank());
        grid.set(99, 99, Cell::new('x', Pen::default(), 1));
        assert!(grid.row(99).is_empty());
        assert!(grid.row_mut(99).is_empty());
        assert!(!grid.is_dirty(99));
    }

    #[test]
    fn rows_expose_their_cells() {
        let mut grid = Grid::new(2, 2);
        grid.set(0, 1, Cell::new('a', Pen::default(), 1));
        assert_eq!(grid.row(1).len(), 2);
        assert_eq!(grid.row(1)[0].ch, 'a');
        grid.row_mut(1)[1] = Cell::new('b', Pen::default(), 1);
        assert_eq!(grid.cell(1, 1).ch, 'b');
    }

    #[test]
    fn resizing_keeps_the_top_left_corner() {
        let mut grid = Grid::new(3, 2);
        grid.set(0, 0, Cell::new('a', Pen::default(), 1));
        grid.set(2, 1, Cell::new('z', Pen::default(), 1));

        grid.resize(5, 3);
        assert_eq!(grid.cell(0, 0).ch, 'a');
        assert_eq!(grid.cell(2, 1).ch, 'z');
        assert!(grid.cell(4, 2).is_blank());

        grid.resize(2, 1);
        assert_eq!(grid.cell(0, 0).ch, 'a');
        assert_eq!(grid.cell(2, 1).ch, ' ');
    }

    #[test]
    fn clearing_restores_blanks() {
        let mut grid = Grid::new(2, 1);
        grid.set(0, 0, Cell::new('a', Pen::default(), 1));
        grid.clear();
        assert!(grid.cell(0, 0).is_blank());
    }

    #[test]
    fn scrolling_up_moves_rows_and_blanks_the_last() {
        let mut grid = Grid::new(3, 4);
        for y in 0..4 {
            grid.set(
                0,
                y,
                Cell::new(char::from(b'a' + y as u8), Pen::default(), 1),
            );
        }
        grid.scroll_up(0, 3, 1, Pen::default());
        assert_eq!(xs(grid.row(0)), "b  ");
        assert_eq!(xs(grid.row(2)), "d  ");
        assert_eq!(xs(grid.row(3)), "   ");
    }

    #[test]
    fn scrolling_down_blanks_the_first() {
        let mut grid = Grid::new(3, 3);
        grid.set(0, 0, Cell::new('a', Pen::default(), 1));
        grid.set(0, 1, Cell::new('b', Pen::default(), 1));
        grid.scroll_down(0, 2, 1, Pen::default());
        assert_eq!(xs(grid.row(0)), "   ");
        assert_eq!(xs(grid.row(1)), "a  ");
        assert_eq!(xs(grid.row(2)), "b  ");
    }

    #[test]
    fn scrolling_respects_the_region() {
        let mut grid = Grid::new(2, 4);
        for y in 0..4 {
            grid.set(
                0,
                y,
                Cell::new(char::from(b'a' + y as u8), Pen::default(), 1),
            );
        }
        grid.scroll_up(1, 2, 1, Pen::default());
        assert_eq!(grid.cell(0, 0).ch, 'a');
        assert_eq!(grid.cell(0, 1).ch, 'c');
        assert_eq!(grid.cell(0, 2).ch, ' ');
        assert_eq!(grid.cell(0, 3).ch, 'd');
    }

    #[test]
    fn scrolling_by_more_than_the_region_clears_it() {
        let mut grid = Grid::new(1, 3);
        grid.set(0, 0, Cell::new('a', Pen::default(), 1));
        grid.scroll_up(0, 2, 9, Pen::default());
        assert!(grid.cell(0, 0).is_blank());
        assert!(grid.cell(0, 2).is_blank());
    }

    #[test]
    fn erasing_cells_respects_its_range() {
        let mut grid = Grid::new(4, 1);
        for x in 0..4 {
            grid.set(x, 0, Cell::new('#', Pen::default(), 1));
        }
        grid.erase_cells(0, 1, 2, Pen::default());
        assert_eq!(xs(grid.row(0)), "#  #");
    }

    #[test]
    fn dirty_flags_track_repaints() {
        let mut grid = Grid::new(2, 2);
        grid.clear_dirty();
        assert!(!grid.any_dirty());
        grid.set(0, 1, Cell::new('a', Pen::default(), 1));
        assert!(grid.is_dirty(1));
        assert!(!grid.is_dirty(0));
        grid.clear_dirty();
        assert!(!grid.any_dirty());
    }
}
