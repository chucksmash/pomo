#![feature(duration_as_u128)]

extern crate termion;

use std::io::{self, Write};
use std::thread::sleep;
use std::time::{Duration, Instant};

use termion::{async_stdin, clear, screen};
use termion::cursor::{self, DetectCursorPos};
use termion::event::{Event, Key, MouseEvent};
use termion::input::{MouseTerminal, TermRead};
use termion::raw::IntoRawMode;

type TermResult = Result<(), io::Error>;

fn run_w_mouse() -> TermResult {
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

fn run() -> TermResult {
    const MILLI_IN_SEC: u128 = 1000;
    const MILLI_IN_MINUTE: u128 = MILLI_IN_SEC * 60;
    const MILLI_IN_HOUR: u128 = MILLI_IN_MINUTE * 60;
    const MILLI_IN_DAY: u128 = MILLI_IN_HOUR * 24;
    const RUNTIME: Duration = Duration::from_secs(10);
    const SLEEP: Duration = Duration::from_millis(100);
    let stdin = async_stdin();
    let mut stdout = MouseTerminal::from(io::stdout().into_raw_mode()?);

    writeln!(stdout,
             "{}{}{}",
             clear::All,
             cursor::Hide,
             cursor::Goto(1, 1))?;

    let start = Instant::now();
    loop {
        let elapsed = start.elapsed();
        match RUNTIME.checked_sub(elapsed) {
            None => break,
            Some(remaining) => {
                let mut t = remaining.as_millis();
                let total = t;
                let days = t / MILLI_IN_DAY;
                t %= MILLI_IN_DAY;
                let hours = t / MILLI_IN_HOUR;
                t %= MILLI_IN_HOUR;
                let minutes = t / MILLI_IN_MINUTE;
                t %= MILLI_IN_MINUTE;
                let seconds = t / MILLI_IN_SEC;
                t %= MILLI_IN_SEC;
                let millis = t;

                let day_str = match total >= MILLI_IN_DAY {
                    true => format!("{} days ", days),
                    false => "".to_owned(),
                };
                let hour_str = match total >= MILLI_IN_HOUR {
                    true => format!("{} hours ", hours),
                    false => "".to_owned(),
                };
                let minute_str = match total >= MILLI_IN_MINUTE {
                    true => format!("{} minutes", minutes),
                    false => "".to_owned(),
                };
                writeln!(stdout,
                         "{}{}{}{}{}{}.{:02} seconds",
                         cursor::Goto(1, 1),
                         clear::All,
                         day_str,
                         hour_str,
                         minute_str,
                         seconds,
                         millis / 10)?;
                sleep(SLEEP);
            },
        }
    }
    write!(stdout, "{}{}{}{}\n", cursor::Goto(1, 1), clear::All, cursor::Goto(1, 1), cursor::Show)?;
    Ok(())
}

fn main() {
    run().unwrap();
    // run_w_mouse().unwrap();
}
