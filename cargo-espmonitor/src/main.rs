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

use cargo_project::{Artifact, Profile, Project};
use espmonitor::{AppArgs, Chip, Framework, run};
use pico_args::Arguments;
use std::{
    convert::TryFrom,
    env,
    error::Error,
    ffi::OsString,
    io,
    process::Command,
};

const DEFAULT_FLASH_BAUD_RATE: u32 = 460_800;

struct CargoAppArgs {
    flash: bool,
    flash_speed: u32,
    release: bool,
    example: Option<String>,
    features: Option<String>,
    app_args: AppArgs,
}

fn main() {
    // Skip first two args ('cargo', 'espmonitor')
    let args = env::args().skip(2).map(OsString::from).collect();

    if let Err(err) = parse_args(args).and_then(|cargo_app_args|
        cargo_app_args
            .map(|mut cargo_app_args| {
                if cargo_app_args.flash {
                    run_flash(&mut cargo_app_args)?;
                }
                run(cargo_app_args.app_args)
            })
            .unwrap_or(Ok(()))
    ) {
        eprintln!("Error: {}", err);
        eprintln!();
        if err.downcast::<pico_args::Error>().is_ok() {
            print_usage();
        }
        std::process::exit(1);
    }
}

fn run_flash(cargo_app_args: &mut CargoAppArgs) -> Result<(), Box<dyn Error>> {
    let mut args = vec!["espflash".to_string()];
    if cargo_app_args.release {
        args.push("--release".to_string());
    }
    if let Some(example) = cargo_app_args.example.take() {
        args.push("--example".to_string());
        args.push(example);
    }
    if let Some(features) = cargo_app_args.features.take() {
        args.push("--features".to_string());
        args.push(features);
    }
    args.push("--speed".to_string());
    args.push(cargo_app_args.flash_speed.to_string());
    args.push(cargo_app_args.app_args.serial.clone());

    let status = Command::new("cargo")
        .args(&args[..])
        .spawn()?
        .wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, "Flash failed".to_string()).into())
    }
}

fn parse_args(args: Vec<OsString>) -> Result<Option<CargoAppArgs>, Box<dyn Error>> {
    let mut args = Arguments::from_vec(args);

    if args.contains("-h") || args.contains("--help") {
        print_usage();
        Ok(None)
    } else if args.contains("-V") || args.contains("--version") {
        print_version();
        Ok(None)
    } else {
        let (chip, framework) = match args.opt_value_from_str::<&str, String>("--target")? {
            Some(ref target) => (
                Chip::from_target(target)?,
                Framework::from_target(target)?,
            ),
            None => (
                #[allow(clippy::redundant_closure)]
                args.opt_value_from_fn("--chip", |s| Chip::try_from(s))?.unwrap_or_default(),
                #[allow(clippy::redundant_closure)]
                args.opt_value_from_fn("--framework", |s| Framework::try_from(s))?.unwrap_or_default(),
            )
        };

        let release = args.contains("--release");
        let example: Option<String> = args.opt_value_from_str("--example")?;

        let project = Project::query(".").unwrap();
        let artifact = match example.as_ref() {
            Some(example) => Artifact::Example(example.as_str()),
            None => Artifact::Bin(project.name()),
        };
        let profile = if release { Profile::Release } else { Profile::Dev };

        let host = "x86_64-unknown-linux-gnu";  // FIXME: does this even matter?
        let bin = project.path(artifact, profile, Some(&chip.target(framework)), host)?;

        Ok(Some(
            CargoAppArgs {
                flash: args.contains("--flash"),
                flash_speed: args.opt_value_from_fn("--flash-speed", |s| s.parse::<u32>())?.unwrap_or(DEFAULT_FLASH_BAUD_RATE),
                release: args.contains("--release"),
                example: args.opt_value_from_str("--example")?,
                features: args.opt_value_from_str("--features")?,
                app_args: AppArgs {
                    chip,
                    framework,
                    reset: args.contains("--reset") || !args.contains("--no-reset"),
                    speed: args.opt_value_from_fn("--speed", |s| s.parse::<usize>())?,
                    bin: Some(bin.as_os_str().to_os_string()),
                    serial: args.free_from_str()?,
                }
            }
        ))
    }
}

fn print_usage() {
    let usage = "Usage: cargo espmonitor [OPTIONS] SERIAL_DEVICE\n\
        \n\
        \x20   --flash                         Flashes image to device (building first if necessary; requires 'cargo-espflash')\n\
        \x20   --flash-speed                   Baud rate when flashing (default 460800)\n\
        \x20   --example EXAMPLE               If flashing, flash this example app\n\
        \x20   --features FEATURES             If flashing, build with these features first\n\
        \x20   --target TARGET                 Infer chip and framework from target triple\n\
        \x20   --chip {esp32|esp32c3|esp8266}  Which ESP chip to target\n\
        \x20   --framework {baremetal,esp-idf} Which framework to target\n\
        \x20   --release                       Use the release build\n\
        \x20   --example EXAMPLE               Use the named example app binary\n\
        \x20   --reset                         Reset the chip on start (default)\n\
        \x20   --no-reset                      Do not reset thechip on start\n\
        \x20   --speed BAUD                    Baud rate of serial device (default: 115200)\n\
        \x20   --version                       Output version information and exit\n\
        \x20   SERIAL_DEVICE                   Path to the serial device";

    println!("{}", usage);
}

fn print_version() {
    println!("espmonitor {}", env!("CARGO_PKG_VERSION"));
}
