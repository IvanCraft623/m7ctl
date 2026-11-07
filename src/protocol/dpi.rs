//! DPI stages, their indicator colours and the polling rate.

use super::*;

/// The sensor (PixArt 3395 on this model variant) steps in 50 DPI and tops out at 26000.
pub const DPI_STEP: u32 = 50;

pub const MAX_DPI: u32 = 26000;

/// The mouse has up to eight DPI stages.
pub const MAX_STAGES: usize = 8;

/// DPI stages (register 0x05). `active` is the zero-based stage the mouse starts on.
/// Each stage is sent as the table's code for that DPI (`DPI / 50 - 1` on the M7 Pro) in little endian; the mask
/// enables the first `n` stages.
pub fn dpi_stages(seq: u8, stages: &[u32], active: usize, table: &DpiTable) -> Result<Frame, String> {
    let n = stages.len();
    if n == 0 || n > MAX_STAGES {
        return Err(format!("give between 1 and {MAX_STAGES} DPI values"));
    }
    if active >= n {
        return Err(format!("active stage must be between 1 and {n}"));
    }
    let codes: Vec<u16> = stages.iter().map(|&d| table.code_for(d)).collect::<Result<_, _>>()?;
    let mut p = [0u8; 18];
    p[0] = (((active + 1) as u8) << 4) | n as u8;
    p[1] = ((1u16 << n) - 1) as u8;
    for i in 0..MAX_STAGES {
        let code = codes[i.min(n - 1)];
        p[2 + 2 * i..4 + 2 * i].copy_from_slice(&code.to_le_bytes());
    }
    Ok(frame(REG_DPI, seq, &p))
}

/// Polling rate (register 0x07). The wire value is the report interval in milliseconds.
pub fn polling_rate(seq: u8, hz: u32) -> Result<Frame, String> {
    let interval = match hz {
        1000 => 1,
        500 => 2,
        250 => 4,
        125 => 8,
        _ => return Err("polling rate must be 125, 250, 500 or 1000".into()),
    };
    Ok(frame(REG_POLLING, seq, &[interval]))
}

/// Colours of the scroll-wheel lights that show the active DPI stage (register 0x06): 8 colours,
/// the list is padded with its last colour.
pub fn dpi_colors(seq: u8, colors: &[Rgb]) -> Result<Frame, String> {
    if colors.is_empty() || colors.len() > 8 {
        return Err("give between 1 and 8 colours".into());
    }
    let mut p = [0u8; 24];
    for i in 0..8 {
        p[3 * i..3 * i + 3].copy_from_slice(&colors[i.min(colors.len() - 1)]);
    }
    Ok(frame(REG_DPI_COLORS, seq, &p))
}
