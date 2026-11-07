//! Macro events and their upload frames.

use super::buttons::key_usage;
use super::*;

/// Macro data bytes per upload frame, and the most frames the 4-bit chunk count allows.
const MACRO_CHUNK: usize = 25;

pub const MACRO_MAX_EVENTS: usize = MACRO_CHUNK * 15 / 3;

/// Default pause after every macro event, in the mouse's delay units (about 1 ms each).
const MACRO_STEP: u16 = 20;

fn macro_event(kind: u8, release: bool, delay: u16, code: u8) -> [u8; 3] {
    let delay = delay.min(0xfff);
    [(kind << 4) | ((release as u8) << 7) | (delay >> 8) as u8, delay as u8, code]
}

/// One key or modifier named in a macro: (event kind, code).
fn macro_key(name: &str) -> Option<(u8, u8)> {
    Some(match name.to_ascii_lowercase().as_str() {
        "ctrl" => (2, 0x01),
        "shift" => (2, 0x02),
        "alt" => (2, 0x04),
        "win" => (2, 0x08),
        k => (3, key_usage(k)?),
    })
}

/// Turn a macro description into events. Comma-separated steps:
/// `tap:<key or combo>` (ctrl+c, a, enter), `down:<key>`, `up:<key>`, `type:<text>` (letters, digits, space, . - / ; = '),
/// `click:<left|right|middle|back|forward>` (untested), `wait:<ms>` (extra pause after the previous step).
pub fn parse_macro(spec: &str) -> Result<Vec<[u8; 3]>, String> {
    let mut ev: Vec<[u8; 3]> = Vec::new();
    let push = |ev: &mut Vec<[u8; 3]>, kind, release, code| ev.push(macro_event(kind, release, MACRO_STEP, code));
    for step in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (verb, arg) = step.split_once(':').ok_or(format!("'{step}': expected verb:argument"))?;
        match verb {
            "tap" => {
                let keys: Vec<(u8, u8)> = arg.split('+').map(|k| macro_key(k).ok_or(format!("unknown key '{k}'"))).collect::<Result<_, _>>()?;
                for (kind, code) in &keys {
                    push(&mut ev, *kind, false, *code);
                }
                for (kind, code) in keys.iter().rev() {
                    push(&mut ev, *kind, true, *code);
                }
            }
            "down" | "up" => {
                let (kind, code) = macro_key(arg).ok_or(format!("unknown key '{arg}'"))?;
                push(&mut ev, kind, verb == "up", code);
            }
            "type" => {
                for ch in arg.chars() {
                    let upper = ch.is_ascii_uppercase();
                    let name = match ch {
                        ' ' => "space".to_string(),
                        '.' => "period".to_string(),
                        '-' => "minus".to_string(),
                        '/' => "slash".to_string(),
                        ';' => "semicolon".to_string(),
                        '=' => "equal".to_string(),
                        '\'' => "quote".to_string(),
                        c => c.to_ascii_lowercase().to_string(),
                    };
                    let (_, code) = macro_key(&name).ok_or(format!("cannot type '{ch}'"))?;
                    if upper {
                        push(&mut ev, 2, false, 0x02);
                    }
                    push(&mut ev, 3, false, code);
                    push(&mut ev, 3, true, code);
                    if upper {
                        push(&mut ev, 2, true, 0x02);
                    }
                }
            }
            "click" => {
                let mask = match arg {
                    "left" => 0x01,
                    "right" => 0x02,
                    "middle" => 0x04,
                    "back" => 0x08,
                    "forward" => 0x10,
                    _ => return Err(format!("unknown mouse button '{arg}'")),
                };
                push(&mut ev, 1, false, mask);
                push(&mut ev, 1, true, mask);
            }
            "wait" => {
                let ms: u16 = arg.parse().map_err(|_| format!("'{arg}' is not a number of milliseconds"))?;
                let last = ev.last_mut().ok_or("wait needs a step before it")?;
                let delay = ((((last[0] & 0x0f) as u16) << 8) | last[1] as u16).saturating_add(ms).min(0xfff);
                last[0] = (last[0] & 0xf0) | (delay >> 8) as u8;
                last[1] = delay as u8;
            }
            other => return Err(format!("unknown macro step '{other}'")),
        }
    }
    if ev.is_empty() {
        return Err("the macro is empty".into());
    }
    if ev.len() > MACRO_MAX_EVENTS {
        return Err(format!("a macro holds at most {MACRO_MAX_EVENTS} events (this one has {})", ev.len()));
    }
    Ok(ev)
}

/// A macro that presses no real key (key usage 0): used to stop a running or stale macro.
pub fn macro_clear() -> Vec<[u8; 3]> {
    vec![macro_event(3, false, MACRO_STEP, 0), macro_event(3, true, MACRO_STEP, 0)]
}

/// The frames that upload one macro (register 0x0e): the events (3 bytes each) are cut into 25-byte chunks.
/// Frame layout: byte 5 = chunk index << 4 | chunk count, byte 6 = chunk length + 1,
/// byte 7 = macro slot (1-based), then the chunk (padded with zeros).
pub fn macro_frames(seqs: &mut impl FnMut() -> u8, slot: u8, events: &[[u8; 3]]) -> Result<Vec<Frame>, String> {
    if !(1..=15).contains(&slot) {
        return Err("macro slot must be 1-15".into());
    }
    if events.is_empty() || events.len() > MACRO_MAX_EVENTS {
        return Err(format!("a macro needs 1-{MACRO_MAX_EVENTS} events"));
    }
    let data: Vec<u8> = events.iter().flatten().copied().collect();
    let chunks: Vec<&[u8]> = data.chunks(MACRO_CHUNK).collect();
    let n = chunks.len() as u8;
    Ok(chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let mut f = [0u8; REPORT_LEN];
            f[0] = REPORT_ID;
            f[1] = 0x01;
            f[2] = REG_MACRO;
            f[4] = seqs();
            f[5] = ((i as u8) << 4) | n;
            f[6] = chunk.len() as u8 + 1;
            f[7] = slot;
            f[8..8 + chunk.len()].copy_from_slice(chunk);
            f
        })
        .collect())
}
