extern crate chrono;
extern crate clap;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate termion;

mod events {
    use std::time::{Duration, Instant};

    use super::timer::State;

    #[derive(Debug)]
    struct Event {
        state: State,
        time: Instant,
    }

    #[derive(Debug)]
    pub struct Logger {
        title: String,
        states: Vec<Event>,
    }

    impl Logger {
        pub fn new(title: &str) -> Logger {
            Logger {
                title: title.to_owned(),
                states: vec![],
            }
        }

        pub fn log(&mut self, state: State) {
            let ref mut states = self.states;
            let len = states.len();
            if len == 0 || states[len - 1].state != state {
                states.push(Event {
                    state,
                    time: Instant::now(),
                })
            }
        }

        pub fn format(&self) -> Formatted {
            Formatted::from(&self)
        }
    }

    #[derive(Serialize)]
    struct Span {
        state: State,
        duration: String,
    }

    impl Span {
        fn from(start: &Event, end: &Event) -> Span {
            let d = end.time - start.time;
            let secs = d.as_secs();
            let tenths = d.subsec_millis() / 100;
            Span {
                state: start.state,
                duration: format!("{}.{}", secs, tenths),
            }
        }
    }

    #[derive(Serialize)]
    pub struct Formatted {
        title: String,
        events: Vec<Span>,
    }

    impl Formatted {
        fn from(logger: &Logger) -> Formatted {
            Formatted {
                title: logger.title.clone(),
                events: logger
                    .states
                    .windows(2)
                    .map(|evs| Span::from(&evs[0], &evs[1]))
                    .collect::<Vec<_>>(),
            }
        }
    }
}

pub mod pomo {
    use std::io::{self, Read, Write};
    use std::thread::sleep;
    use std::time::Duration;

    use termion::{clear, cursor, screen, style};

    use super::card;
    use super::events::Logger;
    use super::help;
    use super::timer;
    use super::timer::Countdown;

    type TermResult = Result<(), io::Error>;

    const FAREWELL_BELLS: u16 = 5;
    const SLEEP: Duration = Duration::from_millis(100);

    pub struct Pomodoro<R, W> {
        current: Countdown,
        logger: Logger,
        stdin: R,
        stdout: W,
    }

    impl<R: Read, W: Write> Pomodoro<R, W> {
        pub fn new(stdin: R, stdout: W, counter: Countdown, logger: Logger) -> Pomodoro<R, W> {
            Pomodoro {
                current: counter,
                logger,
                stdin,
                stdout,
            }
        }

        pub fn from_parts(stdin: R, stdout: W, name: String, duration: Duration) -> Pomodoro<R, W> {
            let counter = Countdown::new(duration, &name);
            let logger = Logger::new(&name);
            Pomodoro::new(stdin, stdout, counter, logger)
        }

        fn ring_once(&mut self) -> TermResult {
            write!(self.stdout, "\x07")?;
            self.stdout.flush()?;
            Ok(())
        }

        fn ring(&mut self, times: u16, delay: Duration) -> TermResult {
            let mut rings = 0;
            while rings < times {
                self.ring_once()?;
                rings += 1;
                sleep(delay);
            }
            Ok(())
        }

        pub fn run(&mut self) -> TermResult {
            writeln!(
                self.stdout,
                "{}{}{}",
                clear::All,
                cursor::Hide,
                cursor::Goto(1, 1)
            )?;

            // loop-and-a-half
            loop {
                let curr_state = self.current.tick();
                self.logger.log(curr_state);
                if curr_state == timer::State::Finished {
                    break;
                }
                let mut key_bytes = [0];
                self.stdin.read(&mut key_bytes)?;

                match key_bytes[0] {
                    b'q' => {
                        self.current.finish();
                        continue;
                    }
                    b' ' => {
                        self.current.toggle();
                        self.ring_once()?;
                    }
                    _ => {}
                }
                write!(self.stdout, "{}", clear::All);
                let card_dims = card::Dims {
                    x: 3,
                    y: 2,
                    height: 15,
                    width: 50,
                };
                let rendered_card = card::render(&card_dims);
                let rendered = timer::render(&self.current, &card::Position { x: 5, y: 3 });
                let rendered_help = help::render(&card::Position { x: 5, y: 15 });
                write!(self.stdout, "{}", rendered_card)?;
                write!(self.stdout, "{}", rendered)?;
                write!(self.stdout, "{}", rendered_help)?;
                self.stdout.flush()?;
                sleep(SLEEP);
            }
            if timer::State::Finished == self.current.tick() {
                self.ring(FAREWELL_BELLS, SLEEP * 3)?;
            }

            self.cleanup()?;
            Ok(())
        }

        fn cleanup(&mut self) -> TermResult {
            write!(
                self.stdout,
                "{}{}{}{}{}{}\n",
                cursor::Goto(1, 1),
                clear::All,
                cursor::Goto(1, 1),
                cursor::Show,
                screen::ToMainScreen,
                style::Reset
            )?;
            let s = serde_json::to_string_pretty(&self.logger.format())?;
            writeln!(self.stdout, "{}\r\n", s.replace("\n", "\r\n"),)
        }
    }
}

pub mod parser {
    use std::time::Duration;

    fn parse_part(s: &str) -> Result<u64, ()> {
        s.parse::<u64>().or(Err(()))
    }

    pub fn parse_time(raw_time: &str) -> Result<Duration, ()> {
        let parts: Vec<_> = raw_time.split(":").collect();
        // TODO: Handle overflow case without panicking
        match parts.len() {
            p if p == 3 => {
                let hours = parse_part(parts[0])?;
                let minutes = parse_part(parts[1])?;
                let seconds = parse_part(parts[2])?;
                let total = hours * 3600 + minutes * 60 + seconds;
                Ok(Duration::from_secs(total))
            }
            p if p == 2 => {
                let minutes = parse_part(parts[0])?;
                let seconds = parse_part(parts[1])?;
                let total = minutes * 60 + seconds;
                Ok(Duration::from_secs(total))
            }
            p if p == 1 => {
                let seconds = parse_part(parts[0])?;
                Ok(Duration::from_secs(seconds))
            }
            _ => Err(()),
        }
    }
}

mod help {
    use termion::cursor;

    use super::card::Position;

    macro_rules! help {
        ( $key:expr, $prefix:expr, $suffix:expr ) => {{
            use termion::style;
            format!(
                "{reset}{prefix}{bold}{key}{reset}{suffix}",
                reset = style::Reset,
                bold = style::Bold,
                prefix = $prefix,
                key = $key,
                suffix = $suffix
            )
        }};
    }

    pub fn render(pos: &Position) -> String {
        let commands = vec![
            help!{"<SPACE>", "", ": pause/unpause"},
            help!{"(q)", "", "uit"},
        ].join("   ");
        format!("{}{}", cursor::Goto(pos.x, pos.y), commands)
    }
}

mod card {
    use termion::{cursor, style};

    pub struct Dims {
        pub x: u16,
        pub y: u16,
        pub height: u16,
        pub width: u16,
    }

    pub struct Position {
        pub x: u16,
        pub y: u16,
    }

    pub fn render(dims: &Dims) -> String {
        let &Dims {
            x,
            y,
            height,
            width,
        } = dims;
        let w = width as usize;
        let mut rows = vec![];
        for offset in 0..height {
            rows.push(match offset {
                o if o == 0 => format!(
                    "{loc}{reset}{left}{empty:━>width$}{right}{reset}",
                    loc = cursor::Goto(x, y + o),
                    reset = style::Reset,
                    left = "┏",
                    empty = "",
                    width = w,
                    right = "┓"
                ),
                o if o == 2 => format!(
                    "{loc}{reset}{side}{linner}{empty:─>width$}{rinner}{side}{reset}",
                    loc = cursor::Goto(x, y + o),
                    reset = style::Reset,
                    side = "┃",
                    linner = "╶",
                    empty = "",
                    width = w - 2,
                    rinner = "╴"
                ),
                o if o == height - 3 => format!(
                    "{loc}{reset}{side}{linner}{empty:─>width$}{rinner}{side}{reset}",
                    loc = cursor::Goto(x, y + o),
                    reset = style::Reset,
                    side = "┃",
                    linner = "╶",
                    empty = "",
                    width = w - 2,
                    rinner = "╴"
                ),
                o if o == height - 1 => format!(
                    "{loc}{reset}{left}{empty:━>width$}{right}{reset}",
                    loc = cursor::Goto(x, y + o),
                    reset = style::Reset,
                    left = "┗",
                    empty = "",
                    width = w,
                    right = "┛"
                ),
                o => format!(
                    "{loc}{reset}{side}{empty:width$}{side}{reset}",
                    loc = cursor::Goto(x, y + o),
                    reset = style::Reset,
                    side = "┃",
                    empty = "",
                    width = w
                ),
            });
        }
        rows.join("")
    }
}

mod timer {
    use std::default::Default;
    use std::time::Duration;

    use chrono::{DateTime, Duration as OldDuration, Local};
    use termion::{cursor, style};

    use super::card::Position;

    macro_rules! maybe_str {
        ( $val:expr, $test:expr ) => {{
            match $test {
                true => format!("{}", $val),
                false => "".to_owned(),
            }
        }};
    }

    macro_rules! lines {
        ( $( $line:expr ),* ) => {
            {
                use termion::color;
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

    const SEC_IN_MINUTE: u64 = 60;
    const SEC_IN_HOUR: u64 = SEC_IN_MINUTE * 60;

    #[derive(Clone, Copy, Debug, PartialEq, Serialize)]
    pub enum State {
        Running,
        Paused,
        Finished,
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
                Running => {
                    self.running = elapsed - self.paused;
                }
                Paused => {
                    self.paused = elapsed - self.running;
                }
                _ => {}
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

        pub fn finish(&mut self) {
            self.state = State::Finished;
        }
    }

    impl Default for Countdown {
        fn default() -> Countdown {
            Countdown::new(Duration::from_secs(0), "")
        }
    }

    fn render_digit(digit: &str) -> Vec<String> {
        match digit {
            "0" => lines![
                "█████",
                "█   █",
                "█   █",
                "█   █",
                "█████"
            ],
            "1" => lines!["    █", "    █", "    █", "    █", "    █"],
            "2" => lines![
                "█████",
                "   ██",
                "█████",
                "█    ",
                "█████"
            ],
            "3" => lines![
                "█████",
                "    █",
                "█████",
                "    █",
                "█████"
            ],
            "4" => lines![
                "█   █",
                "█   █",
                "█████",
                "    █",
                "    █"
            ],
            "5" => lines![
                "█████",
                "█    ",
                "█████",
                "    █",
                "█████"
            ],
            "6" => lines![
                "█████",
                "█    ",
                "█████",
                "█   █",
                "█████"
            ],
            "7" => lines![
                "█████",
                "    █",
                "    █",
                "    █",
                "    █"
            ],
            "8" => lines![
                "█████",
                "█   █",
                "█████",
                "█   █",
                "█████"
            ],
            "9" => lines![
                "█████",
                "█   █",
                "█████",
                "    █",
                "█████"
            ],
            ":" => lines!["   ", " █ ", "   ", " █ ", "   "],
            " " => (0..5).map(|_| " ".to_owned()).collect::<Vec<_>>(),
            _ => (0..5).map(|_| "".to_owned()).collect::<Vec<_>>(),
        }
    }

    fn to_digit_strs(countdown: &Countdown) -> Vec<String> {
        if let Some(left) = countdown.duration.checked_sub(countdown.running) {
            let total = countdown.duration.as_secs();
            let as_secs = left.as_secs();
            // we fudge a little bit so that when you ask for 5 seconds
            // you see the value 5 for the first second and also so that
            // the countdown ends on zero.
            let mut tmp = if as_secs > 0 || left.subsec_millis() > 150 {
                as_secs + 1
            } else {
                0
            };
            let hours = tmp / 3600;
            tmp %= 3600;
            let minutes = tmp / 60;
            tmp %= 60;
            let seconds = tmp;
            format!(
                "{}{:02}:{:02}",
                maybe_str!(format!("{}:", hours), total >= SEC_IN_HOUR),
                minutes,
                seconds
            ).split("")
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
        let rendered_title = format!(
            "{under}{bold}{title}",
            under = style::Underline,
            bold = style::Bold,
            title = countdown.title
        );
        let rendered_status = format!(
            "{}",
            match countdown.state {
                State::Paused => "[PAUSED]",
                _ => "",
            }
        );
        let mut lines: Vec<Vec<String>> = (4..9)
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
        format!(
            "{pos}{reset}{title}{reset} {status}{lines}",
            pos = cursor::Goto(pos.x, pos.y),
            reset = style::Reset,
            title = rendered_title,
            status = rendered_status,
            lines = lines_str
        )
    }
}

pub mod cli {
    use clap::{App, Arg};

    pub fn build_cli<'a, 'b>() -> clap::App<'a, 'b> {
        App::new("Pomo")
            .version("0.1.0")
            .author("Chuck Bassett <3101367+chucksmash@users.noreply.github.com>")
            .about("Quick and Dirty CLI Pomodoro Timer")
            .arg(
                Arg::with_name("goal")
                    .long("goal")
                    .short("g")
                    .value_name("NAME")
                    .help(
                        "Name of the current task you are working on.
(default: \"\")

",
                    ).takes_value(true),
            ).arg(
                Arg::with_name("time")
                    .long("time")
                    .short("t")
                    .value_name("TIME")
                    .help(
                        "Initial time (format: [[HH:]MM:]SS).
(default: 25:00 minutes)

",
                    ),
            )
    }
}
