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

use espmonitor::{AppArgs, Chip, Framework, run};
use pico_args::Arguments;
use std::convert::TryFrom;
use std::error::Error;

fn main() {
    #[cfg(windows)]
    let _ = crossterm::ansi_support::supports_ansi();
    // supports_ansi() returns what it suggests, and as a side effect enables ANSI support

    match parse_args().and_then(|args| args.map(run).unwrap_or(Ok(()))) {
        Ok(_) => (),
        Err(err) => {
            println!("Error: {}", err);
            println!();
            if err.downcast::<pico_args::Error>().is_ok() {
                print_usage();
            }
            std::process::exit(1);
        },
    }
}

fn parse_args() -> Result<Option<AppArgs>, Box<dyn Error>> {
    let mut args = Arguments::from_env();
    if args.contains("-h") || args.contains("--help") {
        print_usage();
        Ok(None)
    } else if args.contains("-V") || args.contains("--version") {
        print_version();
        Ok(None)
    } else {
        #[allow(clippy::redundant_closure)]
        let chip = args.opt_value_from_fn("--chip", |s| Chip::try_from(s))?.unwrap_or_default();
        Ok(Some(AppArgs {
            chip,
            framework: Framework::default(),
            speed: args.opt_value_from_fn("--speed", |s| s.parse::<usize>())?,
            reset: args.contains("--reset") || !args.contains("--no-reset"),
            bin: args.opt_value_from_str("--bin")?,
            serial: args.free_from_str()?,
        }))
    }
}

fn print_usage() {
    let usage = "Usage: espmonitor [OPTIONS] SERIAL_DEVICE\n\
        \n\
        \x20   --chip {esp32|esp32c3|esp8266}   Which ESP chip to target\n\
        \x20   --reset                          Reset the chip on start (default)\n\
        \x20   --no-reset                       Do not reset thechip on start\n\
        \x20   --speed BAUD                     Baud rate of serial device (default: 115200)\n\
        \x20   --bin BINARY                     Path to executable matching what is on the device\n\
        \x20   --version                        Output version information and exit\n\
        \x20   SERIAL_DEVICE                    Path to the serial device";

    println!("{}", usage);
}

fn print_version() {
    println!("espmonitor {}", env!("CARGO_PKG_VERSION"));
}
