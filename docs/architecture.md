# Architecture

```
src/
  main.rs       parse arguments, build the real Context, run
  protocol/     bytes only, no I/O: frames, status, DPI, lights, buttons, macros, input reports
  device/       Transport trait; hid.rs (hidapi); fake.rs (tests); Mouse = protocol rules
                (sequence numbers, pacing, acknowledgement, retrying while a wireless mouse sleeps)
  settings/     Store (config directory), the record of what was written, profiles, .psf import
  cli/          clap definitions and one file per group of commands
  notify.rs     desktop popups
  tui/          terminal interface (cargo feature `tui`): model, pages, widgets, actions, input, view, i18n
tests/cli.rs   command-line tests against a fake mouse and a temporary config directory
dist/           udev rule (Linux), install script
```

- **The mouse cannot be read back**, so every command records what it wrote in `Store`. Profiles are copies of that
  record; loading one replays it through the same command parser.
- **`Context`** carries what a command needs from outside (store, how to open the mouse, where messages go). Tests give it
  a fake mouse and a temporary store.
- **The TUI runs commands in-process** through the `Runner` trait. Tests run background work inline.
- **English is the default language.** `tui/i18n` translates drawn text; logic never depends on translated text.
