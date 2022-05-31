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

use clap::{ArgEnum, Parser};
use std::{
    convert::TryFrom,
    ffi::OsString,
    io::{Error as IoError, ErrorKind},
};

#[derive(Debug, Clone, Copy, PartialEq, ArgEnum)]
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
            Err(IoError::new(
                ErrorKind::InvalidInput,
                format!("Can't figure out framework from target '{}'", target),
            ))
        }
    }
}

impl TryFrom<&str> for Framework {
    type Error = IoError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "baremetal" => Ok(Framework::Baremetal),
            "esp-idf" | "espidf" => Ok(Framework::EspIdf),
            _ => Err(IoError::new(
                ErrorKind::InvalidInput,
                format!("'{}' is not a valid framework", value),
            )),
        }
    }
}

impl Default for Framework {
    fn default() -> Self {
        Framework::Baremetal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, ArgEnum)]
pub enum Chip {
    ESP32,
    ESP32S2,
    ESP8266,
    ESP32C3,
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
            Err(IoError::new(
                ErrorKind::InvalidInput,
                format!(
                    "Can't figure out chip from target '{}'; try specifying the --chip option",
                    target
                ),
            ))
        }
    }
}

impl Chip {
    pub fn target(&self, framework: Framework) -> String {
        let mut target = String::new();
        target.push_str(match self {
            Chip::ESP32C3 => "riscv32imc-",
            _ => "xtensa-",
        });
        target.push_str(match self {
            Chip::ESP32 => "esp32-",
            Chip::ESP32S2 => "esp32s2-",
            Chip::ESP8266 => "esp8266-",
            Chip::ESP32C3 => match framework {
                Framework::Baremetal => "unknown-",
                Framework::EspIdf => "esp-",
            },
        });
        target.push_str(match framework {
            Framework::Baremetal => "none-elf",
            Framework::EspIdf => "espidf",
        });
        target
    }
}

impl TryFrom<&str> for Chip {
    type Error = IoError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "esp32" => Ok(Chip::ESP32),
            "esp32c3" => Ok(Chip::ESP32C3),
            "esp8266" => Ok(Chip::ESP8266),
            _ => Err(IoError::new(
                ErrorKind::InvalidInput,
                format!("'{}' is not a valid chip", value),
            )),
        }
    }
}

impl Default for Chip {
    fn default() -> Self {
        Chip::ESP32
    }
}

#[derive(Parser, Debug)]
#[clap(author, version, about)]
pub struct AppArgs {
    /// Reset the chip on start [default]
    #[clap(short, long)]
    pub reset: bool,

    /// Do not reset the chip on start
    #[clap(long, conflicts_with("reset"))]
    pub no_reset: bool,

    /// Baud rate of serial device
    #[clap(long, short, default_value = "115200", name = "BAUD")]
    pub speed: usize,

    /// Path to executable matching what is on the device
    #[clap(long, short, name = "BINARY")]
    pub bin: Option<OsString>,

    /// Path to the serial device
    #[clap(name = "SERIAL_DEVICE")]
    pub serial: String,
}
