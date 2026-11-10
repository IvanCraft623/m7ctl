# m7ctl

Command-line and terminal UI to configure a **Machenike M7 Pro** mouse on Linux, Windows and macOS. Not affiliated with
Machenike; use at your own risk.

<p align="center"><img src="docs/images/status.png" alt="Status page of the terminal interface" width="720"></p>

<table>
  <tr>
    <td><img src="docs/images/dpi.png" alt="DPI stages with their scroll-light colours"></td>
    <td><img src="docs/images/colour-picker.png" alt="Colour picker"></td>
  </tr>
</table>

Tested on Linux with a wired mouse and the wireless receiver. Windows and macOS builds are compiled and unit-tested in CI
but not yet tried on real hardware; reports are welcome (`m7ctl list --verbose` and `m7ctl status` output helps).

## Install

```bash
cargo build --release
./target/release/m7ctl --help
```

On Linux, let your user open the device once, then replug the mouse (`dist/install.sh` can also do this and install completions):

```bash
sudo cp dist/udev/70-machenike-m7.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger --subsystem-match=hidraw
```

Settings are kept in the platform config directory (`~/.config/m7ctl`, `%APPDATA%\m7ctl`,
`~/Library/Application Support/m7ctl`); `M7CTL_CONFIG_DIR` overrides it.

## Usage

```bash
m7ctl status                                 # model, battery, charging
m7ctl dpi 800,1600,3200 --active 2           # up to 8 stages
m7ctl rate 1000                              # polling rate in Hz
m7ctl button forward middle                  # give a button a function (see `m7ctl button --help`)
m7ctl light steady 0080ff --brightness 3     # also: cycle, breathing, flash, off
m7ctl dpi-colors ff0000,00ff00,0000ff        # scroll-light colour per DPI stage
m7ctl macro 1 "tap:ctrl+c,wait:100,tap:ctrl+v"
m7ctl button dpi macro:1,2,1                 # run slot 1 once per press
m7ctl profile save work                      # profile load / list / show / import <file.psf>
m7ctl watch --notify                         # popup whenever the DPI stage changes
m7ctl tui                                    # interactive interface
```

The mouse stores its settings but cannot report them back, so m7ctl records what it last wrote; profiles are copies of
that record. A sleeping wireless mouse drops commands: m7ctl asks you to move it and retries for about 12 seconds.

The TUI is English by default (`L` switches to Spanish). `--no-default-features` builds a CLI-only binary.

See [docs/protocol.md](docs/protocol.md) for the protocol and [docs/architecture.md](docs/architecture.md) for the code layout.
