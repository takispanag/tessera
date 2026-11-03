//! Plays a `.vt` recording and prints the resulting screen.
//!
//! This is the command-line half of the conformance harness: a recording goes
//! in, a screen comes out as text, and the output can be diffed against an
//! expected file.

use std::io::{self, Read};
use std::process::ExitCode;

use tessera::dump::dump;
use tessera::recording::play;

fn usage() -> &'static str {
    "usage: tessera-dump [--size COLSxROWS] [FILE]\n\
     \n\
     Plays a .vt recording and prints the screen as text.\n\
     Reads standard input when FILE is omitted or is `-`.\n\
     \n\
     options:\n\
     \x20 --size COLSxROWS   screen size, default 80x24\n\
     \x20 -h, --help         show this message"
}

fn main() -> ExitCode {
    let mut cols = 80u16;
    let mut rows = 24u16;
    let mut path: Option<String> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{}", usage());
                return ExitCode::SUCCESS;
            }
            "--size" => {
                let Some(value) = args.next() else {
                    eprintln!("error: --size needs a value like 80x24");
                    return ExitCode::from(2);
                };
                match parse_size(&value) {
                    Some((c, r)) => {
                        cols = c;
                        rows = r;
                    }
                    None => {
                        eprintln!("error: {value:?} is not a size like 80x24");
                        return ExitCode::from(2);
                    }
                }
            }
            other if other.starts_with('-') && other != "-" => {
                eprintln!("error: unknown option {other:?}\n\n{}", usage());
                return ExitCode::from(2);
            }
            other => path = Some(other.to_string()),
        }
    }

    let source = match path.as_deref() {
        None | Some("-") => {
            let mut buffer = String::new();
            if let Err(error) = io::stdin().read_to_string(&mut buffer) {
                eprintln!("error: cannot read standard input: {error}");
                return ExitCode::from(1);
            }
            buffer
        }
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("error: cannot read {path}: {error}");
                return ExitCode::from(1);
            }
        },
    };

    let screen = play(cols, rows, &source);
    println!("{}", dump(&screen));
    ExitCode::SUCCESS
}

fn parse_size(value: &str) -> Option<(u16, u16)> {
    let (cols, rows) = value.split_once(['x', 'X'])?;
    let cols: u16 = cols.parse().ok()?;
    let rows: u16 = rows.parse().ok()?;
    if cols == 0 || rows == 0 {
        return None;
    }
    Some((cols, rows))
}
