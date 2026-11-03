//! The escape-sequence state machine.
//!
//! A terminal is fed a stream of characters in which most are ordinary text and
//! a few open a control sequence. The parser's only job is to tell the two
//! apart and to hand the rest of the engine one clean message at a time: print
//! this character, execute this control, or apply this control sequence.
//!
//! The machine is the one described by ECMA-48 and used by xterm: ground,
//! escape, control-sequence entry, control-sequence parameters, control-sequence
//! intermediates, and the operating-system-command string. Because it consumes
//! `char` rather than bytes, UTF-8 has already been decoded by the time we see
//! it, and every syntax character in the specification is ASCII.

/// A control sequence, split into the parts that decide what it means.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Csi {
    /// The private marker that opened the sequence, if any: `?`, `>`, `<`, `=`.
    pub private: Option<char>,
    /// Numeric parameters. `None` is an omitted parameter, which is not the
    /// same as `0` for the handful of sequences where zero is meaningful.
    pub params: Vec<Option<u16>>,
    /// A single intermediate byte, if the sequence carried one.
    pub intermediate: Option<char>,
    /// The final byte that identifies the sequence.
    pub final_byte: char,
}

impl Csi {
    /// Parameter `index`, or `default` when it is absent or zero.
    ///
    /// Most of ECMA-48 treats an omitted parameter and an explicit `0` the same
    /// way, which is what this helper is for. Use [`Csi::raw`] when zero must
    /// be distinguished from omitted.
    pub fn param(&self, index: usize, default: u16) -> u16 {
        match self.params.get(index) {
            Some(Some(0)) | None => default,
            Some(Some(value)) => *value,
            Some(None) => default,
        }
    }

    /// Parameter `index` exactly as written, if it was written.
    pub fn raw(&self, index: usize) -> Option<u16> {
        self.params.get(index).copied().flatten()
    }

    /// Whether the sequence carried no parameters at all.
    pub fn is_bare(&self) -> bool {
        self.params.is_empty()
    }
}

/// One thing the parser recognised.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Action {
    /// Ordinary text.
    Print(char),
    /// A control character, executed immediately.
    Execute(char),
    /// A control sequence, dispatched by the screen.
    Csi(Csi),
    /// A simple escape sequence, such as `ESC 7` to save the cursor.
    Esc(char),
    /// An operating-system command: the string between `ESC ]` and the
    /// terminator, untouched.
    Osc(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    Ground,
    Escape,
    CsiEntry,
    CsiParam,
    CsiIntermediate,
    OscString,
    OscEscape,
    DcsIgnore,
    DcsEscape,
}

/// The parser state machine.
#[derive(Clone, Debug)]
pub struct Parser {
    state: State,
    csi: Csi,
    current: Option<u16>,
    osc: String,
}

impl Default for Parser {
    fn default() -> Self {
        Parser::new()
    }
}

impl Parser {
    /// A parser in the ground state.
    pub fn new() -> Self {
        Parser {
            state: State::Ground,
            csi: Csi::default(),
            current: None,
            osc: String::new(),
        }
    }

    /// Feeds one character and returns the action it completed, if any.
    pub fn advance(&mut self, c: char) -> Option<Action> {
        match self.state {
            State::Ground => self.ground(c),
            State::Escape => self.escape(c),
            State::CsiEntry | State::CsiParam | State::CsiIntermediate => self.csi_step(c),
            State::OscString => self.osc_step(c),
            State::OscEscape => match c {
                '\\' => {
                    self.state = State::Ground;
                    Some(Action::Osc(std::mem::take(&mut self.osc)))
                }
                _ => {
                    // An unterminated OSC gives way to whatever follows.
                    self.osc.clear();
                    self.state = State::Escape;
                    self.escape(c)
                }
            },
            State::DcsIgnore => match c {
                '\x1b' => {
                    self.state = State::DcsEscape;
                    None
                }
                _ => None,
            },
            State::DcsEscape => {
                if c == '\\' {
                    self.state = State::Ground;
                } else {
                    self.state = State::DcsIgnore;
                }
                None
            }
        }
    }

    fn ground(&mut self, c: char) -> Option<Action> {
        match c {
            '\x1b' => {
                self.state = State::Escape;
                None
            }
            '\x00'..='\x1f' => Some(Action::Execute(c)),
            '\x7f' => None,
            _ => Some(Action::Print(c)),
        }
    }

    fn escape(&mut self, c: char) -> Option<Action> {
        match c {
            '[' => {
                self.csi = Csi::default();
                self.current = None;
                self.state = State::CsiEntry;
                None
            }
            ']' => {
                self.osc.clear();
                self.state = State::OscString;
                None
            }
            'P' | '^' | '_' => {
                self.state = State::DcsIgnore;
                None
            }
            '\x1b' => None,
            _ => {
                self.state = State::Ground;
                Some(Action::Esc(c))
            }
        }
    }

    fn csi_step(&mut self, c: char) -> Option<Action> {
        match c {
            '0'..='9' => {
                let digit = c as u16 - '0' as u16;
                self.current = Some(
                    self.current
                        .unwrap_or(0)
                        .saturating_mul(10)
                        .saturating_add(digit),
                );
                self.state = State::CsiParam;
                None
            }
            ';' => {
                let value = self.current.take();
                self.csi.params.push(value);
                self.state = State::CsiParam;
                None
            }
            '<' | '=' | '>' | '?' if self.state == State::CsiEntry => {
                self.csi.private = Some(c);
                None
            }
            ' '..='/' => {
                self.csi.intermediate = Some(c);
                self.state = State::CsiIntermediate;
                None
            }
            '\x40'..='\x7e' => {
                if self.current.is_some() || !self.csi.params.is_empty() {
                    self.csi.params.push(self.current.take());
                }
                self.csi.final_byte = c;
                self.state = State::Ground;
                Some(Action::Csi(std::mem::take(&mut self.csi)))
            }
            // ESC cancels the sequence it interrupts, so a truncated CSI
            // cannot swallow the one that follows it.
            '\x1b' => {
                self.csi = Csi::default();
                self.current = None;
                self.state = State::Escape;
                None
            }
            // The other C0 controls inside a sequence are executed, not
            // swallowed.
            '\x00'..='\x1f' => Some(Action::Execute(c)),
            _ => {
                self.state = State::Ground;
                None
            }
        }
    }

    fn osc_step(&mut self, c: char) -> Option<Action> {
        match c {
            '\x07' => {
                self.state = State::Ground;
                Some(Action::Osc(std::mem::take(&mut self.osc)))
            }
            '\x1b' => {
                self.state = State::OscEscape;
                None
            }
            '\x00'..='\x1f' => None,
            _ => {
                self.osc.push(c);
                None
            }
        }
    }
}

/// Runs a whole string through a fresh parser.
pub fn parse(input: &str) -> Vec<Action> {
    let mut parser = Parser::new();
    input.chars().filter_map(|c| parser.advance(c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn csi(input: &str) -> Csi {
        match parse(input).pop().expect("an action") {
            Action::Csi(csi) => csi,
            other => panic!("expected a CSI, got {other:?}"),
        }
    }

    #[test]
    fn text_prints_character_by_character() {
        assert_eq!(parse("hi"), vec![Action::Print('h'), Action::Print('i')]);
    }

    #[test]
    fn control_characters_execute() {
        assert_eq!(
            parse("\r\n"),
            vec![Action::Execute('\r'), Action::Execute('\n')]
        );
    }

    #[test]
    fn a_bare_csi_has_no_parameters() {
        let s = csi("\x1b[m");
        assert!(s.is_bare());
        assert_eq!(s.final_byte, 'm');
        assert_eq!(s.private, None);
        assert_eq!(s.param(0, 1), 1);
    }

    #[test]
    fn parameters_are_parsed_in_order() {
        let s = csi("\x1b[2;7H");
        assert_eq!(s.params, vec![Some(2), Some(7)]);
        assert_eq!(s.final_byte, 'H');
        assert_eq!(s.param(0, 1), 2);
        assert_eq!(s.param(1, 1), 7);
        assert_eq!(s.param(2, 9), 9);
    }

    #[test]
    fn omitted_parameters_are_not_zero() {
        let s = csi("\x1b[;5m");
        assert_eq!(s.params, vec![None, Some(5)]);
        assert_eq!(s.raw(0), None);
        assert_eq!(s.param(0, 1), 1);
    }

    #[test]
    fn explicit_zero_is_kept_for_sgr() {
        let s = csi("\x1b[0m");
        assert_eq!(s.raw(0), Some(0));
        // The helper treats zero as omitted, which is right for most
        // sequences and wrong for SGR; SGR reads `raw` instead.
        assert_eq!(s.param(0, 1), 1);
    }

    #[test]
    fn private_markers_are_recognised() {
        let s = csi("\x1b[?25l");
        assert_eq!(s.private, Some('?'));
        assert_eq!(s.raw(0), Some(25));
        assert_eq!(s.final_byte, 'l');
    }

    #[test]
    fn intermediates_are_kept() {
        let s = csi("\x1b[4 q");
        assert_eq!(s.intermediate, Some(' '));
        assert_eq!(s.final_byte, 'q');
    }

    #[test]
    fn osc_ends_at_bel_or_string_terminator() {
        assert_eq!(
            parse("\x1b]0;title\x07"),
            vec![Action::Osc("0;title".to_string())]
        );
        assert_eq!(
            parse("\x1b]2;other\x1b\\"),
            vec![Action::Osc("2;other".to_string())]
        );
    }

    #[test]
    fn simple_escapes_are_reported() {
        assert_eq!(parse("\x1b7"), vec![Action::Esc('7')]);
        assert_eq!(parse("\x1b8"), vec![Action::Esc('8')]);
    }

    #[test]
    fn dcs_payloads_are_swallowed() {
        assert_eq!(parse("\x1bP1$r\x1b\\x"), vec![Action::Print('x')]);
    }

    #[test]
    fn interrupted_sequences_recover() {
        // A CSI cut short by an escape must not eat the next sequence.
        let actions = parse("\x1b[12\x1b[31m");
        assert_eq!(
            actions.last(),
            Some(&Action::Csi(Csi {
                private: None,
                params: vec![Some(31)],
                intermediate: None,
                final_byte: 'm',
            }))
        );
    }

    #[test]
    fn saturating_parameters_do_not_overflow() {
        let s = csi("\x1b[999999999999H");
        assert_eq!(s.raw(0), Some(u16::MAX));
    }
}
