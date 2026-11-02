//! Replaying a terminal recording.
//!
//! A recording is the byte stream a program would have sent, written as text so
//! that it can live in the repository and be read in a diff. Escape bytes are
//! spelled `\e`, the C0 controls keep their usual spelling, and anything else
//! can be written as `\xNN`.
//!
//! Playing one is the whole test harness: the recording goes in, the screen
//! comes out, and [`crate::dump::dump`] turns that into text that can be
//! compared with an expected file.

use crate::screen::Screen;

/// Decodes the escapes of a recording into the characters they stand for.
pub fn decode(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('e') | Some('E') => out.push('\x1b'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some('x') => {
                let mut hex = String::new();
                for _ in 0..2 {
                    if let Some(digit) = chars.peek().copied().filter(|d| d.is_ascii_hexdigit()) {
                        hex.push(digit);
                        chars.next();
                    }
                }
                match u8::from_str_radix(&hex, 16) {
                    Ok(byte) => out.push(char::from(byte)),
                    // An incomplete or invalid escape is kept as written, so a
                    // typo in a recording is visible rather than silent.
                    Err(_) => {
                        out.push('\\');
                        out.push('x');
                        out.push_str(&hex);
                    }
                }
            }
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Plays a recording onto a fresh screen of the given size.
pub fn play(cols: u16, rows: u16, recording: &str) -> Screen {
    let mut screen = Screen::new(cols, rows);
    screen.feed(&decode(recording));
    screen
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dump::dump;

    #[test]
    fn escape_sequences_are_decoded() {
        assert_eq!(decode("a\\e[31mb"), "a\x1b[31mb");
        assert_eq!(decode("\\r\\n"), "\r\n");
    }

    #[test]
    fn hex_escapes_carry_arbitrary_bytes() {
        assert_eq!(decode("\\x41\\x0a"), "A\n");
        assert_eq!(decode("\\x1b"), "\x1b");
    }

    #[test]
    fn an_unknown_escape_is_kept_visible() {
        assert_eq!(decode("\\q"), "\\q");
        assert_eq!(decode("\\xzz"), "\\xzz");
    }

    #[test]
    fn playing_a_recording_produces_a_screen() {
        let screen = play(8, 2, "hi\\r\\nthere");
        assert_eq!(dump(&screen), "hi\nthere");
    }

    #[test]
    fn a_coloured_recording_lands_in_the_right_cells() {
        let screen = play(4, 1, "\\e[32mok\\e[0m");
        assert_eq!(screen.grid().cell(0, 0).pen.fg, crate::cell::Color::Indexed(2));
    }
}
