//! Input reports the mouse sends by itself.

use super::DpiTable;

/// A DPI-stage change announced by the mouse: input report `05 06 01 02 <stage> <DPI code LE16> ...`.
/// Returns (stage 1-based, dpi).
pub fn parse_dpi_event(r: &[u8], table: &DpiTable) -> Option<(u8, u32)> {
    if r.len() < 7 || r[..4] != [0x05, 0x06, 0x01, 0x02] || r[4] == 0 {
        return None;
    }
    let code = u16::from_le_bytes([r[5], r[6]]);
    Some((r[4], table.dpi_for_code(code)?))
}
