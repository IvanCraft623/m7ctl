//! `list` and `status`.

use super::Context;
use crate::device::{self, link_name};
use crate::protocol::Charge;
use std::io::Write;

fn w(e: std::io::Error) -> String {
    e.to_string()
}

pub fn list(verbose: bool, out: &mut dyn Write) -> Result<(), String> {
    if verbose {
        let all = device::all_interfaces()?;
        if all.is_empty() {
            writeln!(out, "no Machenike HID device found").map_err(w)?;
        }
        for i in all {
            writeln!(out, "{}  {:04x}:{:04x}  interface {}  usage {:04x}:{:04x}", i.path, i.vid, i.pid, i.interface, i.usage_page, i.usage).map_err(w)?;
        }
        return Ok(());
    }
    let found = device::find_devices()?;
    if found.is_empty() {
        writeln!(out, "no Machenike M7 Pro configuration interface found").map_err(w)?;
    }
    for c in found {
        let mode = if c.pid == device::PID_WIRED { "wired" } else { "wireless receiver" };
        writeln!(out, "{}  ({mode}, 248a:{:04x})", c.path, c.pid).map_err(w)?;
    }
    Ok(())
}

/// What `status` reports, also used directly by the terminal UI.
pub struct StatusReport {
    pub device: String,
    pub link: &'static str,
    pub model: String,
    pub variant: u8,
    pub battery: u8,
    pub charge: &'static str,
    pub online: bool,
    pub identity: [u8; 3],
}

pub fn read_status(ctx: &Context) -> Result<StatusReport, String> {
    let mut mouse = ctx.mouse()?;
    let s = mouse.status()?;
    if crate::protocol::DpiTable::for_variant(s.variant).is_some() {
        ctx.store.set_variant(s.variant);
    }
    Ok(StatusReport {
        device: mouse.path().to_string(),
        link: link_name(mouse.pid()),
        model: s.model,
        variant: s.variant,
        battery: s.battery,
        charge: match s.charge {
            Charge::Charging => "charging",
            Charge::Full => "full",
            Charge::Discharging => "on battery",
        },
        online: s.online,
        identity: s.identity,
    })
}

pub fn status(ctx: &Context, json: bool, out: &mut dyn Write) -> Result<(), String> {
    let r = read_status(ctx)?;
    if json {
        writeln!(
            out,
            "{{\"device\":\"{}\",\"link\":\"{}\",\"model\":\"{}\",\"variant\":{},\"battery\":{},\"charge\":\"{}\",\"online\":{},\"identity\":[{},{},{}]}}",
            r.device.replace('\\', "\\\\").replace('"', "\\\""),
            r.link,
            r.model,
            r.variant,
            r.battery,
            r.charge,
            r.online,
            r.identity[0],
            r.identity[1],
            r.identity[2]
        )
        .map_err(w)?;
        return Ok(());
    }
    writeln!(out, "device   {} ({})", r.device, r.link).map_err(w)?;
    writeln!(out, "model    {} (variant {:#04x})", r.model, r.variant).map_err(w)?;
    writeln!(out, "battery  {}% ({})", r.battery, r.charge).map_err(w)?;
    writeln!(out, "online   {}", if r.online { "yes" } else { "no" }).map_err(w)?;
    let table = ctx.store.dpi_table();
    writeln!(out, "dpi      {}-{} (sensor table of variant {:#04x})", table.min_dpi(), table.max_dpi(), table.variant).map_err(w)?;
    writeln!(out, "identity {:02x} {:02x} {:02x}", r.identity[0], r.identity[1], r.identity[2]).map_err(w)?;
    Ok(())
}
