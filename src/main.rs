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

use self::timer::{Countdown, Position};

macro_rules! maybe_val {
    ( $val:expr, $test:expr, $label:expr ) => {
        {
            match $test {
                true => format!("{:02}{}", $val, $label),
                false => "".to_owned(),
            }
        }
    }
}

macro_rules! lines {
    ( $( $line:expr ),* ) => {
        {
            let mut tmp = Vec::new();
            let blue = color::Fg(color::Rgb(0x26, 0x8b, 0xd2));
            $(
                tmp.push(
                    format!("{reset}{color}{line}{reset}",
                            reset=style::Reset, color=blue, line=$line));
            )*
            tmp
        }
    }
}

type TermResult = Result<(), io::Error>;

const SEC_IN_MINUTE: u64 = 60;
const SEC_IN_HOUR: u64 = SEC_IN_MINUTE * 60;
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
        let hours = tmp / SEC_IN_HOUR;
        tmp %= SEC_IN_HOUR;
        let minutes = tmp / SEC_IN_MINUTE;
        tmp %= SEC_IN_MINUTE;
        let seconds = tmp;

        format!("{}{:02}:{:02}",
                maybe_val!(hours, max >= SEC_IN_HOUR, ":"),
                minutes,
                seconds)
    }

    #[derive(Debug, PartialEq)]
    pub struct Position {
        pub x: u16,
        pub y: u16,
    }

    fn render_digit(digit: &str) -> Vec<String> {
        match digit {
            "0" => lines!["█████",
                          "█   █",
                          "█   █",
                          "█   █",
                          "█████"],
            "1" => lines!["    █",
                          "    █",
                          "    █",
                          "    █",
                          "    █"],
            "2" => lines!["█████",
                          "   ██",
                          "█████",
                          "█    ",
                          "█████"],
            "3" => lines!["█████",
                          "    █",
                          "█████",
                          "    █",
                          "█████"],
            "4" => lines!["█   █",
                          "█   █",
                          "█████",
                          "    █",
                          "    █"],
            "5" => lines!["█████",
                          "█    ",
                          "█████",
                          "    █",
                          "█████"],
            "6" => lines!["█████",
                          "█    ",
                          "█████",
                          "█   █",
                          "█████"],
            "7" => lines!["█████",
                          "    █",
                          "    █",
                          "    █",
                          "    █"],
            "8" => lines!["█████",
                          "█   █",
                          "█████",
                          "█   █",
                          "█████"],
            "9" => lines!["█████",
                          "█   █",
                          "█████",
                          "    █",
                          "█████"],
            ":" => lines!["     ",
                          "  █  ",
                          "     ",
                          "  █  ",
                          "     "],
            " " => (0..5).map(|_| " ".to_owned()).collect::<Vec<_>>(),
            _ => (0..5).map(|_| "".to_owned()).collect::<Vec<_>>(),
        }
    }

    fn to_digits(countdown: &Countdown) -> Vec<String> {
        if let Some(left) = countdown.duration.checked_sub(countdown.running) {
            let total = countdown.duration.as_secs();
            let mut tmp = left.as_secs();
            let hours = tmp / 3600;
            tmp %= 3600;
            let minutes = tmp / 60;
            tmp %= 60;
            let seconds = tmp;
            format!("{}{:02}:{:02}",
                    maybe_val!(format!("{}:", hours), total >= SEC_IN_HOUR, ""),
                    minutes,
                    seconds)
                .split("")
                .collect::<Vec<&str>>()
                .join(" ")
                .split("")
                .map(|c| String::from(c))
                .collect::<Vec<String>>()
        } else {
            vec![]
        }
    }

    pub fn render(countdown: &Countdown, pos: &Position) -> String {
        let rendered_title = format!("{under}{bold}{title}",
                                     under=style::Underline,
                                     bold=style::Bold,
                                     title=countdown.title);
        let rendered_status = format!("{}",
                                      match countdown.state {
                                          State::Paused => "[PAUSED]",
                                          _ => "",
                                      });
        let mut lines: Vec<Vec<String>> = (2..7)
            .map(|idx| vec![format!("{}", cursor::Goto(pos.x, pos.y + idx))])
            .collect();
        let digits = to_digits(countdown);
        for digit in &digits {
            let mut digit_lines = render_digit(digit);
            for i in (0..5).rev() {
                lines[i].push(digit_lines.pop().unwrap());
            }
        }
        let lines_str = lines
            .into_iter()
            .map(|line| line.join(""))
            .collect::<Vec<String>>()
            .join("");
        format!("{clear}{pos}{reset}{title}{reset} {status}{lines}",
                clear=clear::All,
                pos=cursor::Goto(pos.x, pos.y),
                reset=style::Reset,
                title=rendered_title,
                status=rendered_status,
                lines=lines_str)
    }
}

fn main() {
    let stdout = io::stdout();
    let mut screen = AlternateScreen::from(
        stdout.lock().into_raw_mode().unwrap());
    let mut pomo = Pomodoro::new(async_stdin(), screen);
    pomo.run().unwrap();
}
