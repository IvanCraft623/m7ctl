//! Desktop popups (through the `notify-rust` crate: D-Bus on Linux, native toasts on Windows and macOS).

use notify_rust::{Notification, Timeout};

/// Popup for a DPI-stage change. On Linux a hint makes the desktop replace the previous popup instead of
/// stacking them (KDE and GNOME honour it); other systems show a short-lived toast. A missing notification
/// service is not an error.
pub fn dpi_changed(stage: u8, dpi: u32) {
    let mut n = Notification::new();
    n.appname("m7ctl").summary(&format!("DPI {dpi}")).body(&format!("Stage {stage}")).timeout(Timeout::Milliseconds(1500));
    #[cfg(all(unix, not(target_os = "macos")))]
    n.hint(notify_rust::Hint::Custom("x-canonical-private-synchronous".into(), "m7ctl-dpi".into()));
    let _ = n.show();
}
