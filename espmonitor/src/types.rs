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

use std::{
    convert::TryFrom,
    ffi::OsString,
    io::{Error as IoError, ErrorKind},
};

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

impl TryFrom<&str> for Framework {
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

impl TryFrom<&str> for Chip {
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
    pub speed: Option<usize>,
    pub reset: bool,
    pub bin: Option<OsString>,
}
