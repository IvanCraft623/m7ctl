//! Import of the vendor `.psf` profile files (UTF-16 text, `Key=Value` lines) into a settings record.
//!
//! Imports DPI stages and colours, polling rate, lift-off distance, buttons, power settings and the main lights;
//! anything else is reported in the returned notes.

use super::store::State;
use crate::protocol::DpiTable;
use std::collections::HashMap;

/// The record to store plus things the user should know about.
pub struct Imported {
    pub name: String,
    pub state: State,
    pub notes: Vec<String>,
}

/// Decode UTF-16 (little endian unless a big-endian byte order mark says otherwise).
fn decode(bytes: &[u8]) -> Result<String, String> {
    let (body, big) = match bytes {
        [0xff, 0xfe, rest @ ..] => (rest, false),
        [0xfe, 0xff, rest @ ..] => (rest, true),
        _ => (bytes, false),
    };
    if body.len() % 2 != 0 {
        return Err("not a UTF-16 text file".into());
    }
    let units: Vec<u16> = body.chunks(2).map(|c| if big { u16::from_be_bytes([c[0], c[1]]) } else { u16::from_le_bytes([c[0], c[1]]) }).collect();
    String::from_utf16(&units).map_err(|_| "not a valid UTF-16 text file".to_string())
}

fn fields(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|l| {
            let (k, v) = l.split_once('=')?;
            Some((k.trim().to_string(), v.trim().to_string()))
        })
        .collect()
}

fn hex_byte(s: &str) -> Option<u8> {
    u8::from_str_radix(s.get(..2)?, 16).ok()
}

fn colour(s: &str) -> Option<String> {
    let c = s.trim_start_matches('#');
    (c.len() == 6 && c.chars().all(|ch| ch.is_ascii_hexdigit())).then(|| c.to_ascii_lowercase())
}

/// `a|b|c;d|e|f` -> groups of items.
fn groups(s: &str) -> Vec<Vec<&str>> {
    s.split(';').filter(|g| !g.is_empty()).map(|g| g.split('|').collect()).collect()
}

pub fn import(bytes: &[u8], fallback_name: &str, table: &DpiTable) -> Result<Imported, String> {
    let f = fields(&decode(bytes)?);
    if !f.contains_key("DPIOffOnFlagValue") && !f.contains_key("PollingRateValue") {
        return Err("this does not look like a vendor .psf profile".into());
    }
    let mut state: State = vec![];
    let mut notes = vec![];
    let name = f
        .get("ProfileName")
        .map(|n| n.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect::<String>())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| fallback_name.to_string());

    // DPI: enabled stages in order; a stage value is an entry number of the sensor's DPI table
    if let Some(mask) = f.get("DPIOffOnFlagValue").and_then(|v| hex_byte(v)) {
        let current = f.get("CurrentDPILevelValue").and_then(|v| hex_byte(v)).unwrap_or(0) as usize;
        let (mut stages, mut colours, mut active) = (vec![], vec![], 1);
        for i in 0..8 {
            if mask & (1 << i) == 0 {
                continue;
            }
            let Some(index) = f.get(&format!("XYSetting{}Value", i + 1)).and_then(|v| u16::from_str_radix(v, 16).ok()) else { continue };
            let Some(dpi) = table.dpi_for_index(index) else {
                notes.push(format!("stage {} (table entry {index}) does not exist on this sensor and was skipped", i + 1));
                continue;
            };
            if i == current {
                active = stages.len() + 1;
            }
            stages.push(dpi.to_string());
            colours.push(f.get(&format!("DPILevelIndication{i}Valuer")).and_then(|c| colour(c)).unwrap_or_else(|| "ffffff".into()));
        }
        if !stages.is_empty() {
            state.push(("dpi".into(), vec![stages.join(","), "--active".into(), active.to_string()]));
            state.push(("dpi-colors".into(), vec![colours.join(",")]));
        }
    }

    if let Some(code) = f.get("PollingRateValue").and_then(|v| hex_byte(v)) {
        match [125, 250, 500, 1000].get(code as usize) {
            Some(hz) => state.push(("rate".into(), vec![hz.to_string()])),
            None => notes.push(format!("unknown polling rate code {code}")),
        }
    }

    if let Some(v) = f.get("LiftDetectionHeight").or_else(|| f.get("LiftDetectionValue")) {
        state.push(("lod".into(), vec![if v.trim().trim_start_matches('0') == "1" { "1" } else { "2" }.into()]));
    }

    if let Some(minutes) = f.get("PowerDownTime").and_then(|v| v.parse::<u8>().ok()) {
        let moving = f.get("MoveLightingOnOffMode").map(|v| v.trim() == "1").unwrap_or(false);
        let mut args = vec![minutes.to_string()];
        if moving {
            args.push("lights-off-moving".into());
        }
        if !(3..=10).contains(&minutes) {
            args.push("--force".into());
        }
        state.push(("power".into(), args));
    }
    if let Some(v) = f.get("MotionWakeup") {
        state.push(("motion-wake".into(), vec![if v.trim() == "0" { "off" } else { "on" }.into()]));
    }

    // buttons: the vendor order is left, right, middle, forward, back, dpi
    for (i, button) in ["left", "right", "middle", "forward", "back", "dpi"].iter().enumerate() {
        let Some(code) = f.get(&format!("Key{i}Value")).filter(|c| c.len() == 6 && c.chars().all(|ch| ch.is_ascii_hexdigit())) else { continue };
        if *button == "left" {
            continue; // the left click is never re-assigned by an import
        }
        if code.starts_with("90") {
            notes.push(format!("the {button} button runs a macro; macros are not imported"));
            continue;
        }
        state.push((format!("button:{button}"), vec![button.to_string(), format!("raw:{}", code.to_ascii_lowercase())]));
    }

    lights(&f, &mut state, &mut notes);
    Ok(Imported { name, state, notes })
}

fn lights(f: &HashMap<String, String>, state: &mut State, notes: &mut Vec<String>) {
    let Some(mode) = f.get("Lighting1ModeValue").and_then(|v| hex_byte(v)) else { return };
    let m = mode as usize;
    if f.get("Lighting1OffOn").map(|v| v.trim() == "0").unwrap_or(false) {
        state.push(("light".into(), vec!["off".into()]));
        return;
    }
    let pick = |key: &str| f.get(key).and_then(|v| v.split('|').nth(m)).and_then(|x| x.trim().parse::<u8>().ok()).unwrap_or(1).min(3);
    let (speed, brightness) = (pick("Lighting1Speed"), pick("Lighting1Brightness"));
    let colours: Vec<String> =
        f.get("Lighting1FixedColor").map(|v| groups(v)).and_then(|g| g.get(m).cloned()).unwrap_or_default().iter().filter_map(|c| colour(c)).collect();
    let selected: Vec<bool> =
        f.get("Lighting1ColorSelected").map(|v| groups(v)).and_then(|g| g.get(m).cloned()).unwrap_or_default().iter().map(|x| x.trim() == "1").collect();
    let chosen: Vec<String> = colours.iter().enumerate().filter(|(i, _)| selected.get(*i).copied().unwrap_or(false)).map(|(_, c)| c.clone()).collect();
    let first = colours.first().cloned();
    let b = brightness.to_string();
    let rec = match (mode, first) {
        (0, _) => Some(vec!["off".to_string()]),
        (3, Some(c)) => Some(vec!["steady".into(), c, "--brightness".into(), b]),
        (6, Some(c)) => Some(vec!["breathing".into(), c, "--brightness".into(), b]),
        (7, Some(c)) => Some(vec!["flash".into(), c, "--brightness".into(), b]),
        (2, _) if !chosen.is_empty() => {
            Some(vec!["cycle".into(), chosen[..chosen.len().min(7)].join(","), "--brightness".into(), b, "--speed".into(), speed.to_string()])
        }
        _ => None,
    };
    match rec {
        Some(args) => state.push(("light".into(), args)),
        None => notes.push(format!("lighting effect {mode} is not supported by m7ctl and was skipped")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(s: &str) -> Vec<u8> {
        let mut v = vec![0xff, 0xfe];
        v.extend(s.encode_utf16().flat_map(|u| u.to_le_bytes()));
        v
    }

    const SAMPLE: &str = "[Setting]\r\nProfileName=Pro file 1\r\nDPIOffOnFlagValue=3F\r\n\tXYSetting1Value=0F\r\n\tXYSetting2Value=1F\r\n\tXYSetting3Value=2F\r\n\tXYSetting4Value=3F\r\n\tXYSetting5Value=4F\r\n\tXYSetting6Value=5F\r\n\tXYSetting7Value=32\r\n\tXYSetting8Value=32\r\n\tCurrentDPILevelValue=02\r\n\tDPILevelIndication0Valuer=FF0000\r\n\tDPILevelIndication1Valuer=00FF00\r\n\tDPILevelIndication2Valuer=0000FF\r\n\tPollingRateValue=010000\r\n\tLiftDetectionHeight=1\r\nKey1Value=100200\r\nKey3Value=101000\r\nKey4Value=700106\r\nKey5Value=900101\r\nLighting1ModeValue=030000\r\nLighting1OffOn=01\r\nLighting1Speed=01|01|01|01|01|01|01|01|01|01\r\nLighting1Brightness=01|01|01|03|01|01|01|01|01|01\r\nLighting1ColorSelected=0|0|0|1|0\r\nLighting1FixedColor=000000;000000;000000;BE00FF|000000;000000\r\n";

    #[test]
    fn imports_dpi_rate_lod_and_buttons() {
        let i = import(&utf16(SAMPLE), "fallback", &DpiTable::standard()).unwrap();
        assert_eq!(i.name, "Profile1");
        let get = |k: &str| i.state.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
        assert_eq!(get("dpi").unwrap(), vec!["800,1600,2400,3200,4000,4800", "--active", "3"]);
        assert_eq!(get("dpi-colors").unwrap()[0].split(',').next(), Some("ff0000"));
        assert_eq!(get("rate").unwrap(), vec!["250"]);
        assert_eq!(get("lod").unwrap(), vec!["1"]);
        assert_eq!(get("button:right").unwrap(), vec!["right", "raw:100200"]);
        assert_eq!(get("button:back").unwrap(), vec!["back", "raw:700106"], "vendor Key4 is the back button");
        assert!(get("button:left").is_none());
        assert!(i.notes.iter().any(|n| n.contains("dpi button runs a macro")));
        assert_eq!(get("light").unwrap(), vec!["steady", "be00ff", "--brightness", "3"]);
    }

    #[test]
    fn rejects_other_files() {
        assert!(import(&utf16("hello=world"), "x", &DpiTable::standard()).is_err());
        assert!(import(&[1, 2, 3], "x", &DpiTable::standard()).is_err());
    }
}
