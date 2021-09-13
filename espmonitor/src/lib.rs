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

use lazy_static::lazy_static;
use mio::{Interest, Poll, Token, event::Events};
use mio_serial::{SerialPort, SerialPortBuilderExt, SerialStream};
use regex::Regex;
use std::{ffi::OsString, ffi::OsStr, path::Path, process::Stdio, time::Instant};
use std::io::{self, Error as IoError, ErrorKind, Read, Write};
use std::process::Command;
use std::time::Duration;

#[cfg(unix)]
use termios::{ISIG, OPOST, TCSAFLUSH, Termios, cfmakeraw, tcsetattr};

const DEFAULT_BAUD_RATE: u32 = 115_200;
const UNFINISHED_LINE_TIMEOUT: Duration = Duration::from_secs(5);

const SERIAL: Token = Token(0);
const STDIN: Token = Token(1);

const RESET_KEYCODE: u8 = 18;

lazy_static! {
    static ref FUNC_ADDR_RE: Regex = Regex::new(r"0x4[0-9a-f]{7}")
        .expect("Failed to parse program address regex");
    static ref ADDR2LINE_RE: Regex = Regex::new(r"^0x[0-9a-f]+:\s+([^ ]+)\s+at\s+(\?\?|[0-9]+):(\?|[0-9]+)")
        .expect("Failed to parse addr2line output regex");
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Framework {
    Baremetal,
    EspIdf,
}

impl Framework {
    pub fn from_target<S: AsRef<str>>(target: S) -> Result<Self, IoError> {
        let target = target.as_ref();
        if target.ends_with("-espidf") {
            Ok(Framework::EspIdf)
        } else if target.ends_with("-none-elf") {
            Ok(Framework::Baremetal)
        } else {
            Err(IoError::new(ErrorKind::InvalidInput, format!("Can't figure out framework from target '{}'", target)))
        }
    }
}

impl std::convert::TryFrom<&str> for Framework {
    type Error = IoError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "baremetal" => Ok(Framework::Baremetal),
            "esp-idf" | "espidf" => Ok(Framework::EspIdf),
            _ => Err(IoError::new(ErrorKind::InvalidInput, format!("'{}' is not a valid framework", value))),
        }
    }
}

impl Default for Framework {
    fn default() -> Self {
        Framework::Baremetal
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Chip {
    ESP32,
    ESP32S2,
    ESP8266,
}

impl Chip {
    pub fn from_target<S: AsRef<str>>(target: S) -> Result<Chip, IoError> {
        let target = target.as_ref();
        if target.contains("-esp32-") {
            Ok(Chip::ESP32)
        } else if target.contains("-esp32s2-") {
            Ok(Chip::ESP32S2)
        } else if target.contains("-esp8266-") {
            Ok(Chip::ESP8266)
        } else {
            Err(IoError::new(ErrorKind::InvalidInput, format!("Can't figure out chip from target '{}'", target)))
        }
    }
}

impl Chip {
    pub fn target(&self, framework: Framework) -> String {
        let mut target = String::from("xtensa-");
        target.push_str(match self {
            Chip::ESP32 => "esp32-",
            Chip::ESP32S2 => "esp32s2-",
            Chip::ESP8266 => "esp8266-",
        });
        target.push_str(match framework {
            Framework::Baremetal => "none-elf",
            Framework::EspIdf=> "espidf",
        });
        target
    }

    pub fn tool_prefix(&self) -> &'static str {
        match self {
            Chip::ESP32 => "xtensa-esp32-elf-",
            Chip::ESP32S2 => "xtensa-esp32s2-elf-",
            Chip::ESP8266 => "xtensa-esp8266-elf-",
        }
    }
}

impl std::convert::TryFrom<&str> for Chip {
    type Error = IoError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "esp32" => Ok(Chip::ESP32),
            "esp8266" => Ok(Chip::ESP8266),
            _ => Err(IoError::new(ErrorKind::InvalidInput, format!("'{}' is not a valid chip", value))),
        }
    }
}

impl Default for Chip {
    fn default() -> Self {
        Chip::ESP32
    }
}

#[derive(Debug)]
pub struct AppArgs {
    pub serial: String,
    pub chip: Chip,
    pub framework: Framework,
    pub speed: Option<u32>,
    pub reset: bool,
    pub bin: Option<OsString>,
}

struct SerialReader {
    dev: SerialStream,
    unfinished_line: String,
    last_unfinished_line_at: Instant,
    bin: Option<OsString>,
    tool_prefix: &'static str,
}

#[cfg(unix)]
pub fn run(args: AppArgs) -> Result<(), Box<dyn std::error::Error>> {
    use nix::{sys::wait::{WaitStatus, waitpid}, unistd::{ForkResult, fork}};
    use std::process::exit;

    let orig_tty_settings = set_tty_raw()?;

    match unsafe { fork() } {
        Err(err) => Err(err.into()),
        Ok(ForkResult::Parent { child }) => loop {
            match waitpid(child, None) {
                Ok(WaitStatus::Exited(_, status)) => {
                    restore_tty(&orig_tty_settings);
                    exit(status);
                },
                Ok(WaitStatus::Signaled(_, _, _)) => {
                    restore_tty(&orig_tty_settings);
                    exit(255);
                },
                _ => (),
            }
        }
        Ok(ForkResult::Child) => run_child(args),
    }
}

#[cfg(windows)]
pub fn run(args: AppArgs) -> Result<(), Box<dyn std::error::Error>> {
    run_child(args)
}

fn run_child(mut args: AppArgs) -> Result<(), Box<dyn std::error::Error>> {
    println!(concat!("ESPMonitor ", env!("CARGO_PKG_VERSION")));
    println!();
    println!("Commands:");
    println!("    CTRL+R    Reset chip");
    println!("    CTRL+C    Exit");
    println!();

    let speed = args.speed.unwrap_or(DEFAULT_BAUD_RATE);
    println!("Opening {} with speed {}", args.serial, speed);

    let mut dev = mio_serial::new(args.serial, speed)
        .timeout(Duration::from_millis(200))
        .open_native_async()?;

    if let Some(bin) = args.bin.as_ref() {
        if Path::new(bin).exists() {
            println!("Using {} as flash image", bin.to_string_lossy());
        } else {
            eprintln!("WARNING: Flash image {} does not exist (you may need to build it)", bin.to_string_lossy());
        }
    }

    if args.reset {
        reset_chip(&mut dev)?;
    }

    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(512);

    poll.registry().register(&mut dev, SERIAL, Interest::READABLE)?;
    #[cfg(unix)]
    poll.registry().register(&mut mio::unix::SourceFd(&0), STDIN, Interest::READABLE)?;

    let mut serial_reader = SerialReader {
        dev,
        unfinished_line: String::new(),
        last_unfinished_line_at: Instant::now(),
        bin: args.bin.take(),
        tool_prefix: args.chip.tool_prefix(),
    };

    loop {
        poll.poll(&mut events, None)?;
        for event in events.iter() {
            match event.token() {
                STDIN => handle_stdin(&mut serial_reader)?,
                SERIAL => handle_serial(&mut serial_reader)?,
                _ => (),
            }
        }
    }
}

#[cfg(unix)]
fn set_tty_raw() -> io::Result<Termios> {
    let orig_settings = Termios::from_fd(0)?;
    let mut raw_settings = orig_settings;
    cfmakeraw(&mut raw_settings);
    raw_settings.c_oflag |= OPOST; // Continue processing \n as \r\n for output
    raw_settings.c_lflag |= ISIG; // Allow signals (like ctrl+c -> SIGINT) through
    tcsetattr(0, TCSAFLUSH, &raw_settings)?;
    Ok(orig_settings)
}

#[cfg(unix)]
fn restore_tty(settings: &Termios) {
    if let Err(err) = tcsetattr(0, TCSAFLUSH, settings) {
        eprintln!("Failed to return terminal to cooked mode: {}", err);
    }
}

fn reset_chip(dev: &mut SerialStream) -> io::Result<()> {
    print!("Resetting device... ");
    std::io::stdout().flush()?;
    dev.write_data_terminal_ready(false)?;
    dev.write_request_to_send(true)?;
    dev.write_request_to_send(false)?;
    println!("done");
    Ok(())
}

fn handle_stdin(reader: &mut SerialReader) -> io::Result<()> {
    let mut buf = [0; 32];
    match io::stdin().read(&mut buf)? {
        bytes if bytes > 0 => {
            for b in buf[0..bytes].iter() {
                #[allow(clippy::single_match)]
                match *b {
                    RESET_KEYCODE => reset_chip(&mut reader.dev)?,
                    _ => (),
                }
            }
            Ok(())
        },
        _ => Ok(()),
    }
}

fn handle_serial(reader: &mut SerialReader) -> io::Result<()> {
    let mut buf = [0u8; 1024];
    loop {
        match reader.dev.read(&mut buf) {
            Ok(bytes) if bytes > 0 => {
                let data = String::from_utf8_lossy(&buf[0..bytes]);
                let mut lines = data.split('\n').collect::<Vec<&str>>();

                let new_unfinished_line =
                   if buf[bytes - 1] != b'\n' {
                       lines.pop()
                   } else {
                       None
                   };

                for line in lines {
                    let full_line =
                       if !reader.unfinished_line.is_empty() {
                           reader.unfinished_line.push_str(line);
                           reader.unfinished_line.as_str()
                       } else {
                           line
                       };

                    if !full_line.is_empty() {
                        let processed_line = process_line(reader, full_line);
                        println!("{}", processed_line);
                        reader.unfinished_line.clear();
                    }
                }

                if let Some(nel) = new_unfinished_line {
                    reader.unfinished_line.push_str(nel);
                    reader.last_unfinished_line_at = Instant::now();
                } else if !reader.unfinished_line.is_empty() && reader.last_unfinished_line_at.elapsed() > UNFINISHED_LINE_TIMEOUT {
                    println!("{}", reader.unfinished_line);
                    reader.unfinished_line.clear();
                }
            },
            Ok(_) => return Ok(()),
            Err(err) if err.kind() == ErrorKind::TimedOut => return Ok(()),
            Err(err) if err.kind() == ErrorKind::WouldBlock => return Ok(()),
            Err(err) => return Err(err),
        }
    }
}

fn process_line(reader: &SerialReader, line: &str) -> String {
    let mut updated_line = line.to_string();

    if let Some(bin) = reader.bin.as_ref() {
        for mat in FUNC_ADDR_RE.find_iter(line) {
            let cmd = format!("{}addr2line", reader.tool_prefix);
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
