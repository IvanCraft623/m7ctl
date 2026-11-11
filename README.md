# m7ctl

[![CI](https://github.com/IvanCraft623/m7ctl/actions/workflows/ci.yml/badge.svg)](https://github.com/IvanCraft623/m7ctl/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/IvanCraft623/m7ctl)](https://github.com/IvanCraft623/m7ctl/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

Configure a **Machenike M7 Pro** mouse from the terminal on Linux, Windows and macOS: DPI, polling rate, buttons,
lighting, macros and profiles. Not affiliated with Machenike.

<p align="center"><img src="docs/images/status.png" alt="Status page of the terminal interface" width="720"></p>

<table>
  <tr>
    <td><img src="docs/images/dpi.png" alt="DPI stages with their scroll-light colours"></td>
    <td><img src="docs/images/colour-picker.png" alt="Colour picker"></td>
  </tr>
</table>

## Quick start

1. [Download](#download) m7ctl and unpack it.
2. **Linux only:** run `./install.sh` once, then replug the mouse.
3. Run **`m7ctl tui`** (`m7ctl.exe tui` on Windows). Press `?` for the keys; the mouse works too.

## Download

| System | Latest |
|---|---|
| Linux | [x86_64](https://github.com/IvanCraft623/m7ctl/releases/latest/download/m7ctl-linux-x86_64.tar.gz), [ARM64](https://github.com/IvanCraft623/m7ctl/releases/latest/download/m7ctl-linux-aarch64.tar.gz) |
| Windows | [x86_64](https://github.com/IvanCraft623/m7ctl/releases/latest/download/m7ctl-windows-x86_64.zip), [ARM64](https://github.com/IvanCraft623/m7ctl/releases/latest/download/m7ctl-windows-aarch64.zip) |
| macOS | [Intel and Apple Silicon](https://github.com/IvanCraft623/m7ctl/releases/latest/download/m7ctl-macos-universal.tar.gz) |

The binaries are not code-signed, so the system warns the first time:

- **Windows:** SmartScreen shows "Windows protected your PC": click *More info*, then *Run anyway*.
- **macOS:** run `xattr -d com.apple.quarantine m7ctl` once after unpacking, or right-click the file and choose *Open*.

Only Linux has been tested on a real mouse; reports from Windows and macOS are welcome (`m7ctl list --verbose`).

## Command line

```bash
m7ctl status                              # model, battery, charging
m7ctl dpi 800,1600,3200 --active 2        # up to 8 stages
m7ctl rate 1000                           # polling rate in Hz
m7ctl button forward middle               # see `m7ctl button --help`
m7ctl light steady 0080ff --brightness 3  # also: cycle, breathing, flash, off
m7ctl macro 1 "tap:ctrl+c,wait:100,tap:ctrl+v"
m7ctl profile save work                   # also: load, list, import <file.psf>
m7ctl watch --notify                      # popup when the DPI stage changes
```

The mouse cannot report its settings back, so m7ctl remembers what it last wrote; profiles are copies of that record.
`M7CTL_CONFIG_DIR` changes where it is kept.

## Troubleshooting

- **"permission denied" or no device (Linux):** run `./install.sh` (it installs the udev rule) and replug the mouse.
- **"the mouse is asleep":** move it; m7ctl retries for about 12 seconds.
- **The interface looks broken:** use a terminal of at least 80x24.

## Build from source

```bash
cargo build --release          # needs a Rust toolchain
./target/release/m7ctl tui
```

On Linux, `./dist/install.sh` also sets up access to the mouse. `--no-default-features` builds a CLI-only binary.

See [docs/protocol.md](docs/protocol.md) and [docs/architecture.md](docs/architecture.md).
