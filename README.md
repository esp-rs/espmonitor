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

### Keyboard Commands

While monitoring, ESPMonitor accepts the following keyboard commands:

* CTRL+R: Reset chip
* CTRL+C: Quit

## Releasing

See [RELEASING](RELEASING.md) for instructions.
