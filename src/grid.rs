//! The cell grid.
//!
//! The grid owns the rectangle of cells and nothing else: no cursor, no
//! attributes, no rules about what a control sequence means. Keeping it that
//! dumb makes it cheap to copy, resize and test, and it means the screen layer
//! above it is the only place that has to know about the specification.

use crate::cell::{Cell, Pen};

/// A rectangle of [`Cell`]s addressed by column and row.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Grid {
    cols: u16,
    rows: u16,
    cells: Vec<Cell>,
}

impl Grid {
    /// A grid of `cols` × `rows` blank cells.
    pub fn new(cols: u16, rows: u16) -> Self {
        Grid {
            cols,
            rows,
            cells: vec![Cell::default(); usize::from(cols) * usize::from(rows)],
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
        self.index(x, y).map_or_else(Cell::default, |i| self.cells[i])
    }

    /// Writes `cell` at (`x`, `y`). Coordinates outside the grid are ignored.
    pub fn set(&mut self, x: u16, y: u16, cell: Cell) {
        if let Some(i) = self.index(x, y) {
            self.cells[i] = cell;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Color;

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
    }

    #[test]
    fn cells_round_trip() {
        let mut grid = Grid::new(3, 3);
        let mut pen = Pen::default();
        pen.fg = Color::Indexed(1);
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
}
