// Copyright 2021 Brian J. Tarricone <brian@tarricone.org>
//
// This file is part of ESPFlash.
//
// ESPFLash is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// ESPFlash is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with ESPFlash.  If not, see <https://www.gnu.org/licenses/>.

use lazy_static::lazy_static;
use regex::Regex;
use serial_core::{BaudRate, SerialDevice, SerialPortSettings};
use std::{ffi::OsString, ffi::OsStr, path::Path, process::Stdio, time::Instant};
use std::io::{Error as IoError, ErrorKind, Read, Write};
use std::process::Command;
use std::time::Duration;

const DEFAULT_BAUD_RATE: usize = 115_200;
const UNFINISHED_LINE_TIMEOUT: Duration = Duration::from_secs(5);

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
            Chip::ESP8266 => "esp82660",
        });
        target.push_str(match framework {
            Framework::Baremetal => "none-elf",
            Framework::EspIdf=> "espidf",
        });
        target
    }

    pub fn tool_prefix(&self) -> &str {
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
    pub serial: OsString,
    pub chip: Chip,
    pub framework: Framework,
    pub speed: Option<usize>,
    pub reset: bool,
    pub bin: Option<OsString>,
}

pub fn run(args: AppArgs) -> Result<(), Box<dyn std::error::Error>> {
    let speed = args.speed.unwrap_or(DEFAULT_BAUD_RATE);
    println!("Opening {} with speed {}", args.serial.to_string_lossy(), speed);

    if let Some(bin) = args.bin.as_ref() {
        if Path::new(bin).exists() {
            println!("Using {} as flash image", bin.to_string_lossy());
        } else {
            eprintln!("WARNING: Flash image {} does not exist (you may need to build it)", bin.to_string_lossy());
        }
    }

    let mut dev = serial::open(&args.serial)?;
    dev.set_timeout(Duration::from_millis(200))?;
    let mut settings = dev.read_settings()?;
    settings.set_baud_rate(BaudRate::from_speed(speed))?;
    dev.write_settings(&settings)?;

    let mut unfinished_line: String = String::new();
    let mut last_unfinished_line_at = Instant::now();
    let mut buf = [0u8; 1024];

    if args.reset {
        print!("Resetting device... ");
        std::io::stdout().flush()?;
        dev.set_dtr(false)?;
        dev.set_rts(true)?;
        dev.set_rts(false)?;
        println!("done");
    }

    loop {
        match dev.read(&mut buf) {
            Ok(bytes) if bytes > 0 => {
                let data = String::from_utf8_lossy(&buf[0..bytes]);
                let mut lines = data.split('\n').collect::<Vec<&str>>();

                let new_unfinished_line =
                    if buf[bytes-1] != b'\n' {
                        lines.pop()
                    } else {
                        None
                    };

                for line in lines {
                    let full_line =
                        if !unfinished_line.is_empty() {
                            unfinished_line.push_str(line);
                            unfinished_line.as_str()
                        } else {
                            line
                        };

                    if !full_line.is_empty() {
                        let processed_line = process_line(&args, full_line);
                        println!("{}", processed_line);
                        unfinished_line.clear();
                    }
                }

                if let Some(nel) = new_unfinished_line {
                    unfinished_line.push_str(nel);
                    last_unfinished_line_at = Instant::now();
                } else if !unfinished_line.is_empty() && last_unfinished_line_at.elapsed() > UNFINISHED_LINE_TIMEOUT {
                    println!("{}", unfinished_line);
                    unfinished_line.clear();
                }
            },
            Ok(_) => (),
            Err(err) if err.kind() == ErrorKind::TimedOut => (),
            Err(err) => return Err(err.into()),
        };
    }
}

fn process_line(args: &AppArgs, line: &str) -> String {
    let mut updated_line = line.to_string();

    if let Some(bin) = args.bin.as_ref() {
        for mat in FUNC_ADDR_RE.find_iter(line) {
            let cmd = format!("{}addr2line", args.chip.tool_prefix());
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
