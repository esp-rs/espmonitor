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

use cargo_project::{Artifact, Profile, Project};
use espmonitor::{AppArgs, Chip, run};
use pico_args::Arguments;
use std::convert::TryFrom;
use std::env;
use std::ffi::OsString;

fn main() {
    match parse_args().map(|args| args.map(run)) {
        Ok(_) => (),
        Err(err) => {
            println!("Error: {}", err);
            println!();
            print_usage();
            std::process::exit(1);
        },
    }
}

fn parse_args() -> Result<Option<AppArgs>, Box<dyn std::error::Error>> {
    // Skip 'cargo espmonitor'
    let args = env::args().skip(2).map(OsString::from).collect();
    let mut args = Arguments::from_vec(args);

    if args.contains("-h") || args.contains("--help") {
        print_usage();
        Ok(None)
    } else {
        #[allow(clippy::redundant_closure)]
        let chip = args.opt_value_from_fn("--chip", |s| Chip::try_from(s))?.unwrap_or_default();
        let release = args.contains("--release");
        let example: Option<String> = args.opt_value_from_str("--example")?;

        let project = Project::query(".").unwrap();
        let artifact = match example.as_ref() {
            Some(example) => Artifact::Example(example.as_str()),
            None => Artifact::Bin(project.name()),
        };
        let profile = if release { Profile::Release } else { Profile::Dev };

        let host = "x86_64-unknown-linux-gnu";  // FIXME: does this even matter?
        let bin = project.path(artifact, profile, Some(chip.target()), host)?;

        Ok(Some(AppArgs {
            chip,
            reset: args.contains("--reset") || !args.contains("--no-reset"),
            speed: args.opt_value_from_fn("--speed", |s| s.parse::<usize>())?,
            bin: Some(bin.as_os_str().to_os_string()),
            serial: args.free_from_str()?,
        }))
    }
}

fn print_usage() {
    let usage = "Usage: cargo espflash [OPTIONS] SERIAL_DEVICE\n\
        \n\
        \x20   --chip {esp32|esp8266}   Which ESP chip to target\n\
        \x20   --release                Use the release build\n\
        \x20   --example EXAMPLE        Use the named example app binary\n\
        \x20   --reset                  Reset the chip on start (default)\n\
        \x20   --no-reset               Do not reset thechip on start\n\
        \x20   --speed BAUD             Baud rate of serial device (default: 115200)\n\
        \x20   SERIAL_DEVICE            Path to the serial device";

    println!("{}", usage);
}
