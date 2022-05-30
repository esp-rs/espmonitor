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

use clap::Parser;
use espmonitor::{run, AppArgs};

fn main() {
    #[cfg(windows)]
    let _ = crossterm::ansi_support::supports_ansi();
    // supports_ansi() returns what it suggests, and as a side effect enables ANSI support

    let mut args = AppArgs::parse();
    // TODO: This feels wrong...
    args.reset = !args.no_reset;
    match run(args) {
        Ok(_) => (),
        Err(err) => {
            eprintln!("Error: {}", err);
            std::process::exit(1);
        }
    };
}
