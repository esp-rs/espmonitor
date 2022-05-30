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

use addr2line::Context;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    style::{Color, Print, PrintStyledContent, Stylize},
    terminal::{disable_raw_mode, enable_raw_mode},
    QueueableCommand,
};
use gimli::{EndianRcSlice, RunTimeEndian};
use lazy_static::lazy_static;
use object::read::Object;
use regex::Regex;
use serial::{self, BaudRate, SerialPort, SystemPort};
use std::{
    fs,
    io::{self, stdout, ErrorKind, Read, Write},
    process::exit,
    time::{Duration, Instant},
};

mod types;

pub use types::{AppArgs, Chip, Framework};

const UNFINISHED_LINE_TIMEOUT: Duration = Duration::from_secs(5);

lazy_static! {
    static ref LINE_SEP_RE: Regex =
        Regex::new("\r?\n").expect("Failed to parse line separator regex");
    static ref FUNC_ADDR_RE: Regex =
        Regex::new(r"0x4[0-9a-fA-F]{7}").expect("Failed to parse program address regex");
}

macro_rules! rprintln {
    () => (print!("\r\n"));
    ($fmt:literal) => (print!(concat!($fmt, "\r\n")));
    ($fmt:literal, $($arg:tt)+) => (print!(concat!($fmt, "\r\n"), $($arg)*));
}

pub struct Symbols<'a> {
    obj: object::read::File<'a, &'a [u8]>,
    context: Context<EndianRcSlice<RunTimeEndian>>,
}

pub struct SerialState<'a> {
    unfinished_line: String,
    last_unfinished_line_at: Instant,
    symbols: Option<Symbols<'a>>,
}

impl<'a> SerialState<'a> {
    pub fn new(symbols: Option<Symbols<'a>>) -> Self {
        Self {
            unfinished_line: "".to_owned(),
            last_unfinished_line_at: Instant::now(),
            symbols,
        }
    }
}

#[cfg(unix)]
pub fn run(args: AppArgs) -> Result<(), Box<dyn std::error::Error>> {
    use nix::{
        sys::wait::{waitpid, WaitStatus},
        unistd::{fork, ForkResult},
    };

    enable_raw_mode()?;

    match unsafe { fork() } {
        Err(err) => {
            disable_raw_mode()?;
            Err(err.into())
        }
        Ok(ForkResult::Parent { child }) => loop {
            match waitpid(child, None) {
                Ok(WaitStatus::Exited(_, status)) => {
                    disable_raw_mode()?;
                    exit(status);
                }
                Ok(WaitStatus::Signaled(_, _, _)) => {
                    disable_raw_mode()?;
                    exit(255);
                }
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

fn run_child(args: AppArgs) -> Result<(), Box<dyn std::error::Error>> {
    rprintln!("ESPMonitor {}", env!("CARGO_PKG_VERSION"));
    rprintln!();
    rprintln!("Commands:");
    rprintln!("    CTRL+R    Reset chip");
    rprintln!("    CTRL+C    Exit");
    rprintln!();

    let speed = BaudRate::from_speed(args.speed);
    rprintln!("Opening {} with speed {}", args.serial, speed.speed());

    let mut dev = serial::open(&args.serial)?;
    dev.set_timeout(Duration::from_millis(200))?;

    // The only thing we reconfigure and that could thus cause an error is the baud rate setting.
    // Hence we can explicitly handle this case here and give the user a better idea of which part
    // of their input was actually invalid.
    dev.reconfigure(&|settings| settings.set_baud_rate(speed))
        .map_err(|err| {
            if let serial::ErrorKind::InvalidInput = err.kind() {
                format!("Baud rate {} not supported by hardware", speed.speed())
            } else {
                format!("{}", err)
            }
        })?;

    let bin_data = args
        .bin
        .as_ref()
        .and_then(|bin_name| match fs::read(bin_name) {
            Ok(bin_data) => {
                rprintln!("Using {} as flash image", bin_name.to_string_lossy());
                Some(bin_data)
            }
            Err(err) => {
                rprintln!(
                    "WARNING: Unable to open flash image {}: {}",
                    bin_name.to_string_lossy(),
                    err
                );
                None
            }
        });

    let symbols =
        bin_data
            .as_ref()
            .and_then(|bin_data| match load_bin_context(bin_data.as_slice()) {
                Ok(symbols) => Some(symbols),
                Err(err) => {
                    rprintln!("WARNING: Failed to parse flash image: {}", err);
                    None
                }
            });

    if args.reset {
        reset_chip(&mut dev)?;
    }

    let mut serial_state = SerialState {
        unfinished_line: String::new(),
        last_unfinished_line_at: Instant::now(),
        symbols,
    };

    let mut output = stdout();
    let mut buf = [0u8; 1024];
    loop {
        match dev.read(&mut buf) {
            Ok(bytes) if bytes > 0 => {
                handle_serial(&mut serial_state, &buf[0..bytes], &mut output)?
            }
            Ok(_) => {
                if dev.read_dsr().is_err() {
                    rprintln!("Device disconnected; exiting");
                    break Ok(());
                }
            }
            Err(err) if err.kind() == ErrorKind::TimedOut => (),
            Err(err) if err.kind() == ErrorKind::WouldBlock => (),
            Err(err) if err.kind() == ErrorKind::Interrupted => (),
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

pub fn load_bin_context(data: &[u8]) -> Result<Symbols, Box<dyn std::error::Error + 'static>> {
    let obj = object::File::parse(data)?;
    let context = Context::new(&obj)?;
    Ok(Symbols { obj, context })
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

pub fn handle_serial(
    state: &mut SerialState,
    buf: &[u8],
    output: &mut dyn Write,
) -> io::Result<()> {
    let data = String::from_utf8_lossy(buf);
    let mut lines = LINE_SEP_RE.split(&data).collect::<Vec<&str>>();

    let new_unfinished_line = if data.ends_with('\n') {
        None
    } else {
        lines.pop()
    };

    for line in lines {
        let full_line = if !state.unfinished_line.is_empty() {
            state.unfinished_line.push_str(line);
            state.unfinished_line.as_str()
        } else {
            line
        };

        if !full_line.is_empty() {
            output_line(state, full_line, output)?;
            state.unfinished_line.clear();
        }
    }

    if let Some(nel) = new_unfinished_line {
        state.unfinished_line.push_str(nel);
        state.last_unfinished_line_at = Instant::now();
    } else if !state.unfinished_line.is_empty()
        && state.last_unfinished_line_at.elapsed() > UNFINISHED_LINE_TIMEOUT
    {
        output_line(state, &state.unfinished_line, output)?;
        state.unfinished_line.clear();
    }

    Ok(())
}

pub fn output_line(state: &SerialState, line: &str, output: &mut dyn Write) -> io::Result<()> {
    output.queue(Print(line.to_string()))?;

    if let Some(symbols) = state.symbols.as_ref() {
        for mat in FUNC_ADDR_RE.find_iter(line) {
            let (function, file, lineno) = u64::from_str_radix(&mat.as_str()[2..], 16)
                .ok()
                .map(|addr| {
                    let function = find_function_name(symbols, addr);
                    let (file, lineno) = find_location(symbols, addr);
                    (function, file, lineno)
                })
                .unwrap_or((None, None, None));

            fn or_qq(s: Option<String>) -> String {
                s.unwrap_or_else(|| "??".to_string())
            }

            let symbolicated_name = format!(
                "\r\n{} - {}\r\n    at {}:{}",
                mat.as_str(),
                or_qq(function),
                or_qq(file),
                or_qq(lineno.map(|l| l.to_string())),
            )
            .with(Color::Yellow);
            output.queue(PrintStyledContent(symbolicated_name))?;
        }
    }

    output.write_all(b"\r\n")?;
    output.flush()?;

    Ok(())
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

pub fn find_function_name(symbols: &Symbols<'_>, addr: u64) -> Option<String> {
    symbols
        .context
        .find_frames(addr)
        .ok()
        .and_then(|mut frames| frames.next().ok().flatten())
        .and_then(|frame| {
            frame
                .function
                .and_then(|f| f.demangle().ok().map(|c| c.into_owned()))
        })
        .or_else(|| {
            symbols
                .obj
                .symbol_map()
                .get(addr)
                .map(|sym| sym.name().to_string())
        })
}

pub fn find_location(symbols: &Symbols<'_>, addr: u64) -> (Option<String>, Option<u32>) {
    symbols
        .context
        .find_location(addr)
        .ok()
        .map(|location| {
            (
                location
                    .as_ref()
                    .and_then(|location| location.file)
                    .map(|file| file.to_string()),
                location.as_ref().and_then(|location| location.line),
            )
        })
        .unwrap_or((None, None))
}
