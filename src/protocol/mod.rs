//! Wire protocol of the Machenike M7 Pro configuration channel.
//!
//! Settings travel as 33-byte HID feature reports with report ID 0x0c:
//!
//! ```text
//! [0] 0x0c   report id        [4] seq    rolling counter
//! [1] 0x01   command class    [5] 0x01   (0x00 for the status query)
//! [2] reg    register         [6] len    payload length
//! [3] 0x00                    [7..] payload
//! ```

pub const REPORT_ID: u8 = 0x0c;
pub const REPORT_LEN: usize = 33;
pub type Frame = [u8; REPORT_LEN];
pub type Rgb = [u8; 3];

mod buttons;
mod dpi;
mod dpi_table;
mod events;
mod lights;
mod macros;
mod status;
#[cfg(test)]
mod tests;

pub use buttons::*;
pub use dpi::*;
pub use dpi_table::{DpiTable, DEFAULT_VARIANT};
pub use events::parse_dpi_event;
pub use lights::*;
pub use macros::*;
pub use status::*;

pub const REG_BUTTON: u8 = 0x04;
pub const REG_MACRO: u8 = 0x0e;
pub const REG_DPI: u8 = 0x05;
pub const REG_DPI_COLORS: u8 = 0x06;
pub const REG_POLLING: u8 = 0x07;
pub const REG_LIGHT: u8 = 0x08;
pub const REG_STATUS: u8 = 0x20;

fn frame(reg: u8, seq: u8, payload: &[u8]) -> Frame {
    let mut f = [0u8; REPORT_LEN];
    f[0] = REPORT_ID;
    f[1] = 0x01;
    f[2] = reg;
    f[4] = seq;
    f[5] = 0x01;
    f[6] = payload.len() as u8;
    f[7..7 + payload.len()].copy_from_slice(payload);
    f
}

/// Raw frame for experiments: any register and payload (at most 26 bytes).
pub fn raw(seq: u8, reg: u8, payload: &[u8]) -> Result<Frame, String> {
    if payload.len() > REPORT_LEN - 7 {
        return Err(format!("payload is limited to {} bytes", REPORT_LEN - 7));
    }
    Ok(frame(reg, seq, payload))
}

pub fn parse_rgb(s: &str) -> Result<Rgb, String> {
    let h = s.trim_start_matches('#');
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("'{s}' is not a colour like ff8000"));
    }
    let v = u32::from_str_radix(h, 16).map_err(|e| e.to_string())?;
    Ok([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

pub fn parse_hex_bytes(s: &str) -> Result<Vec<u8>, String> {
    let h: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if !h.len().is_multiple_of(2) || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("'{s}' is not an even-length hex string"));
    }
    (0..h.len()).step_by(2).map(|i| u8::from_str_radix(&h[i..i + 2], 16).map_err(|e| e.to_string())).collect()
}
