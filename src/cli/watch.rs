//! `watch`: follow the mouse's DPI-stage reports.

use super::Context;
use crate::{notify, protocol};
use std::io::Write;
use std::time::Duration;

/// Runs until interrupted; reconnects when the receiver goes away.
pub fn run(ctx: &Context, popup: bool, out: &mut dyn Write) -> Result<(), String> {
    let mut announced = false;
    loop {
        match ctx.mouse() {
            Ok(mut mouse) => {
                if !announced {
                    eprintln!("watching {} (Ctrl-C to stop)", mouse.path());
                    announced = true;
                }
                while let Ok(r) = mouse.read_report() {
                    if let Some((stage, dpi)) = protocol::parse_dpi_event(&r, &ctx.store.dpi_table()) {
                        writeln!(out, "DPI stage {stage}: {dpi}").map_err(|e| e.to_string())?;
                        if popup {
                            notify::dpi_changed(stage, dpi);
                        }
                    }
                }
            }
            Err(e) if ctx.device.is_some() => return Err(e),
            Err(_) => {}
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}
