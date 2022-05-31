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
use clap::{ArgGroup, Parser};
use espmonitor::{run, AppArgs, Chip, Framework};
use std::{error::Error, io, process::Command};

#[derive(Parser)]
#[clap(name = "cargo")]
#[clap(bin_name = "cargo")]
enum Cargo {
    Espmonitor(CargoAppArgs),
}

#[derive(clap::Args)]
#[clap(author, version, about)]
// None of the arguments related to flashing can appear without "--flash", but they aren't required
// (e.g. if "--flash" isn't specified) and multiple of them can appear
#[clap(group(
        ArgGroup::new("will_flash")
            .required(false)
            .multiple(true)
            .args(&["release", "example", "features"])
            .requires("flash")))]
struct CargoAppArgs {
    /// Flashes image to device (building first if necessary; requires 'cargo-espflash')
    #[clap(long)]
    flash: bool,

    /// Baud rate when flashing
    #[clap(long, default_value_t = 460800, name = "FLASH_BAUD", requires("flash"))]
    flash_speed: u32,

    /// Which ESP chip to target
    #[clap(short, long, arg_enum, default_value_t = Chip::ESP32)]
    chip: Chip,

    /// Which framework to target
    #[clap(long, arg_enum, default_value_t = Framework::Baremetal, requires("chip"))]
    framework: Framework,

    /// Use the release build
    #[clap(long)]
    release: bool,

    /// If flashing, flash this example app
    #[clap(long)]
    example: Option<String>,

    /// If flashing, build with these features first
    #[clap(long)]
    features: Option<String>,

    /// Infer chip and framework from target triple
    #[clap(long, name = "TARGET_TRIPLE", conflicts_with("chip"))]
    target: Option<String>,

    #[clap(flatten)]
    app_args: AppArgs,
}

fn main() {
    let Cargo::Espmonitor(mut args) = Cargo::parse();

    if let Err(err) = handle_args(&mut args) {
        eprintln!("Error: {}", err);
        eprintln!();
        std::process::exit(1);
    };

    if args.flash {
        if let Err(err) = run_flash(&mut args) {
            eprintln!("Error: {}", err);
            eprintln!();
            std::process::exit(1);
        };
    }
    if let Err(err) = run(args.app_args) {
        eprintln!("Error: {}", err);
        eprintln!();
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

    let status = Command::new("cargo").args(&args[..]).spawn()?.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, "Flash failed".to_string()).into())
    }
}

fn handle_args(args: &mut CargoAppArgs) -> Result<(), Box<dyn Error>> {
    let (chip, framework) = match args.target {
        Some(ref target) => (Chip::from_target(target)?, Framework::from_target(target)?),
        None => (
            #[allow(clippy::redundant_closure)]
            args.chip,
            #[allow(clippy::redundant_closure)]
            args.framework,
        ),
    };

    let project = Project::query(".").unwrap();
    let artifact = match args.example.as_ref() {
        Some(example) => Artifact::Example(example.as_str()),
        None => Artifact::Bin(project.name()),
    };
    let profile = if args.release {
        Profile::Release
    } else {
        Profile::Dev
    };

    let host = "x86_64-unknown-linux-gnu"; // FIXME: does this even matter?
    let bin = project.path(artifact, profile, Some(&chip.target(framework)), host)?;

    args.app_args.bin = Some(bin.as_os_str().to_os_string());

    args.app_args.reset = !args.app_args.no_reset;

    Ok(())
}
