//! Main lights.

use super::*;

/// Low nibble is speed+1, high nibble is brightness+1.
fn light_nibbles(brightness: u8, speed: u8) -> u8 {
    ((brightness + 1) << 4) | (speed + 1)
}

fn check_speed(s: u8) -> Result<(), String> {
    if s > 3 {
        Err("speed must be between 0 and 3".into())
    } else {
        Ok(())
    }
}

fn check_brightness(b: u8) -> Result<(), String> {
    if b > 3 {
        Err("brightness must be between 0 and 3".into())
    } else {
        Ok(())
    }
}

/// Main lights off (mode 0).
pub fn light_off(seq: u8) -> Frame {
    frame(REG_LIGHT, seq, &[0, light_nibbles(3, 1)])
}

/// One steady colour (mode 3). Colours are plain R, G, B.
pub fn light_steady(seq: u8, color: Rgb, brightness: u8) -> Result<Frame, String> {
    check_brightness(brightness)?;
    Ok(frame(REG_LIGHT, seq, &[3, light_nibbles(brightness, 1), color[0], color[1], color[2]]))
}

/// One colour that flashes on and off quickly (mode 7).
pub fn light_flash(seq: u8, color: Rgb, brightness: u8) -> Result<Frame, String> {
    check_brightness(brightness)?;
    Ok(frame(REG_LIGHT, seq, &[7, light_nibbles(brightness, 1), color[0], color[1], color[2]]))
}

/// One colour that fades in and out slowly (mode 6); only the first colour is used.
pub fn light_breathing(seq: u8, color: Rgb, brightness: u8) -> Result<Frame, String> {
    check_brightness(brightness)?;
    Ok(frame(REG_LIGHT, seq, &[6, light_nibbles(brightness, 1), color[0], color[1], color[2]]))
}

/// Colour cycle through up to seven colours (mode 2).
pub fn light_cycle(seq: u8, colors: &[Rgb], brightness: u8, speed: u8) -> Result<Frame, String> {
    check_brightness(brightness)?;
    check_speed(speed)?;
    if colors.is_empty() || colors.len() > 7 {
        return Err("give between 1 and 7 colours".into());
    }
    let mut body = vec![2, light_nibbles(brightness, speed), colors.len() as u8];
    for c in colors {
        body.extend_from_slice(c);
    }
    Ok(frame(REG_LIGHT, seq, &body))
}
