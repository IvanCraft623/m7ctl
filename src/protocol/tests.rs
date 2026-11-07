//! Byte-level tests of the protocol builders and parsers.

use super::*;

fn hex(f: &Frame, n: usize) -> String {
    f[..n].iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}

// The expected strings below are frames that were sent to the real mouse and acknowledged.

#[test]
fn macro_descriptions() {
    assert_eq!(parse_macro("tap:a").unwrap(), vec![[0x30, 20, 0x04], [0xb0, 20, 0x04]]);
    assert_eq!(parse_macro("tap:ctrl+c").unwrap(), vec![[0x20, 20, 0x01], [0x30, 20, 0x06], [0xb0, 20, 0x06], [0xa0, 20, 0x01]]);
    // a delay is added to the previous event: 20 + 1000 = 1020 = 0x3fc
    assert_eq!(parse_macro("down:a,wait:1000").unwrap(), vec![[0x33, 0xfc, 0x04]]);
    assert_eq!(parse_macro("type:Hi").unwrap().len(), 6);
    assert!(parse_macro("wait:5").is_err());
    assert!(parse_macro("tap:nope").is_err());
    assert_eq!(macro_clear(), vec![[0x30, 20, 0], [0xb0, 20, 0]]);
}

#[test]
fn macro_upload_frames() {
    let mut seq = 0x10u8;
    let mut next = || {
        seq += 1;
        seq
    };
    let one = macro_frames(&mut next, 1, &[[0x30, 0x14, 0x04], [0xb0, 0x14, 0x04]]).unwrap();
    assert_eq!(one.len(), 1);
    assert_eq!(&one[0][..14], &[0x0c, 0x01, 0x0e, 0x00, 0x11, 0x01, 0x07, 0x01, 0x30, 0x14, 0x04, 0xb0, 0x14, 0x04]);
    // 9 events = 27 bytes = two chunks (25 + 2)
    let nine = macro_frames(&mut next, 3, &[[1, 2, 3]; 9]).unwrap();
    assert_eq!(nine.len(), 2);
    assert_eq!((nine[0][5], nine[0][6], nine[1][5], nine[1][6]), (0x02, 26, 0x12, 3));
    assert!(macro_frames(&mut next, 0, &[[0; 3]]).is_err());
}

#[test]
fn status_query_matches_capture() {
    assert_eq!(hex(&status_query(1), 8), "0c 01 20 00 01 00 00 00");
}

#[test]
fn status_reply_is_parsed() {
    let bytes = "0c 01 20 00 02 01 10 00 4d 32 36 33 27 01 00 95 46 ff 64 01 ff 01 00";
    let mut reply = [0u8; REPORT_LEN];
    for (i, b) in bytes.split(' ').enumerate() {
        reply[i] = u8::from_str_radix(b, 16).unwrap();
    }
    let s = parse_status(&reply).unwrap();
    assert_eq!((s.model.as_str(), s.variant, s.battery, s.online), ("M263", 0x95, 100, true));
    assert_eq!(s.charge, Charge::Charging);
    assert!(parse_status(&[0u8; REPORT_LEN]).is_none());
}

/// The vendor app wrote zeros into the two unused slots; we repeat the last stage (masked out either way).
#[test]
fn dpi_matches_capture() {
    let f = dpi_stages(0x28, &[800, 1600, 2400, 3200, 4800, 6400], 0, &DpiTable::standard()).unwrap();
    assert_eq!(hex(&f, 25), "0c 01 05 00 28 01 12 16 3f 0f 00 1f 00 2f 00 3f 00 5f 00 7f 00 7f 00 7f 00");
}

#[test]
fn dpi_rejects_bad_values() {
    let t = DpiTable::standard();
    assert!(dpi_stages(1, &[], 0, &t).is_err());
    assert!(dpi_stages(1, &[825], 0, &t).is_err());
    assert!(dpi_stages(1, &[26050], 0, &t).is_err());
    assert!(dpi_stages(1, &[800, 1600], 2, &t).is_err());
}

#[test]
fn polling_matches_capture() {
    assert_eq!(hex(&polling_rate(10, 250).unwrap(), 8), "0c 01 07 00 0a 01 01 04");
    assert_eq!(polling_rate(1, 1000).unwrap()[7], 1);
    assert_eq!(polling_rate(1, 500).unwrap()[7], 2);
    assert!(polling_rate(1, 333).is_err());
}

#[test]
fn button_matches_capture() {
    let f = button(0x5a, Button::Dpi, Function::Middle);
    assert_eq!(hex(&f, 11), "0c 01 04 00 5a 01 04 05 10 04 00");
    assert_eq!(Button::Back.index(), 3);
    assert_eq!(Button::Forward.index(), 4);
    assert_eq!(Function::DpiPlus.code(), [0x40, 0x02, 0]);
    // codes confirmed on the live mouse (they produced these HID reports)
    assert_eq!(Function::parse("volume-up").unwrap().code(), [0x80, 0xe9, 0x00]);
    assert_eq!(Function::parse("calculator").unwrap().code(), [0x80, 0x92, 0x01]);
    assert_eq!(Function::parse("key:ctrl+c").unwrap().code(), [0x70, 0x01, 0x06]);
    assert_eq!(Function::parse("key:ctrl+v").unwrap().code(), [0x70, 0x01, 0x19]);
    assert_eq!(Function::parse("key:alt+f4").unwrap().code(), [0x70, 0x04, 0x3d]);
    assert_eq!(Function::parse("key:win+d").unwrap().code(), [0x70, 0x08, 0x07]);
    assert!(Function::parse("key:ctrl+").is_none());
    assert_eq!(Function::parse("macro:1").unwrap().code(), [0x90, 0x01, 0]);
    assert_eq!(Function::parse("macro:3,2,5").unwrap().code(), [0x90, 0x23, 5]);
    assert_eq!(Function::parse("dpi-lock:6").unwrap().code(), [0x50, 6, 0]);
    assert!(Function::parse("dpi-lock:12").is_none());
    assert_eq!(Function::parse("three-click").unwrap().code(), [0x30, 10, 3]);
    assert_eq!(Function::parse("disable").unwrap().code(), [0, 0, 0]);
    assert_eq!(Function::parse("tilt-left").unwrap().code(), [0x10, 0x40, 0x00]);
    assert_eq!(Function::parse("tilt-right").unwrap().code(), [0x10, 0x80, 0x00]);
    assert_eq!(Function::parse("fire:10,3").unwrap().code(), [0x30, 10, 3]);
    assert_eq!(Function::parse("fire:10").unwrap().code(), [0x30, 10, 0]);
    assert!(Function::parse("fire:2").is_none());
    assert_eq!(Function::parse("switch-effect").unwrap().code(), [0x60, 0x04, 0x00]);
    assert_eq!(Function::parse("copy").unwrap().code(), [0x70, 0x01, 0x06]);
    assert_eq!(Function::parse("open").unwrap().code(), [0x70, 0x08, 0x08]);
    assert_eq!(Function::parse("maximize").unwrap().code(), [0x70, 0x08, 0x52]);
    assert_eq!(Function::parse("undo").unwrap().code(), [0x70, 0x01, 0x1d]);
}

#[test]
fn lighting_matches_captures() {
    assert_eq!(hex(&light_steady(100, [255, 0, 0], 3).unwrap(), 12), "0c 01 08 00 64 01 05 03 42 ff 00 00");
    assert_eq!(hex(&light_off(0x67), 9), "0c 01 08 00 67 01 02 00 42");
    let f = light_cycle(0x75, &[[255, 0, 0], [0, 255, 0], [0, 0, 255]], 3, 1).unwrap();
    assert_eq!(hex(&f, 19), "0c 01 08 00 75 01 0c 02 42 03 ff 00 00 00 ff 00 00 00 ff");
}

#[test]
fn dpi_colors_are_padded() {
    let f = dpi_colors(1, &[[1, 2, 3]]).unwrap();
    assert_eq!(f[6], 0x18);
    assert_eq!(&f[7 + 21..7 + 24], &[1, 2, 3]);
}

#[test]
fn parsing_helpers() {
    assert_eq!(parse_rgb("#ff8000").unwrap(), [255, 128, 0]);
    assert!(parse_rgb("ff80").is_err());
    assert_eq!(parse_hex_bytes("0a ff").unwrap(), vec![10, 255]);
    assert!(parse_hex_bytes("abc").is_err());
}

#[test]
fn eight_dpi_stages_fill_the_payload_and_mask() {
    let d: Vec<u32> = (1..=8).map(|i| i * 400).collect();
    let f = dpi_stages(1, &d, 0, &DpiTable::standard()).unwrap();
    assert_eq!(f[7], 0x18);
    assert_eq!(f[8], 0xff);
    assert_eq!(&f[9..13], &[7, 0, 15, 0]);
    assert_eq!(&f[23..25], &[63, 0]);
}

#[test]
fn light_breathing_is_mode_6() {
    assert_eq!(hex(&light_breathing(0x10, [255, 0, 0], 2).unwrap(), 12), "0c 01 08 00 10 01 05 06 32 ff 00 00");
}

#[test]
fn light_flash_and_speed() {
    assert_eq!(hex(&light_flash(0x10, [255, 0, 0], 2).unwrap(), 12), "0c 01 08 00 10 01 05 07 32 ff 00 00");
    let f = light_cycle(0x11, &[[255, 0, 0]], 2, 3).unwrap();
    assert_eq!(hex(&f, 10), "0c 01 08 00 11 01 06 02 34 01");
    assert!(light_cycle(0x11, &[[255, 0, 0]], 2, 4).is_err());
}

#[test]
fn macro_accepts_editing_keys_and_punctuation() {
    assert!(parse_macro("tap:backspace,tap:delete,tap:home,tap:end,tap:pageup,tap:pagedown,tap:insert").is_ok());
    assert!(parse_macro("tap:comma,tap:period,tap:slash,tap:minus,tap:equal,tap:semicolon,tap:quote").is_ok());
    let ev = parse_macro("type:a.b-c/d").unwrap();
    assert_eq!(ev.len(), 14, "7 characters, each pressed and released");
    assert!(parse_macro("tap:ctrl+backspace").is_ok());
    assert!(parse_macro("type:a!").is_err(), "shifted punctuation is not supported");
}

mod dpi_event_tests {
    use super::*;
    #[test]
    fn parses_captured_report() {
        let r = [0x05, 0x06, 0x01, 0x02, 0x03, 0x2f, 0x00, 0x2f, 0x00, 0, 0, 0];
        assert_eq!(parse_dpi_event(&r, &DpiTable::standard()), Some((3, 2400)));
        assert_eq!(parse_dpi_event(&[0x05, 0x06, 0x01, 0x02, 0x01, 0x0f, 0x00], &DpiTable::standard()), Some((1, 800)));
        assert_eq!(parse_dpi_event(&[0x01, 0, 0, 0, 0, 0, 0], &DpiTable::standard()), None);
    }
}
