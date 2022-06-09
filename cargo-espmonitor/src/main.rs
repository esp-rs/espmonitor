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

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate serde;

use cargo_project::{Artifact, Profile, Project};
use clap::Parser;
use espmonitor::{run, AppArgs, Chip, Framework};
use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Parser)]
#[clap(name = "cargo")]
#[clap(bin_name = "cargo")]
enum Cargo {
    Espmonitor(CargoAppArgs),
}

#[derive(clap::Args)]
#[clap(author, version, about)]
struct CargoAppArgs {
    /// Reset the chip on start [default]
    #[clap(short, long)]
    reset: bool,

    /// Do not reset the chip on start
    #[clap(long, conflicts_with("reset"))]
    no_reset: bool,

    /// Baud rate of serial device
    #[clap(long, short, default_value = "115200", name = "BAUD")]
    speed: usize,

    /// Flashes image to device (building first if necessary; requires 'cargo-espflash')
    #[clap(long)]
    flash: bool,

    /// Baud rate when flashing
    #[clap(
        long,
        default_value_t = 460800,
        name = "FLASH_BAUD",
        requires = "flash"
    )]
    flash_speed: u32,

    /// If flashing, build with these features first
    #[clap(long, requires = "flash")]
    features: Option<String>,

    /// Which ESP chip to target
    #[clap(short, long, arg_enum, default_value_t = Chip::ESP32)]
    chip: Chip,

    /// Which framework to target
    #[clap(long, arg_enum, default_value_t = Framework::EspIdf)]
    framework: Framework,

    /// Use the release build
    #[clap(long)]
    release: bool,

    /// Example app to use
    #[clap(long, conflicts_with = "bin")]
    example: Option<String>,

    /// Bin target to use
    #[clap(long, conflicts_with = "example")]
    bin: Option<String>,

    /// Infer chip and framework from target triple
    #[clap(
        long,
        name = "TARGET_TRIPLE",
        conflicts_with("chip"),
        conflicts_with("framework")
    )]
    target: Option<String>,

    /// Path to the serial device
    #[clap(name = "SERIAL_DEVICE")]
    serial: String,
}

fn main() {
    let Cargo::Espmonitor(mut args) = Cargo::parse();

    let app_args = match handle_args(&mut args) {
        Err(err) => {
            eprintln!("Error: {}", err);
            eprintln!();
            std::process::exit(1);
        }
        Ok(app_args) => app_args,
    };

    if args.flash {
        if let Err(err) = run_flash(&mut args) {
            eprintln!("Error: {}", err);
            eprintln!();
            std::process::exit(1);
        };
    }

    if let Err(err) = run(app_args) {
        eprintln!("Error: {}", err);
        eprintln!();
        std::process::exit(1);
    }
}

fn run_flash(cargo_app_args: &mut CargoAppArgs) -> anyhow::Result<()> {
    let mut args = vec!["espflash".to_string()];
    if cargo_app_args.release {
        args.push("--release".to_string());
    }
    if let Some(example) = &cargo_app_args.example {
        args.push("--example".to_string());
        args.push(example.clone());
    }
    if let Some(bin) = &cargo_app_args.bin {
        args.push("--package".to_string());
        args.push(bin.clone());
    }
    if let Some(features) = cargo_app_args.features.take() {
        args.push("--features".to_string());
        args.push(features);
    }
    args.push("--speed".to_string());
    args.push(cargo_app_args.flash_speed.to_string());
    args.push(cargo_app_args.serial.clone());

    let status = Command::new("cargo").args(&args[..]).spawn()?.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, "Flash failed".to_string()).into())
    }
}

fn handle_args(args: &mut CargoAppArgs) -> anyhow::Result<AppArgs> {
    let (chip, framework) = match args.target {
        Some(ref target) => (Chip::from_target(target)?, Framework::from_target(target)?),
        None => (
            #[allow(clippy::redundant_closure)]
            args.chip,
            #[allow(clippy::redundant_closure)]
            args.framework,
        ),
    };

    let profile = if args.release {
        Profile::Release
    } else {
        Profile::Dev
    };
    let target = chip.target(framework);
    let artifact = match (&args.example, &args.bin) {
        (Some(example), _) => Some(Artifact::Example(example.as_str())),
        (_, Some(bin)) => Some(Artifact::Bin(bin.as_str())),
        _ => None,
    };
    let bin = find_artifact_path(&artifact, profile, &target, PathBuf::from("."))?;

    args.reset = !args.no_reset;

    Ok(AppArgs {
        reset: args.reset,
        no_reset: args.no_reset,
        speed: args.speed,
        bin: Some(bin),
        serial: args.serial.clone(),
    })
}

#[derive(Deserialize)]
struct CargoTomlWorkspace {
    members: Vec<String>,
}

#[derive(Deserialize)]
struct CargoToml {
    workspace: CargoTomlWorkspace,
}

fn find_artifact_path<P: AsRef<Path>>(
    artifact: &Option<Artifact>,
    profile: Profile,
    target: &String,
    project_root: P,
) -> anyhow::Result<OsString> {
    match Project::query(project_root.as_ref()) {
        Ok(project) => {
            let our_artifact = artifact.unwrap_or(Artifact::Bin(project.name()));
            let host = "x86_64-unknown-linux-gnu"; // FIXME: does this even matter?
            let bin = project
                .path(our_artifact, profile, Some(target), host)
                .map_err(|err| anyhow!("{}", err))?;
            Ok(bin.as_os_str().to_os_string())
        }
        Err(_) => {
            let mut cargo_toml_path = project_root.as_ref().to_path_buf();
            cargo_toml_path.push("Cargo.toml");
            let members: anyhow::Result<Vec<String>> = fs::read(cargo_toml_path)
                .map_err(|_| anyhow!("No Cargo.toml found at this location"))
                .and_then(|cargo_toml_bytes| {
                    toml::from_slice::<CargoToml>(&cargo_toml_bytes)
                        .map_err(|_| anyhow!("No valid cargo project found at this location"))
                })
                .map(|mut cargo_toml| std::mem::take(&mut cargo_toml.workspace.members));
            for member in members? {
                let mut member_path = project_root.as_ref().to_path_buf();
                member_path.push(member);
                if let Ok(path) = find_artifact_path(artifact, profile, target, &member_path) {
                    return Ok(path);
                }
            }
            Err(match artifact {
                Some(Artifact::Example(example)) => {
                    anyhow!("Could not find example '{}' in project", example)
                }
                Some(Artifact::Bin(bin)) => anyhow!("Could not find bin '{}' in project", bin),
                None => anyhow!("Couldn't find a binary; try passing --bin or --example"),
                _ => unreachable!(),
            })
        }
    }
}
