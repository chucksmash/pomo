# pomo

A quick and dirty Pomodoro timer that takes styling cues from the
`tmux` clock utility:

![Pomodoro Screenshot](https://raw.githubusercontent.com/chucksmash/pomo/master/docs/screenshot.png)

## Usage

See the output of the `pomo --help` invocation below:

```
USAGE:
    pomo [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -g, --goal <NAME>    Name of the current task you are working on.
                         (default: "")

    -t, --time <TIME>    Initial time (format: [[HH:]MM:]SS).
                         (default: 25:00 minutes)

```

When the timer is running, you can use the SPACE bar to pause the
countdown and `q` to quit.

### Examples

``` bash
$ # Create a new "Finish this README" timer
$ pomo --goal 'Finish this README"
```

``` bash
$ # Create an untitled countdown for one minute
$ pomo --time '1:00'
```

## Installation

This project is not currently available from
[crates.io](https://crates.io) or in binary form. To use it, you'll
need to clone the repo and compile from source. Once you have `rustc`
and `cargo` installed locally, the project is built via:

``` bash
$ git clone git@github.com:chucksmash/poma.git
$ cd poma
$ cargo build --release
```
