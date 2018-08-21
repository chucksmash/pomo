extern crate chrono;
extern crate clap;
extern crate termion;

use std::default::Default;
use std::fmt;
use std::io::{self, Read, Write};
use std::ops::{Deref, DerefMut};
use std::thread::sleep;
use std::time::Duration;

use chrono::{DateTime, Duration as OldDuration, Local};
use clap::{App, Arg};
use termion::{async_stdin, clear, color, cursor, style};
use termion::raw::IntoRawMode;
use termion::screen::{self, AlternateScreen};

use self::timer::{Countdown, Position, Timer};

macro_rules! maybe_str {
    ( $val:expr, $test:expr ) => {
        {
            match $test {
                true => format!("{}", $val),
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
    current: Timer,
    stdin: R,
    stdout: W,
}

impl<R: Read, W: Write> Pomodoro<R, W> {
    fn new(stdin: R, stdout: W, timer: Timer) -> Pomodoro<R, W> {
        Pomodoro {
            current: timer,
            stdin,
            stdout,
        }
    }

    fn from_parts(
        stdin: R,
        stdout: W,
        name: String,
        duration: Duration
    ) -> Pomodoro<R, W> {
        let timer = Timer::Work(Countdown::new(duration, &name));
        Pomodoro::new(stdin, stdout, timer)
    }

    fn bell(&mut self) -> TermResult {
        write!(self.stdout, "\x07")
    }

    fn run(&mut self) -> TermResult {
        writeln!(self.stdout,
                 "{}{}{}",
                 clear::All,
                 cursor::Hide,
                 cursor::Goto(1, 1))?;

        while timer::State::Finished != self.current.tick() {
            let mut key_bytes = [0];
            self.stdin.read(&mut key_bytes)?;

            match key_bytes[0] {
                b'q' => break,
                b' ' => { self.current.toggle(); self.bell()?; },
                _ => {},
            }
            let rendered = timer::render(&self.current, &Position { x: 5, y: 5});
            let rendered_help = help::render(&Position { x: 5, y: 16 });
            write!(self.stdout, "{}", rendered)?;
            write!(self.stdout, "{}", rendered_help)?;
            self.stdout.flush()?;
            sleep(SLEEP);
        }
        self.cleanup()?;
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

mod parser {
    use std::time::Duration;

    pub fn parse_time(raw_time: &str) -> Result<Duration, ()> {
        let mut hours: u64 = 0;
        let mut minutes: u64 = 0;
        let mut seconds: u64 = 0;
        let mut total: u64 = 0;
        let parts: Vec<_> = raw_time.split(":").collect();
        // TODO: Handle overflow case without panicking
        match parts.len() {
            p if p == 3 => {
                hours = parts[0].parse::<u64>().or(Err(()))?;
                minutes = parts[1].parse::<u64>().or(Err(()))?;
                seconds = parts[2].parse::<u64>().or(Err(()))?;
                total = hours * 3600 + minutes * 60 + seconds;
                Ok(Duration::from_secs(total))
            },
            p if p == 2 => {
                minutes = parts[0].parse::<u64>().or(Err(()))?;
                seconds = parts[1].parse::<u64>().or(Err(()))?;
                total = minutes * 60 + seconds;
                Ok(Duration::from_secs(total))
            },
            p if p == 1 => {
                seconds = parts[0].parse::<u64>().or(Err(()))?;
                Ok(Duration::from_secs(seconds))
            },
            _ => Err(()),
        }
    }
}

mod help {
    use super::*;

    macro_rules! help {
        ( $key:expr, $prefix:expr, $suffix:expr ) => {
            {
                format!("{reset}{prefix}{bold}{key}{reset}{suffix}",
                        reset=style::Reset,
                        bold=style::Bold,
                        prefix=$prefix,
                        key=$key,
                        suffix=$suffix)
            }
        }
    }


    pub fn render(pos: &Position) -> String {
        let commands = vec![help!{"<SPACE>", "", ": pause/unpause"},
                            help!{"(q)", "", "uit"}]
            .join("   ");
        format!("{}{}", cursor::Goto(pos.x, pos.y), commands)
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
    }

    impl Default for Countdown {
        fn default() -> Countdown {
            Countdown::new(Duration::from_secs(0), "")
        }
    }

    pub enum Timer {
        Work(Countdown),
        Break(Countdown),
    }

    impl Deref for Timer {
        type Target = Countdown;

        fn deref(&self) -> &Self::Target {
            match self {
                Timer::Work(c) => &c,
                Timer::Break(c) => &c,
            }
        }
    }

    impl DerefMut for Timer {
        fn deref_mut(&mut self) -> &mut Countdown {
            match self {
                Timer::Work(ref mut c) => c,
                Timer::Break(ref mut c) => c,
            }
        }
    }

    impl Default for Timer {
        fn default() -> Timer {
            Timer::Work(Default::default())
        }
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
            ":" => lines!["   ",
                          " █ ",
                          "   ",
                          " █ ",
                          "   "],
            " " => (0..5).map(|_| " ".to_owned()).collect::<Vec<_>>(),
            _ => (0..5).map(|_| "".to_owned()).collect::<Vec<_>>(),
        }
    }

    fn to_digit_strs(countdown: &Countdown) -> Vec<String> {
        if let Some(left) = countdown.duration.checked_sub(countdown.running) {
            let total = countdown.duration.as_secs();
            let mut tmp = left.as_secs();
            let hours = tmp / 3600;
            tmp %= 3600;
            let minutes = tmp / 60;
            tmp %= 60;
            let seconds = tmp;
            format!("{}{:02}:{:02}",
                    maybe_str!(format!("{}:", hours), total >= SEC_IN_HOUR),
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
        let digits = to_digit_strs(countdown);
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

fn build_cli<'a, 'b>() -> clap::App<'a, 'b> {
    App::new("Pomo")
        .version("0.1.0")
        .author("Chuck Bassett <3101367+chucksmash@users.noreply.github.com>")
        .about("Quick and Dirty CLI Pomodoro Timer")
        .arg(Arg::with_name("goal")
             .long("goal")
             .short("g")
             .value_name("NAME")
             .help("Name of the current task you are working on.
(default: \"\")

")
             .takes_value(true))
        .arg(Arg::with_name("time")
             .long("time")
             .short("t")
             .value_name("TIME")
             .help("Initial time (format: [[HH:]MM:]SS).
(default: 25:00 minutes)

"))
}

fn main() {
    let matches = build_cli().get_matches();
    let raw_time = matches.value_of("time").unwrap_or("25:00");
    let time = parser::parse_time(raw_time).expect("Unable to parse time param");
    let name = matches.value_of("goal").unwrap_or("").to_string();

    let stdout = io::stdout();
    let screen = AlternateScreen::from(
        stdout.lock().into_raw_mode().unwrap());
    let mut pomo = Pomodoro::from_parts(async_stdin(),
                                        screen,
                                        name,
                                        time);
    pomo.run().unwrap();
}
