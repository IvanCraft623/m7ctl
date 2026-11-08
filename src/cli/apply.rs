//! Commands that write a setting to the mouse (and remember it).

use super::{Command, Context, LightCmd};
use crate::device::Delivery;
use crate::protocol::{self, Button, Function, Rgb};
use std::io::Write;

fn w(e: std::io::Error) -> String {
    e.to_string()
}

fn colors(s: &str) -> Result<Vec<Rgb>, String> {
    s.split(',').map(|c| protocol::parse_rgb(c.trim())).collect()
}

fn report(out: &mut dyn Write, what: &str, d: &Delivery) -> Result<(), String> {
    match d {
        Delivery::Applied => writeln!(out, "{what}: done (the mouse acknowledged it)"),
        Delivery::NotAcked => writeln!(out, "{what}: sent, but nothing acknowledged it"),
        Delivery::Asleep => writeln!(out, "{what}: NOT applied (the mouse is asleep)"),
    }
    .map_err(w)
}

/// Report the result and remember the setting when the mouse acknowledged it.
fn done(ctx: &Context, out: &mut dyn Write, key: &str, args: Vec<String>, what: &str, d: Delivery) -> Result<(), String> {
    report(out, what, &d)?;
    if d == Delivery::Applied {
        ctx.record(key, args);
    }
    Ok(())
}

fn strings<const N: usize>(a: [&str; N]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

pub fn run(command: Command, ctx: &Context, out: &mut dyn Write) -> Result<(), String> {
    match command {
        Command::Dpi { values, active } => {
            let stages: Vec<u32> =
                values.split(',').map(|v| v.trim().parse().map_err(|_| format!("'{v}' is not a DPI number"))).collect::<Result<_, String>>()?;
            let active0 = active.checked_sub(1).ok_or("--active starts at 1")?;
            let table = ctx.store.dpi_table();
            let ack = ctx.mouse()?.write_wake(|seq| protocol::dpi_stages(seq, &stages, active0, &table))?;
            let args = vec![values.clone(), "--active".into(), active.to_string()];
            done(ctx, out, "dpi", args, &format!("{} DPI stage(s), starting on stage {active}", stages.len()), ack)
        }
        Command::Rate { hz } => {
            let ack = ctx.mouse()?.write_wake(|seq| protocol::polling_rate(seq, hz))?;
            done(ctx, out, "rate", vec![hz.to_string()], &format!("polling rate {hz} Hz"), ack)
        }
        Command::Button { button, function, force } => {
            let b = Button::parse(&button).ok_or("unknown button (left right middle back forward dpi)")?;
            let f = Function::parse(&function).ok_or("unknown function (see `m7ctl button --help`; shortcuts look like key:ctrl+c)")?;
            if b == Button::Left && f != Function::Left && !force {
                return Err("refusing to remove the left click from the left button (use --force)".into());
            }
            let ack = ctx.mouse()?.write_wake(|seq| Ok(protocol::button(seq, b, f)))?;
            let mut args = vec![button.clone(), function.clone()];
            if force {
                args.push("--force".into());
            }
            done(ctx, out, &format!("button:{}", button.to_ascii_lowercase()), args, &format!("button {b:?} -> {f:?}"), ack)
        }
        Command::Light(cmd) => light(cmd, ctx, out),
        Command::DpiColors { colors: list } => {
            let c = colors(&list)?;
            let ack = ctx.mouse()?.write_wake(|seq| protocol::dpi_colors(seq, &c))?;
            done(ctx, out, "dpi-colors", vec![list], "DPI indicator colours", ack)
        }
        Command::Lod { mm } => {
            if !(1..=2).contains(&mm) {
                return Err("lift-off distance must be 1 or 2 (mm)".into());
            }
            let ack = ctx.mouse()?.write_wake(|seq| protocol::raw(seq, 0x09, &[mm, 0]))?;
            done(ctx, out, "lod", vec![mm.to_string()], &format!("lift-off distance {mm} mm"), ack)
        }
        Command::Power { minutes, lights_off_moving, force } => {
            if !(3..=10).contains(&minutes) && !force {
                return Err("power-down time is 3-10 minutes (use --force for other values)".into());
            }
            let flag = u8::from(lights_off_moving.is_some());
            let ack = ctx.mouse()?.write_wake(|seq| protocol::raw(seq, 0x0b, &[minutes, flag]))?;
            let mut args = vec![minutes.to_string()];
            args.extend(lights_off_moving);
            if force {
                args.push("--force".into());
            }
            let what = format!("power-down time {minutes}, lights off while moving: {}", if flag == 1 { "yes" } else { "no" });
            done(ctx, out, "power", args, &what, ack)
        }
        Command::MotionWake { state } => {
            let on = u8::from(state == "on");
            let ack = ctx.mouse()?.write_wake(|seq| protocol::raw(seq, 0x0c, &[on]))?;
            done(ctx, out, "motion-wake", vec![state.clone()], &format!("motion wake {state}"), ack)
        }
        Command::Macro { slot, steps } => store_macro(slot, &steps, ctx, out),
        Command::MacroRaw { slot, events } => {
            let bytes = protocol::parse_hex_bytes(&events)?;
            if bytes.is_empty() || bytes.len() % 3 != 0 {
                return Err("events are 3 bytes each".into());
            }
            let evs: Vec<[u8; 3]> = bytes.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
            let mut mouse = ctx.mouse()?;
            let mut counter = mouse.next_seq();
            let frames = protocol::macro_frames(
                &mut || {
                    counter = counter.wrapping_add(1);
                    counter
                },
                slot,
                &evs,
            )?;
            for f in &frames {
                match mouse.write_prebuilt(f)? {
                    Delivery::Applied => {}
                    other => return Err(format!("macro upload stopped: {other:?} (move the mouse and retry)")),
                }
            }
            writeln!(out, "macro {slot}: {} events uploaded in {} frame(s)", evs.len(), frames.len()).map_err(w)
        }
        Command::Raw { register, payload } => {
            let reg = u8::from_str_radix(register.trim_start_matches("0x"), 16).map_err(|_| "register must be hex, like 0x07")?;
            let payload = match payload {
                Some(p) => protocol::parse_hex_bytes(&p)?,
                None => vec![],
            };
            let mut mouse = ctx.mouse()?;
            let seq = mouse.next_seq();
            let reply = mouse.raw_exchange(&protocol::raw(seq, reg, &payload)?)?;
            writeln!(out, "reply: {}", reply.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")).map_err(w)
        }
        _ => unreachable!("handled by cli::run"),
    }
}

fn light(cmd: LightCmd, ctx: &Context, out: &mut dyn Write) -> Result<(), String> {
    match cmd {
        LightCmd::Off => {
            let ack = ctx.mouse()?.write_wake(|seq| Ok(protocol::light_off(seq)))?;
            done(ctx, out, "light", strings(["off"]), "lights off", ack)
        }
        LightCmd::Steady { color, b } => {
            let c = protocol::parse_rgb(&color)?;
            let ack = ctx.mouse()?.write_wake(|seq| protocol::light_steady(seq, c, b.brightness))?;
            done(ctx, out, "light", vec!["steady".into(), color, "--brightness".into(), b.brightness.to_string()], "steady light", ack)
        }
        LightCmd::Cycle { colors: list, b, speed } => {
            let c = colors(&list)?;
            let ack = ctx.mouse()?.write_wake(|seq| protocol::light_cycle(seq, &c, b.brightness, speed))?;
            let args = vec!["cycle".into(), list, "--brightness".into(), b.brightness.to_string(), "--speed".into(), speed.to_string()];
            done(ctx, out, "light", args, "colour cycle", ack)
        }
        LightCmd::Breathing { color, b } => {
            let c = protocol::parse_rgb(&color)?;
            let ack = ctx.mouse()?.write_wake(|seq| protocol::light_breathing(seq, c, b.brightness))?;
            done(ctx, out, "light", vec!["breathing".into(), color, "--brightness".into(), b.brightness.to_string()], "breathing light", ack)
        }
        LightCmd::Flash { color, b } => {
            let c = protocol::parse_rgb(&color)?;
            let ack = ctx.mouse()?.write_wake(|seq| protocol::light_flash(seq, c, b.brightness))?;
            done(ctx, out, "light", vec!["flash".into(), color, "--brightness".into(), b.brightness.to_string()], "flash light", ack)
        }
    }
}

fn store_macro(slot: u8, spec: &str, ctx: &Context, out: &mut dyn Write) -> Result<(), String> {
    let events = if spec == "clear" { protocol::macro_clear() } else { protocol::parse_macro(spec)? };
    let mut mouse = ctx.mouse()?;
    let mut counter = mouse.next_seq();
    let frames = protocol::macro_frames(
        &mut || {
            counter = counter.wrapping_add(1);
            counter
        },
        slot,
        &events,
    )?;
    mouse.write_all_wake(&frames, "macro")?;
    if spec == "clear" {
        ctx.forget(&format!("macro:{slot}"));
        writeln!(out, "macro {slot}: cleared (it now presses no key)").map_err(w)
    } else {
        ctx.record(&format!("macro:{slot}"), vec![slot.to_string(), spec.to_string()]);
        writeln!(out, "macro {slot}: {} events uploaded (assign it with `button <button> macro:{slot}`)", events.len()).map_err(w)
    }
}
