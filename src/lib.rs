//! m7ctl: configure a Machenike M7 Pro mouse (Linux, Windows, macOS).
//!
//! - [`protocol`]: the wire format (pure functions, no I/O)
//! - [`device`]: talking to the mouse over HID (and a fake one for tests)
//! - [`settings`]: the record of what was written, and profiles
//! - [`cli`]: the command line
//! - `tui`: the terminal interface (cargo feature `tui`)

pub mod cli;
pub mod device;
pub mod notify;
pub mod protocol;
pub mod settings;
#[cfg(feature = "tui")]
pub mod tui;
