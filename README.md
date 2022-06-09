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

## Contributing

### Hooks

Before you start writing code, run this in the root of the repo:

```
mkdir -p .git/hooks && (cd .git/hooks && ln -s ../../hooks/* .)
```

This will set up a pre-commit hook that will run `cargo clippy` and
`cargo fmt` before each commit, to save you some time getting frustrated
with failed PR checks.

### Releasing

See [RELEASING](RELEASING.md) for instructions.
