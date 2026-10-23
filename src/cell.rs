//! Cells, colour and the pen.
//!
//! A terminal screen is a rectangle of cells. Each cell carries one character
//! and the attributes that character is drawn with. Attributes travel through
//! the escape stream as a "pen" that is set once and applied to every cell
//! written afterwards, which is why the pen is a separate type from the cell.

/// A colour, as the terminal can express one.
///
/// The 256-colour palette and true colour are both part of the xterm
/// specification; the first sixteen entries of the palette are the ANSI ones.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Color {
    /// Whatever the front end uses by default.
    #[default]
    Default,
    /// An index into the 256-colour palette.
    Indexed(u8),
    /// A direct colour.
    Rgb(u8, u8, u8),
}

impl Color {
    /// The palette index, if this is a palette colour.
    pub fn index(self) -> Option<u8> {
        match self {
            Color::Indexed(i) => Some(i),
            _ => None,
        }
    }
}

/// The on/off attributes a cell can carry.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Attributes {
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub blink: bool,
    pub reverse: bool,
    pub hidden: bool,
    pub strikethrough: bool,
}

/// Foreground, background and attributes: everything a cell needs beyond its
/// character.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Pen {
    pub fg: Color,
    pub bg: Color,
    pub attrs: Attributes,
}

impl Pen {
    /// A pen that draws in the default colours with no attributes.
    pub fn new() -> Self {
        Pen::default()
    }
}

/// One character cell.
///
/// `width` is the number of columns the character occupies: 1 for the ordinary
/// case, 2 for a wide character, and 0 for the cell that a wide character
/// borrows from its right-hand neighbour. A continuation cell has no character
/// of its own; the wide character to its left owns it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    pub ch: char,
    pub pen: Pen,
    pub width: u8,
}

impl Default for Cell {
    fn default() -> Self {
        Cell {
            ch: ' ',
            pen: Pen::default(),
            width: 1,
        }
    }
}

impl Cell {
    /// A blank cell drawn with the given pen.
    pub fn blank(pen: Pen) -> Self {
        Cell {
            ch: ' ',
            pen,
            width: 1,
        }
    }

    /// A cell holding `ch`, occupying `width` columns.
    pub fn new(ch: char, pen: Pen, width: u8) -> Self {
        Cell { ch, pen, width }
    }

    /// The right-hand half of a wide character.
    pub fn continuation(pen: Pen) -> Self {
        Cell {
            ch: ' ',
            pen,
            width: 0,
        }
    }

    /// Whether this cell is the right-hand half of a wide character.
    pub fn is_continuation(&self) -> bool {
        self.width == 0
    }

    /// Whether the cell is a blank drawn in the default pen.
    pub fn is_blank(&self) -> bool {
        self.ch == ' ' && self.pen == Pen::default() && self.width == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cell_is_a_blank() {
        let cell = Cell::default();
        assert_eq!(cell.ch, ' ');
        assert_eq!(cell.width, 1);
        assert!(cell.is_blank());
        assert!(!cell.is_continuation());
    }

    #[test]
    fn a_continuation_borrows_the_pen_but_not_the_character() {
        let mut pen = Pen::new();
        pen.fg = Color::Indexed(3);
        let cell = Cell::continuation(pen);
        assert!(cell.is_continuation());
        assert_eq!(cell.pen, pen);
        assert!(!cell.is_blank());
    }

    #[test]
    fn palette_colours_report_their_index() {
        assert_eq!(Color::Indexed(9).index(), Some(9));
        assert_eq!(Color::Rgb(1, 2, 3).index(), None);
        assert_eq!(Color::Default.index(), None);
    }

    #[test]
    fn pens_compare_by_value() {
        let mut a = Pen::new();
        let mut b = Pen::new();
        assert_eq!(a, b);
        a.attrs.bold = true;
        assert_ne!(a, b);
        b.attrs.bold = true;
        assert_eq!(a, b);
    }
}
