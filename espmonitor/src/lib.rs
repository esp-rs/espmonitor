// Copyright 2021 Brian J. Tarricone <brian@tarricone.org>
//
// This file is part of ESPMonitor.
//
// ESPMonitor is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// ESPMonitor is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with ESPMonitor.  If not, see <https://www.gnu.org/licenses/>.

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use lazy_static::lazy_static;
use serial::{self, BaudRate, SerialPort, SystemPort};
use regex::Regex;
use std::{
    ffi::{OsString, OsStr},
    io::{self, ErrorKind, Read, Write},
    path::Path,
    process::{Command, Stdio, exit},
    time::{Duration, Instant},
};

mod types;

pub use types::{AppArgs, Chip, Framework};

const DEFAULT_BAUD_RATE: BaudRate = BaudRate::Baud115200;
const UNFINISHED_LINE_TIMEOUT: Duration = Duration::from_secs(5);

lazy_static! {
    static ref LINE_SEP_RE: Regex = Regex::new("\r?\n")
        .expect("Failed to parse line separator regex");
    static ref FUNC_ADDR_RE: Regex = Regex::new(r"0x4[0-9a-f]{7}")
        .expect("Failed to parse program address regex");
    static ref ADDR2LINE_RE: Regex = Regex::new(r"^0x[0-9a-f]+:\s+([^ ]+)\s+at\s+(\?\?|[0-9]+):(\?|[0-9]+)")
        .expect("Failed to parse addr2line output regex");
}

macro_rules! rprintln {
    () => (print!("\r\n"));
    ($fmt:literal) => (print!(concat!($fmt, "\r\n")));
    ($fmt:literal, $($arg:tt)+) => (print!(concat!($fmt, "\r\n"), $($arg)*));
}

struct SerialState {
    unfinished_line: String,
    last_unfinished_line_at: Instant,
    bin: Option<OsString>,
    tool_prefix: &'static str,
}

#[cfg(unix)]
pub fn run(args: AppArgs) -> Result<(), Box<dyn std::error::Error>> {
    use nix::{sys::wait::{WaitStatus, waitpid}, unistd::{ForkResult, fork}};

    enable_raw_mode()?;

    match unsafe { fork() } {
        Err(err) => {
            disable_raw_mode()?;
            Err(err.into())
        },
        Ok(ForkResult::Parent { child }) => loop {
            match waitpid(child, None) {
                Ok(WaitStatus::Exited(_, status)) => {
                    disable_raw_mode()?;
                    exit(status);
                },
                Ok(WaitStatus::Signaled(_, _, _)) => {
                    disable_raw_mode()?;
                    exit(255);
                },
                _ => (),
            }
        },
        Ok(ForkResult::Child) => run_child(args),
    }
}

#[cfg(windows)]
pub fn run(args: AppArgs) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let result = run_child(args);
    disable_raw_mode()?;
    result
}

fn run_child(mut args: AppArgs) -> Result<(), Box<dyn std::error::Error>> {
    rprintln!("ESPMonitor {}", env!("CARGO_PKG_VERSION"));
    rprintln!();
    rprintln!("Commands:");
    rprintln!("    CTRL+R    Reset chip");
    rprintln!("    CTRL+C    Exit");
    rprintln!();

    let speed = args.speed.map(BaudRate::from_speed).unwrap_or(DEFAULT_BAUD_RATE);
    rprintln!("Opening {} with speed {}", args.serial, speed.speed());

    let mut dev = serial::open(&args.serial)?;
    dev.set_timeout(Duration::from_millis(200))?;
    dev.reconfigure(&|settings| {
        settings.set_baud_rate(speed)
    })?;

    if let Some(bin) = args.bin.as_ref() {
        if Path::new(bin).exists() {
            rprintln!("Using {} as flash image", bin.to_string_lossy());
        } else {
            rprintln!("WARNING: Flash image {} does not exist (you may need to build it)", bin.to_string_lossy());
        }
    }

    if args.reset {
        reset_chip(&mut dev)?;
    }

    let mut serial_state = SerialState {
        unfinished_line: String::new(),
        last_unfinished_line_at: Instant::now(),
        bin: args.bin.take(),
        tool_prefix: args.chip.tool_prefix(),
    };

    let mut buf = [0u8; 1024];
    loop {
        match dev.read(&mut buf) {
            Ok(bytes) if bytes > 0 => handle_serial(&mut serial_state, &buf[0..bytes])?,
            Ok(_) => if dev.read_dsr().is_err() {
                rprintln!("Device disconnected; exiting");
                break Ok(());
            },
            Err(err) if err.kind() == ErrorKind::TimedOut => (),
            Err(err) if err.kind() == ErrorKind::WouldBlock => (),
            Err(err) => break Err(err.into()),
        }

        while event::poll(Duration::ZERO)? {
            match event::read() {
                Ok(Event::Key(key_event)) => handle_input(&mut dev, key_event)?,
                Ok(_) => (),
                Err(err) => return Err(err.into()),
            }
        }
    }
}

fn reset_chip(dev: &mut SystemPort) -> io::Result<()> {
    print!("Resetting device... ");
    std::io::stdout().flush()?;
    dev.set_dtr(false)?;
    dev.set_rts(true)?;
    dev.set_rts(false)?;
    rprintln!("done");
    Ok(())
}

fn handle_serial(state: &mut SerialState, buf: &[u8]) -> io::Result<()> {
    let data = String::from_utf8_lossy(buf);
    let mut lines = LINE_SEP_RE.split(&data).collect::<Vec<&str>>();

    let new_unfinished_line =
        if data.ends_with('\n') {
            None
        } else {
            lines.pop()
        };

    for line in lines {
        let full_line =
            if !state.unfinished_line.is_empty() {
                state.unfinished_line.push_str(line);
                state.unfinished_line.as_str()
            } else {
                line
            };

        if !full_line.is_empty() {
            let processed_line = process_line(state, full_line);
            rprintln!("{}", processed_line);
            state.unfinished_line.clear();
        }
    }

    if let Some(nel) = new_unfinished_line {
        state.unfinished_line.push_str(nel);
        state.last_unfinished_line_at = Instant::now();
    } else if !state.unfinished_line.is_empty() && state.last_unfinished_line_at.elapsed() > UNFINISHED_LINE_TIMEOUT {
        let processed_line = process_line(state, &state.unfinished_line);
        rprintln!("{}", processed_line);
        state.unfinished_line.clear();
    }

    Ok(())
}

fn process_line(state: &SerialState, line: &str) -> String {
    let mut updated_line = line.to_string();

    if let Some(bin) = state.bin.as_ref() {
        for mat in FUNC_ADDR_RE.find_iter(line) {
            let cmd = format!("{}addr2line", state.tool_prefix);
            if let Some(output) = Command::new(&cmd)
                .args(&[OsStr::new("-pfiaCe"), bin, OsStr::new(mat.as_str())])
                .stdout(Stdio::piped())
                .output()
                .ok()
                .and_then(|output| String::from_utf8(output.stdout).ok())
            {
                if let Some(caps) = ADDR2LINE_RE.captures(&output) {
                    let name = format!("{} [{}:{}:{}]", mat.as_str().to_string(), caps[1].to_string(), caps[2].to_string(), caps[3].to_string());
                    updated_line = updated_line.replace(mat.as_str(), &name);
                }
            }
        }
    }

    updated_line
}

fn handle_input(dev: &mut SystemPort, key_event: KeyEvent) -> io::Result<()> {
    if key_event.modifiers == KeyModifiers::CONTROL {
        match key_event.code {
            KeyCode::Char('r') => reset_chip(dev),
            KeyCode::Char('c') => exit(0),
            _ => Ok(()),
        }
    } else {
        Ok(())
    }
}
