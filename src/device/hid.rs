//! HID access through hidapi (the same code on Linux, Windows and macOS): find the mouse's
//! configuration interface and exchange feature reports.

use super::{Transport, PID_RECEIVER, PID_WIRED, VENDOR};
use crate::protocol::{Frame, REPORT_ID, REPORT_LEN};
use hidapi::{DeviceInfo, HidApi, HidDevice};
use std::ffi::CString;
use std::io;

/// The configuration interface is the vendor-defined collection (usage page 0xff00, usage 1) that holds report 0x0c.
const CONFIG_USAGE: (u16, u16) = (0xff00, 0x01);
/// The collection that carries the mouse's own input reports (DPI-stage changes, report 5).
const EVENT_USAGE: (u16, u16) = (0x01, 0x00);

/// A configuration interface found by [`find_devices`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub path: String,
    pub pid: u16,
}

/// One HID collection as hidapi lists it, reduced to what selection needs (and to what tests can build by hand).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interface {
    pub path: String,
    pub vid: u16,
    pub pid: u16,
    pub usage_page: u16,
    pub usage: u16,
    pub interface: i32,
}

impl Interface {
    fn from_info(i: &DeviceInfo) -> Interface {
        Interface {
            path: i.path().to_string_lossy().into_owned(),
            vid: i.vendor_id(),
            pid: i.product_id(),
            usage_page: i.usage_page(),
            usage: i.usage(),
            interface: i.interface_number(),
        }
    }

    fn is_m7(&self) -> bool {
        self.vid == VENDOR && (self.pid == PID_WIRED || self.pid == PID_RECEIVER)
    }
}

/// All M7 Pro configuration interfaces, wired mouse first.
pub fn select_config(all: &[Interface]) -> Vec<Candidate> {
    let mut found: Vec<Candidate> = Vec::new();
    for i in all.iter().filter(|i| i.is_m7() && (i.usage_page, i.usage) == CONFIG_USAGE) {
        if !found.iter().any(|c| c.path == i.path) {
            found.push(Candidate { path: i.path.clone(), pid: i.pid });
        }
    }
    found.sort_by_key(|c| (c.pid != PID_WIRED, c.path.clone()));
    found
}

/// Where the mouse's input reports come from: the event collection of the same USB interface.
/// On Linux all collections of an interface share one path; Windows and macOS give each its own.
pub fn select_events<'a>(all: &'a [Interface], config: &Interface) -> Option<&'a str> {
    all.iter()
        .find(|i| i.vid == config.vid && i.pid == config.pid && i.interface == config.interface && (i.usage_page, i.usage) == EVENT_USAGE)
        .map(|i| i.path.as_str())
}

fn enumerate(api: &HidApi) -> Vec<Interface> {
    api.device_list().map(Interface::from_info).collect()
}

/// A fresh context, which lists the HID devices present right now.
fn new_api() -> Result<HidApi, String> {
    HidApi::new().map_err(|e| format!("cannot list HID devices: {e}"))
}

/// Every HID collection of the mouse, for `list --verbose`.
pub fn all_interfaces() -> Result<Vec<Interface>, String> {
    Ok(enumerate(&new_api()?).into_iter().filter(Interface::is_m7).collect())
}

/// All M7 Pro configuration interfaces, wired mouse first.
pub fn find_devices() -> Result<Vec<Candidate>, String> {
    Ok(select_config(&all_interfaces()?))
}

fn io_err(e: hidapi::HidError) -> io::Error {
    io::Error::other(e.to_string())
}

fn open_error(path: &str, e: &hidapi::HidError) -> String {
    let text = e.to_string();
    if cfg!(target_os = "linux") && text.to_lowercase().contains("permission") {
        format!("cannot open {path}: permission denied. Install the udev rule from dist/ and replug the mouse")
    } else {
        format!("cannot open {path}: {text}")
    }
}

/// An open configuration interface.
pub struct Hid {
    api: HidApi,
    config: HidDevice,
    /// Opened on the first `read_report`; the config handle never reads.
    events: Option<HidDevice>,
    events_path: CString,
    path: String,
    pid: u16,
}

impl Hid {
    /// Open `path`, or the first M7 Pro configuration interface found (wired first).
    pub fn open(path: Option<&str>) -> Result<Hid, String> {
        let api = new_api()?;
        let all = enumerate(&api);
        let path = match path {
            Some(p) => p.to_string(),
            None => {
                select_config(&all).into_iter().next().map(|c| c.path).ok_or("no Machenike M7 Pro found (is it plugged in or switched on? try `m7ctl list`)")?
            }
        };
        let known = all.iter().find(|i| i.path == path);
        let pid = known.map_or(0, |i| i.pid);
        let events = known.and_then(|c| select_events(&all, c)).unwrap_or(&path);
        let events_path = CString::new(events.to_string()).map_err(|_| format!("bad device path {path}"))?;
        let c_path = CString::new(path.clone()).map_err(|_| format!("bad device path {path}"))?;
        let config = api.open_path(&c_path).map_err(|e| open_error(&path, &e))?;
        Ok(Hid { api, config, events: None, events_path, path, pid })
    }
}

impl Transport for Hid {
    fn set_feature(&mut self, f: &Frame) -> io::Result<()> {
        self.config.send_feature_report(f).map_err(io_err)
    }

    fn get_feature(&mut self) -> io::Result<Frame> {
        let mut buf = [0u8; REPORT_LEN];
        buf[0] = REPORT_ID;
        self.config.get_feature_report(&mut buf).map_err(io_err)?;
        Ok(buf)
    }

    fn read_report(&mut self) -> io::Result<Vec<u8>> {
        if self.events.is_none() {
            self.events = Some(self.api.open_path(&self.events_path).map_err(io_err)?);
        }
        let dev = self.events.as_ref().expect("opened above");
        let mut buf = [0u8; 64];
        let n = dev.read(&mut buf).map_err(io_err)?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "device closed"));
        }
        Ok(buf[..n].to_vec())
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn pid(&self) -> u16 {
        self.pid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iface(path: &str, pid: u16, page: u16, usage: u16, interface: i32) -> Interface {
        Interface { path: path.into(), vid: VENDOR, pid, usage_page: page, usage, interface }
    }

    // The receiver as Linux lists it: one node (hidraw1) carrying several collections, and a plain mouse node.
    fn receiver_linux() -> Vec<Interface> {
        vec![
            iface("/dev/hidraw0", PID_RECEIVER, 0x01, 0x02, 0),
            iface("/dev/hidraw1", PID_RECEIVER, 0x01, 0x06, 1),
            iface("/dev/hidraw1", PID_RECEIVER, 0x0c, 0x01, 1),
            iface("/dev/hidraw1", PID_RECEIVER, 0x01, 0x00, 1),
            iface("/dev/hidraw1", PID_RECEIVER, 0xff00, 0x01, 1),
        ]
    }

    #[test]
    fn picks_the_vendor_collection() {
        let found = select_config(&receiver_linux());
        assert_eq!(found, vec![Candidate { path: "/dev/hidraw1".into(), pid: PID_RECEIVER }]);
    }

    #[test]
    fn wired_comes_first_and_other_devices_are_ignored() {
        let mut all = receiver_linux();
        all.push(iface("/dev/hidraw5", PID_WIRED, 0xff00, 0x01, 1));
        all.push(Interface { vid: 0x046d, ..iface("/dev/hidraw9", PID_WIRED, 0xff00, 0x01, 1) });
        let found = select_config(&all);
        assert_eq!(found.iter().map(|c| c.path.as_str()).collect::<Vec<_>>(), ["/dev/hidraw5", "/dev/hidraw1"]);
    }

    #[test]
    fn a_path_is_listed_once() {
        let mut all = receiver_linux();
        all.push(iface("/dev/hidraw1", PID_RECEIVER, 0xff00, 0x01, 1));
        assert_eq!(select_config(&all).len(), 1);
    }

    #[test]
    fn events_share_the_node_on_linux() {
        let all = receiver_linux();
        let config = all.iter().find(|i| i.usage_page == 0xff00).unwrap();
        assert_eq!(select_events(&all, config), Some("/dev/hidraw1"));
    }

    #[test]
    fn events_have_their_own_path_on_windows_and_macos() {
        let all = vec![
            iface("collection-config", PID_RECEIVER, 0xff00, 0x01, 1),
            iface("collection-events", PID_RECEIVER, 0x01, 0x00, 1),
            iface("collection-events-other-interface", PID_RECEIVER, 0x01, 0x00, 2),
        ];
        assert_eq!(select_events(&all, &all[0]), Some("collection-events"));
    }

    #[test]
    fn no_event_collection_means_no_path() {
        let all = vec![iface("a", PID_WIRED, 0xff00, 0x01, 1)];
        assert_eq!(select_events(&all, &all[0]), None);
    }
}
