extern crate termion;

use std::fmt;
use std::io::{self, Read, Write};
use std::thread::sleep;
use std::time::{Duration, Instant};

use termion::{async_stdin, clear, cursor, style};
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
        start: Instant,
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
                start: Instant::now(),
                duration,
                running: Duration::from_secs(0),
                paused: Duration::from_secs(0),
                title: String::from(title),
                note: None,
            }
        }

        pub fn tick(&mut self) -> State {
            use self::State::*;
            let elapsed = self.start.elapsed();
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

        // TODO: This should not be on the Countdown itself imo
        pub fn one_line(&self) -> String {
            let total = self.total();
            format!("{}> {}{}{}{}{}: {} ({}) {}",
                    style::Bold,
                    style::Reset,
                    style::Underline,
                    style::Italic,
                    self.title,
                    style::Reset,
                    format_duration(&(self.duration - self.running), &self.duration),
                    format_duration(&self.duration, &self.duration),
                    self.state)
        }

        pub fn summarize(&self) -> String {
            let total = self.total();
            format!("{} {}
  - Total Duration: {}
  - Time Running:   {}
  - Time Paused:    {}",
                   self.title, self.state,
                   format_duration(&total, &total),
                   format_duration(&self.running, &total),
                   format_duration(&self.paused, &total))
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

fn cleanup<W: Write>(stdout: &mut W) -> TermResult {
    write!(stdout,
           "{}{}{}{}{}\n",
           cursor::Goto(1, 1),
           clear::All,
           cursor::Goto(1, 1),
           cursor::Show,
           screen::ToMainScreen,
    )
}

fn run() -> TermResult {
    let stdin = async_stdin();
    let stdout = io::stdout();
    let mut screen = AlternateScreen::from(stdout.lock().into_raw_mode()?);

    let mut pomo = Pomodoro::new(stdin, screen);

    writeln!(pomo.stdout,
             "{}{}{}",
             clear::All,
             cursor::Hide,
             cursor::Goto(1, 1))?;

    let mut countdown = Countdown::new(Duration::from_secs(11), "Whatever");

    while timer::State::Finished != countdown.tick() {
        let mut key_bytes = [0];
        pomo.stdin.read(&mut key_bytes)?;

        match key_bytes[0] {
            b'q' => break,
            b' ' => { countdown.toggle(); },
            _ => {},
        }
        writeln!(pomo.stdout, "{}{}{}", clear::All, cursor::Goto(1, 1), countdown);
        sleep(SLEEP);
    }
    cleanup(&mut pomo.stdout)?;
    write!(pomo.stdout, "{}\r\n", countdown)?;
    Ok(())
}

fn main() {
    run().unwrap();
}
