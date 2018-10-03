extern crate termion;

extern crate pomo;

fn main() {
    use std::io;

    use termion::async_stdin;
    use termion::raw::IntoRawMode;
    use termion::screen::AlternateScreen;

    use pomo::cli;
    use pomo::parser;
    use pomo::pomo::Pomodoro;

    let matches = cli::build_cli().get_matches();
    let raw_time = matches.value_of("time").unwrap_or("25:00");
    let time = parser::parse_time(raw_time).expect("Unable to parse time param");
    let name = matches.value_of("goal").unwrap_or("").to_string();

    let stdout = io::stdout();
    let screen = AlternateScreen::from(stdout.lock().into_raw_mode().unwrap());
    let mut pomo = Pomodoro::from_parts(async_stdin(), screen, name, time);
    pomo.run().unwrap();
}
