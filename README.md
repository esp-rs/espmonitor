# ESPMonitor

ESP32 and ESP8266 serial monitor.

## Features

* Resets chip on startup.
* Can match hex sequences in output to function names in a binary.
* Optionally builds and flashes before starting the monitor.
* `cargo` integration.

## Usage

Install with:

```
cargo install cargo-espmonitor
```

Run `cargo espmonitor --help` for details.

If you prefer the standalone monitor app without `cargo` integration,
you can instead install `espmonitor`.

Note that this program will not build or flash your project for you.
