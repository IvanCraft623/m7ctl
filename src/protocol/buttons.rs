//! Buttons and the functions they can be given.

use super::*;

/// Physical buttons, in the order the mouse numbers them on the wire (note: back before forward).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Button {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Dpi,
}

impl Button {
    pub fn parse(s: &str) -> Option<Button> {
        Some(match s {
            "left" => Button::Left,
            "right" => Button::Right,
            "middle" => Button::Middle,
            "back" => Button::Back,
            "forward" => Button::Forward,
            "dpi" => Button::Dpi,
            _ => return None,
        })
    }
    pub fn index(self) -> u8 {
        self as u8
    }
}

/// What a button does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Function {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    DpiLoop,
    DpiPlus,
    DpiMinus,
    /// A code taken verbatim: media key (type 0x80) or keyboard shortcut (type 0x70).
    Raw([u8; 3]),
}

impl Function {
    pub fn parse(s: &str) -> Option<Function> {
        Some(match s {
            "left" => Function::Left,
            "right" => Function::Right,
            "middle" => Function::Middle,
            "back" => Function::Back,
            "forward" => Function::Forward,
            "dpi-loop" => Function::DpiLoop,
            "dpi-plus" => Function::DpiPlus,
            "dpi-minus" => Function::DpiMinus,
            "disable" => Function::Raw([0x00, 0x00, 0x00]),
            "three-click" => Function::Raw([0x30, 10, 3]),
            "tilt-left" => Function::Raw([0x10, 0x40, 0x00]),
            "tilt-right" => Function::Raw([0x10, 0x80, 0x00]),
            "lights-toggle" => Function::Raw([0x60, 0x01, 0x00]),
            "switch-polling" => Function::Raw([0x60, 0x02, 0x00]),
            "switch-effect" => Function::Raw([0x60, 0x04, 0x00]),
            _ => {
                if let Some(args) = s.strip_prefix("macro:") {
                    // macro:<slot 1-15>[,<repeat mode 0-2>[,<times>]]
                    let mut it = args.split(',');
                    let slot: u8 = it.next()?.parse().ok().filter(|v| (1..=15).contains(v))?;
                    let mode: u8 = match it.next() {
                        Some(v) => v.parse().ok().filter(|m| *m <= 2)?,
                        None => 0,
                    };
                    let times: u8 = match it.next() {
                        Some(v) => v.parse().ok()?,
                        None => 0,
                    };
                    return Some(Function::Raw([0x90, slot | (mode << 4), times]));
                }
                if let Some(n) = s.strip_prefix("dpi-lock:") {
                    // DPI Lock: while the button is held the DPI is forced to preset N (1-11)
                    return n.parse::<u8>().ok().filter(|v| (1..=11).contains(v)).map(|v| Function::Raw([0x50, v, 0x00]));
                }
                if let Some(code) = s.strip_prefix("raw:") {
                    // raw:<6 hex digits>: a button code taken verbatim (used by the .psf import)
                    let bytes = parse_hex_bytes(code).ok().filter(|b| b.len() == 3)?;
                    return Some(Function::Raw([bytes[0], bytes[1], bytes[2]]));
                }
                if let Some(args) = s.strip_prefix("fire:") {
                    return parse_fire(args).map(Function::Raw);
                }
                if let Some(combo) = shortcut_alias(s) {
                    return parse_key_combo(combo).map(Function::Raw);
                }
                if let Some(combo) = s.strip_prefix("key:") {
                    return parse_key_combo(combo).map(Function::Raw);
                }
                return media_usage(s).map(|u| Function::Raw([0x80, u as u8, (u >> 8) as u8]));
            }
        })
    }
    /// The three value bytes: a type byte, then a mouse-button mask or a function number.
    pub fn code(self) -> [u8; 3] {
        match self {
            Function::Left => [0x10, 0x01, 0],
            Function::Right => [0x10, 0x02, 0],
            Function::Middle => [0x10, 0x04, 0],
            Function::Back => [0x10, 0x08, 0],
            Function::Forward => [0x10, 0x10, 0],
            Function::DpiLoop => [0x40, 0x01, 0],
            Function::DpiPlus => [0x40, 0x02, 0],
            Function::DpiMinus => [0x40, 0x03, 0],
            Function::Raw(c) => c,
        }
    }
}

/// `fire:<interval>[,<loop>]`: Fire Key, repeated left clicks. Interval 5-255 (the app's unit), loop 0-255
/// (0 = keep firing while the button is held).
fn parse_fire(args: &str) -> Option<[u8; 3]> {
    let mut it = args.split(',');
    let interval: u8 = it.next()?.parse().ok().filter(|v| *v >= 5)?;
    let repeat: u8 = match it.next() {
        Some(v) => v.parse().ok()?,
        None => 0,
    };
    if it.next().is_some() {
        return None;
    }
    Some([0x30, interval, repeat])
}

/// Office and window-management functions, as the shortcuts they send.
fn shortcut_alias(name: &str) -> Option<&'static str> {
    Some(match name {
        "copy" => "ctrl+c",
        "paste" => "ctrl+v",
        "cut" => "ctrl+x",
        "select-all" => "ctrl+a",
        "new" => "ctrl+n",
        "save" => "ctrl+s",
        "print" => "ctrl+p",
        "open" => "win+e",
        "undo" => "ctrl+z",
        "close-window" => "alt+f4",
        "display-desktop" => "win+d",
        "run" => "win+r",
        "minimize" => "win+down",
        "maximize" => "win+up",
        _ => return None,
    })
}

/// HID Consumer-page usage for a media function (sent with type 0x80).
fn media_usage(name: &str) -> Option<u16> {
    Some(match name {
        "media-select" => 0x183,
        "prev-track" => 0xb6,
        "next-track" => 0xb5,
        "stop" => 0xb7,
        "play-pause" => 0xcd,
        "mute" => 0xe2,
        "volume-up" => 0xe9,
        "volume-down" => 0xea,
        "mail" => 0x18a,
        "calculator" => 0x192,
        "my-computer" => 0x194,
        _ => return None,
    })
}

/// `ctrl+c`, `alt+f4`, `win+d`...: a keyboard shortcut (type 0x70, modifier mask, HID key usage).
fn parse_key_combo(combo: &str) -> Option<[u8; 3]> {
    let mut modifiers = 0u8;
    let mut key = None;
    for part in combo.split('+') {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" => modifiers |= 0x01,
            "shift" => modifiers |= 0x02,
            "alt" => modifiers |= 0x04,
            "win" => modifiers |= 0x08,
            k if key.is_none() => key = Some(key_usage(k)?),
            _ => return None,
        }
    }
    Some([0x70, modifiers, key?])
}

/// HID keyboard-page usage of a key name (letters, digits, F1-F12, arrows, editing keys and some punctuation).
pub(super) fn key_usage(k: &str) -> Option<u8> {
    let b = k.as_bytes();
    Some(match k {
        _ if b.len() == 1 && b[0].is_ascii_lowercase() => b[0] - b'a' + 0x04,
        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" => b[0] - b'1' + 0x1e,
        "0" => 0x27,
        "enter" => 0x28,
        "esc" => 0x29,
        "tab" => 0x2b,
        "space" => 0x2c,
        "right" => 0x4f,
        "left" => 0x50,
        "down" => 0x51,
        "up" => 0x52,
        "backspace" => 0x2a,
        "minus" => 0x2d,
        "equal" => 0x2e,
        "semicolon" => 0x33,
        "quote" => 0x34,
        "comma" => 0x36,
        "period" => 0x37,
        "slash" => 0x38,
        "insert" => 0x49,
        "home" => 0x4a,
        "pageup" => 0x4b,
        "delete" => 0x4c,
        "end" => 0x4d,
        "pagedown" => 0x4e,
        _ if k.starts_with('f') => match k[1..].parse::<u8>() {
            Ok(n @ 1..=12) => 0x3a + n - 1,
            _ => return None,
        },
        _ => return None,
    })
}

/// Assign a function to one button (register 0x04).
pub fn button(seq: u8, which: Button, what: Function) -> Frame {
    let c = what.code();
    frame(REG_BUTTON, seq, &[which.index(), c[0], c[1], c[2]])
}
