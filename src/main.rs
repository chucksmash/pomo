extern crate chrono;
extern crate termion;

use std::fmt;
use std::io::{self, Read, Write};
use std::thread::sleep;
use std::time::{Duration, Instant};

use chrono::{DateTime, Duration as OldDuration, Local};
use termion::{async_stdin, clear, color, cursor, style};
use termion::raw::IntoRawMode;
use termion::screen::{self, AlternateScreen};

use self::timer::Countdown;

macro_rules! maybe_val {
    ( $val:expr, $test:expr, $label:expr ) => {
        {
            match $test {
                true => format!("{} {}", $val, $label),
                false => "".to_owned(),
            }
        }
    }
}

type TermResult = Result<(), io::Error>;

const SEC_IN_MINUTE: u64 = 60;
const SEC_IN_HOUR: u64 = SEC_IN_MINUTE * 60;
const SEC_IN_DAY: u64 = SEC_IN_HOUR * 24;
const DEFAULT_DURATION: Duration = Duration::from_secs(10);
const SLEEP: Duration = Duration::from_millis(100);

struct Pomodoro<R, W> {
    current: Countdown,
    previous: Vec<Countdown>,
    run_length: Duration,
    break_length: Duration,
    stdin: R,
    stdout: W,
}

impl<R: Read, W: Write> Pomodoro<R, W> {
    fn new(stdin: R, stdout: W) -> Pomodoro<R, W> {
        Pomodoro {
            current: Countdown::new(Duration::from_secs(0), ""),
            previous: vec![],
            run_length: Duration::from_secs(10),
            break_length: Duration::from_secs(5),
            stdin,
            stdout,
        }
    }

    fn run(&mut self) -> TermResult {
        writeln!(self.stdout,
                 "{}{}{}",
                 clear::All,
                 cursor::Hide,
                 cursor::Goto(1, 1))?;

        self.current = Countdown::new(self.run_length, "Whatever");

        while timer::State::Finished != self.current.tick() {
            let mut key_bytes = [0];
            self.stdin.read(&mut key_bytes)?;

            match key_bytes[0] {
                b'q' => break,
                b' ' => { self.current.toggle(); },
                _ => {},
            }
            writeln!(self.stdout, "{}{}{}", clear::All, cursor::Goto(1, 1), self.current.one_line());
            sleep(SLEEP);
        }
        self.cleanup()?;
        write!(self.stdout,
               "{}\r\n",
               self.current.summarize().replace("\n", "\r\n"))?;
        Ok(())
    }

    fn cleanup(&mut self) -> TermResult {
        write!(self.stdout,
               "{}{}{}{}{}\n",
               cursor::Goto(1, 1),
               clear::All,
               cursor::Goto(1, 1),
               cursor::Show,
               screen::ToMainScreen,
        )
    }
}

mod timer {
    use super::*;

    #[derive(Clone, Copy, PartialEq)]
    pub enum State {
        Running,
        Paused,
        Finished,
    }

    impl fmt::Display for State {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            use self::State::*;

            write!(f, "[{}]",
                   match self {
                       Running => "RUNNING",
                       Paused => "PAUSED",
                       Finished => "FINISHED",
                   })
        }
    }

    pub struct Countdown {
        state: State,
        start: DateTime<Local>,
        duration: Duration,
        running: Duration,
        paused: Duration,
        title: String,
        note: Option<String>,
    }

    impl Countdown {
        pub fn new(duration: Duration, title: &str) -> Countdown {
            Countdown {
                state: State::Running,
                start: Local::now(),
                duration,
                running: Duration::from_secs(0),
                paused: Duration::from_secs(0),
                title: String::from(title),
                note: None,
            }
        }

        pub fn tick(&mut self) -> State {
            use self::State::*;
            let diff = Local::now().signed_duration_since(self.start);
            let elapsed = OldDuration::to_std(&diff).unwrap();
            match self.state {
                Running => { self.running = elapsed - self.paused; },
                Paused => { self.paused = elapsed - self.running; },
                _ => {},
            };
            if self.duration.checked_sub(self.running).is_none() {
                self.state = Finished;
            }
            self.state
        }

        pub fn toggle(&mut self) {
            use self::State::*;
            self.state = match self.state {
                Running => Paused,
                Paused => Running,
                Finished => Finished,
            };
        }

        pub fn total(&self) -> Duration {
            self.running + self.paused
        }

        pub fn summarize(&self) -> String {
            let total = self.total();
            let end = self.start + OldDuration::from_std(total).unwrap();
            format!("{} {}
  - Total Duration: {}
  - Time Running:   {}
  - Time Paused:    {}
  - Started:        {}
  - Ended:          {}",
                    self.title, self.state,
                    format_duration(&total, &total),
                    format_duration(&self.running, &total),
                    format_duration(&self.paused, &total),
                    self.start.to_rfc3339(),
                    end.to_rfc3339())

        }
    }

    impl fmt::Display for Countdown {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            use self::State::*;

            let duration = match self.duration.checked_sub(self.running) {
                Some(elapsed) => elapsed,
                None => self.duration,
            };

            let status = match self.state {
                Finished => "[FINISHED]",
                Paused => "[PAUSED]",
                Running => "",
            };
            write!(f, "{} {}", format_duration(&duration, &self.duration), status)
        }
    }

    /// Duration isn't our struct and Display isn't our trait
    /// so the majority of this cannot go into an
    /// `impl Display for Duration` as I originally intended.
    fn format_duration(curr: &Duration, out_of: &Duration) -> String {
        let max = out_of.as_secs();
        let total = curr.as_secs();
        let mut tmp = total;
        let days = tmp / SEC_IN_DAY;
        tmp %= SEC_IN_DAY;
        let hours = tmp / SEC_IN_HOUR;
        tmp %= SEC_IN_HOUR;
        let minutes = tmp / SEC_IN_MINUTE;
        tmp %= SEC_IN_MINUTE;
        let seconds = tmp;

        format!("{}{}{}{:02} seconds",
                maybe_val!(days, max >= SEC_IN_DAY, "days "),
                maybe_val!(hours, max >= SEC_IN_HOUR, "hours "),
                maybe_val!(minutes, max >= SEC_IN_MINUTE, "minutes "),
                seconds)
    }
}

// fn run() -> TermResult {
//     let stdin = async_stdin();
//     let stdout = io::stdout();
//     let mut screen = AlternateScreen::from(stdout.lock().into_raw_mode()?);

//     let mut pomo = Pomodoro::new(stdin, screen);
//     pomo.run()
// }

fn main() {
    let stdin = async_stdin();
    let stdout = io::stdout();
    let mut screen = AlternateScreen::from(
        stdout.lock().into_raw_mode().unwrap());
    let mut pomo = Pomodoro::new(stdin, screen);
    pomo.run().unwrap();
}
