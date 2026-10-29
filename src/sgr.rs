//! Select graphic rendition.
//!
//! `SGR` is the sequence that sets the pen: bold, colour, underline and the
//! rest. An omitted parameter is read as zero, which is what xterm does, so an
//! empty parameter inside a longer list resets the pen. The extended colour
//! forms consume the parameters that follow them, and a malformed tail
//! consumes only its introducer.

use crate::cell::{Color, Pen};

/// Applies an `SGR` parameter list to a pen.
pub(crate) fn apply(pen: &mut Pen, params: &[Option<u16>]) {
    if params.is_empty() {
        *pen = Pen::default();
        return;
    }
    let mut i = 0;
    while i < params.len() {
        let code = params[i].unwrap_or(0);
        match code {
            0 => *pen = Pen::default(),
            1 => pen.attrs.bold = true,
            2 => pen.attrs.dim = true,
            3 => pen.attrs.italic = true,
            4 => pen.attrs.underline = true,
            5 | 6 => pen.attrs.blink = true,
            7 => pen.attrs.reverse = true,
            8 => pen.attrs.hidden = true,
            9 => pen.attrs.strikethrough = true,
            22 => {
                pen.attrs.bold = false;
                pen.attrs.dim = false;
            }
            23 => pen.attrs.italic = false,
            24 => pen.attrs.underline = false,
            25 => pen.attrs.blink = false,
            27 => pen.attrs.reverse = false,
            28 => pen.attrs.hidden = false,
            29 => pen.attrs.strikethrough = false,
            30..=37 => pen.fg = Color::Indexed((code - 30) as u8),
            39 => pen.fg = Color::Default,
            40..=47 => pen.bg = Color::Indexed((code - 40) as u8),
            49 => pen.bg = Color::Default,
            90..=97 => pen.fg = Color::Indexed((code - 90 + 8) as u8),
            100..=107 => pen.bg = Color::Indexed((code - 100 + 8) as u8),
            38 | 48 => {
                let (colour, used) = extended(&params[i..], code == 48);
                if let Some(colour) = colour {
                    if code == 48 {
                        pen.bg = colour;
                    } else {
                        pen.fg = colour;
                    }
                }
                i += used;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
}

/// Reads the extended colour that follows `38` or `48`.
///
/// Returns the colour and how many parameters it consumed, including the
/// introducer. A malformed tail consumes only the introducer, so the rest of
/// the list is still applied.
fn extended(rest: &[Option<u16>], _background: bool) -> (Option<Color>, usize) {
    match rest.get(1).copied().flatten() {
        // 256-colour: 38;5;n
        Some(5) => match rest.get(2).copied().flatten() {
            Some(index) if index <= 255 => (Some(Color::Indexed(index as u8)), 3),
            _ => (None, 2),
        },
        // True colour: 38;2;r;g;b
        Some(2) => {
            let component = |at: usize| rest.get(at).copied().flatten();
            match (component(2), component(3), component(4)) {
                (Some(r), Some(g), Some(b)) if r <= 255 && g <= 255 && b <= 255 => {
                    (Some(Color::Rgb(r as u8, g as u8, b as u8)), 5)
                }
                _ => (None, 2),
            }
        }
        _ => (None, 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pen(input: &str) -> Pen {
        let mut screen = crate::screen::Screen::new(4, 2);
        screen.feed(input);
        screen.pen()
    }

    #[test]
    fn empty_sgr_resets() {
        assert_eq!(pen("\x1b[m"), Pen::default());
    }

    #[test]
    fn basic_colours_use_the_palette() {
        assert_eq!(pen("\x1b[31m").fg, Color::Indexed(1));
        assert_eq!(pen("\x1b[44m").bg, Color::Indexed(4));
    }

    #[test]
    fn bright_colours_are_the_upper_half_of_the_palette() {
        assert_eq!(pen("\x1b[93m").fg, Color::Indexed(11));
        assert_eq!(pen("\x1b[105m").bg, Color::Indexed(13));
    }

    #[test]
    fn attributes_accumulate_and_reset() {
        let a = pen("\x1b[1;4;31m");
        assert!(a.attrs.bold);
        assert!(a.attrs.underline);
        assert_eq!(a.fg, Color::Indexed(1));
        let b = pen("\x1b[1;4m\x1b[22;24m");
        assert!(!b.attrs.bold);
        assert!(!b.attrs.underline);
    }

    #[test]
    fn the_256_colour_form_is_read() {
        assert_eq!(pen("\x1b[38;5;208m").fg, Color::Indexed(208));
        assert_eq!(pen("\x1b[48;5;17m").bg, Color::Indexed(17));
    }

    #[test]
    fn true_colour_is_read() {
        assert_eq!(pen("\x1b[38;2;10;20;30m").fg, Color::Rgb(10, 20, 30));
    }

    #[test]
    fn a_malformed_extended_colour_does_not_eat_the_rest() {
        let p = pen("\x1b[38;5;1;4m");
        assert_eq!(p.fg, Color::Indexed(1));
        assert!(p.attrs.underline);
    }

    #[test]
    fn default_colour_can_be_restored() {
        let p = pen("\x1b[31;44m\x1b[39;49m");
        assert_eq!(p.fg, Color::Default);
        assert_eq!(p.bg, Color::Default);
    }

    #[test]
    fn an_omitted_parameter_is_read_as_zero() {
        // xterm reads an empty parameter as 0, so this resets the colour and
        // then turns underline on.
        let p = pen("\x1b[31m\x1b[;4m");
        assert_eq!(p.fg, Color::Default);
        assert!(p.attrs.underline);
    }

    #[test]
    fn the_pen_applies_to_every_cell_written_after_it() {
        let mut screen = crate::screen::Screen::new(4, 1);
        screen.feed("\x1b[32mab\x1b[0mc");
        assert_eq!(screen.grid().cell(0, 0).pen.fg, Color::Indexed(2));
        assert_eq!(screen.grid().cell(1, 0).pen.fg, Color::Indexed(2));
        assert_eq!(screen.grid().cell(2, 0).pen.fg, Color::Default);
    }
}
