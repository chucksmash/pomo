extern crate termion;

use std::io::{self, Write};
use std::thread::sleep;
use std::time::{Duration, Instant};

use termion::clear;
use termion::cursor::{self, DetectCursorPos};
use termion::event::{Event, Key, MouseEvent};
use termion::input::{MouseTerminal, TermRead};
use termion::raw::IntoRawMode;
use termion::screen;

type TermResult = Result<(), io::Error>;

fn run() -> TermResult {
    let stdin = io::stdin();
    let mut stdout = MouseTerminal::from(io::stdout().into_raw_mode()?);

    writeln!(stdout,
             "{}{}q to exit. Type stuff, use alt, click around...",
             clear::All,
             cursor::Goto(1, 1))?;

    for c in stdin.events() {
        match c? {
            Event::Key(Key::Char('q')) => break,
            Event::Mouse(me) => {
                match me {
                    MouseEvent::Press(_, a, b) => {
                        write!(stdout, "{}", cursor::Goto(a, b))?;
                        let (x, y) = stdout.cursor_pos()?;
                        write!(stdout,
                               "{}{}Cursor is at: ({},{}){}",
                               cursor::Goto(5, 5),
                               clear::UntilNewline,
                               x,
                               y,
                               cursor::Goto(a, b))?;
                    },
                    _ => {},
                }
            },
            _ => {},
        }
        stdout.flush()?;
    }
    write!(stdout, "{}{}{}\n", cursor::Goto(1, 1), clear::All, cursor::Goto(1, 1))?;
    Ok(())
}

fn main() {
    run().unwrap();
}
