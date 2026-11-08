//! Command-line behaviour against a fake mouse and a temporary configuration directory.

use m7ctl::cli::{execute, Context};
use m7ctl::device::fake::{FakeTransport, Log};
use m7ctl::device::Mouse;
use m7ctl::settings::Store;
use std::sync::{Arc, Mutex};

struct Rig {
    ctx: Context,
    log: Arc<Mutex<Log>>,
}

fn rig(tag: &str, asleep: bool) -> Rig {
    let dir = std::env::temp_dir().join(format!("m7ctl-it-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let log = Arc::new(Mutex::new(Log::default()));
    let shared = log.clone();
    let ctx = Context {
        store: Store::at(dir),
        device: None,
        // every command opens the mouse again; all the fakes record into the same log
        opener: Arc::new(move |_| Ok(Mouse::new(Box::new(FakeTransport::logging_to(shared.clone()).asleep(asleep))).without_delays())),
        notice: Arc::new(|_| {}),
    };
    Rig { ctx, log }
}

fn run(r: &Rig, line: &str) -> Result<String, String> {
    let args: Vec<String> = line.split_whitespace().map(String::from).collect();
    let mut out = Vec::new();
    execute(&args, &r.ctx, &mut out).map(|_| String::from_utf8(out).unwrap())
}

fn registers(r: &Rig) -> Vec<u8> {
    r.log.lock().unwrap().frames.iter().map(|f| f[2]).collect()
}

#[test]
fn rate_is_written_and_remembered() {
    let r = rig("rate", false);
    let out = run(&r, "rate 500").unwrap();
    assert!(out.contains("polling rate 500 Hz: done"), "{out}");
    assert_eq!(registers(&r), vec![0x07]);
    assert_eq!(r.ctx.store.read_state(), vec![("rate".to_string(), vec!["500".to_string()])]);
}

#[test]
fn a_sleeping_mouse_is_not_remembered() {
    let r = rig("asleep", true);
    let err = run(&r, "rate 500").unwrap_err();
    assert!(err.contains("stayed asleep"), "{err}");
    assert!(r.ctx.store.read_state().is_empty());
}

#[test]
fn bad_input_is_rejected_before_touching_the_mouse() {
    let r = rig("bad", false);
    assert!(run(&r, "power 20").unwrap_err().contains("3-10"));
    assert!(run(&r, "button left right").unwrap_err().contains("refusing"));
    assert!(run(&r, "light steady ff0000 --brightness 9").is_err());
    assert!(run(&r, "dpi 800,abc").is_err());
    assert!(registers(&r).is_empty());
}

#[test]
fn profile_save_and_load_replay_the_settings_in_order() {
    let r = rig("profile", false);
    run(&r, "button dpi middle").unwrap();
    run(&r, "rate 1000").unwrap();
    run(&r, "dpi 800,1600 --active 2").unwrap();
    run(&r, "profile save gaming").unwrap();
    r.log.lock().unwrap().frames.clear();
    let out = run(&r, "profile load gaming").unwrap();
    assert!(out.contains("loaded"), "{out}");
    // dpi (0x05), rate (0x07), then buttons (0x04) last
    assert_eq!(registers(&r), vec![0x05, 0x07, 0x04]);
    assert_eq!(r.ctx.store.active_profile().as_deref(), Some("gaming"));
    assert_eq!(r.ctx.store.read_state().len(), 3);
}

#[test]
fn macros_are_stored_and_cleared() {
    let r = rig("macro", false);
    run(&r, "macro 2 tap:ctrl+c").unwrap();
    assert!(r.ctx.store.read_state().iter().any(|(k, _)| k == "macro:2"));
    run(&r, "macro 2 clear").unwrap();
    assert!(r.ctx.store.read_state().is_empty());
}

#[test]
fn status_reports_the_fake_mouse() {
    let r = rig("status", false);
    let out = run(&r, "status --json").unwrap();
    assert!(out.contains("\"model\":\"M263\"") && out.contains("\"link\":\"wired\""), "{out}");
}

#[test]
fn lod_sends_millimetres_then_zero() {
    let r = rig("lod", false);
    run(&r, "lod 1").unwrap();
    let f = r.log.lock().unwrap().frames[0];
    assert_eq!((f[2], f[6], f[7], f[8]), (0x09, 2, 1, 0));
}

#[test]
fn breathing_and_flash_use_modes_6_and_7() {
    let r = rig("light", false);
    run(&r, "light breathing ff0000 --brightness 2").unwrap();
    run(&r, "light flash 00ff00").unwrap();
    let modes: Vec<u8> = r.log.lock().unwrap().frames.iter().map(|f| f[7]).collect();
    assert_eq!(modes, vec![6, 7]);
    assert_eq!(r.ctx.store.read_state().len(), 1, "one light setting is remembered");
}

#[test]
fn imported_psf_profile_loads_onto_the_mouse() {
    let r = rig("psf", false);
    let text = "[Setting]\r\nProfileName=Imp\r\nDPIOffOnFlagValue=03\r\nXYSetting1Value=0F\r\nXYSetting2Value=1F\r\nCurrentDPILevelValue=01\r\nPollingRateValue=020000\r\nKey1Value=700106\r\n";
    let mut bytes = vec![0xff, 0xfe];
    bytes.extend(text.encode_utf16().flat_map(|u| u.to_le_bytes()));
    let file = r.ctx.store.dir().join("in.psf");
    std::fs::create_dir_all(r.ctx.store.dir()).unwrap();
    std::fs::write(&file, bytes).unwrap();
    let out = run(&r, &format!("profile import {}", file.display())).unwrap();
    assert!(out.contains("profile 'Imp' created"), "{out}");
    assert!(r.log.lock().unwrap().frames.is_empty(), "importing must not write to the mouse");
    run(&r, "profile load Imp").unwrap();
    // dpi, dpi-colors, rate, then the button (raw code 70 01 06 = ctrl+c)
    assert_eq!(registers(&r), vec![0x05, 0x06, 0x07, 0x04]);
    let button = r.log.lock().unwrap().frames[3];
    assert_eq!(&button[7..11], &[1, 0x70, 0x01, 0x06]);
}

#[test]
fn dpi_uses_the_table_of_the_remembered_sensor_variant() {
    let r = rig("variant", false);
    r.ctx.store.set_variant(0x25);
    assert!(run(&r, "dpi 250").unwrap_err().contains("not available"));
    run(&r, "dpi 200,5500").unwrap();
    let f = r.log.lock().unwrap().frames[0];
    // variant 0x25: 200 DPI is table entry 0, sent as code 1 (offset 1)
    assert_eq!(&f[9..11], &[1, 0]);
    // the status of a known model remembers its variant
    let r2 = rig("variant2", false);
    run(&r2, "status").unwrap();
    assert_eq!(r2.ctx.store.variant(), Some(0x95));
}
