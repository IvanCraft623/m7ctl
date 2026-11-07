//! Talking to the mouse: a [`Transport`] moves raw reports, [`Mouse`] adds the protocol rules
//! (sequence numbers, pacing, acknowledgement, sleeping wireless mice).

pub mod fake;
pub mod hid;

use crate::protocol::{self, Frame, Status};
use std::io;
use std::sync::Arc;
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub use hid::{all_interfaces, find_devices, Candidate, Hid, Interface};

/// Machenike's USB vendor id and the two ways the M7 Pro attaches.
pub const VENDOR: u16 = 0x248a;
pub const PID_WIRED: u16 = 0x5b2e;
pub const PID_RECEIVER: u16 = 0x5b2f;

/// Raw access to one configuration interface. Implemented by [`Hid`] and by [`fake::FakeTransport`].
pub trait Transport: Send {
    fn set_feature(&mut self, frame: &Frame) -> io::Result<()>;
    fn get_feature(&mut self) -> io::Result<Frame>;
    /// Block until the mouse sends an input report (e.g. a DPI-stage change).
    fn read_report(&mut self) -> io::Result<Vec<u8>>;
    fn path(&self) -> &str;
    /// USB product id, or 0 when the device was given explicitly and its id is unknown.
    fn pid(&self) -> u16;
}

/// How the mouse is attached, from its product id.
pub fn link_name(pid: u16) -> &'static str {
    match pid {
        PID_WIRED => "wired",
        PID_RECEIVER => "wireless receiver",
        _ => "unknown",
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Delivery {
    Applied,
    NotAcked,
    Asleep,
}

pub struct Mouse {
    transport: Box<dyn Transport>,
    seq: u8,
    /// Pause after every frame.
    pace: Duration,
    /// Pause between retries while a wireless mouse is asleep.
    retry_pause: Duration,
    /// Where progress messages go ("the mouse is asleep..."); stderr by default.
    notice: Arc<dyn Fn(&str) + Send + Sync>,
}

impl Mouse {
    /// Open `path`, or the first M7 Pro configuration interface found.
    pub fn open(path: Option<&str>) -> Result<Mouse, String> {
        Ok(Mouse::new(Box::new(Hid::open(path)?)))
    }

    pub fn new(transport: Box<dyn Transport>) -> Mouse {
        // Start the counter somewhere unpredictable; the mouse only echoes it.
        let seed = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos() as u8).unwrap_or(1);
        Mouse { transport, seq: seed, pace: Duration::from_millis(60), retry_pause: Duration::from_millis(400), notice: Arc::new(|m| eprintln!("{m}")) }
    }

    /// Send progress messages to `notice` instead of stderr (the terminal UI shows them in its log).
    pub fn with_notice(mut self, notice: Arc<dyn Fn(&str) + Send + Sync>) -> Mouse {
        self.notice = notice;
        self
    }

    /// Same mouse without the real-time pauses (for tests).
    pub fn without_delays(mut self) -> Mouse {
        self.pace = Duration::ZERO;
        self.retry_pause = Duration::ZERO;
        self
    }

    pub fn path(&self) -> &str {
        self.transport.path()
    }

    pub fn pid(&self) -> u16 {
        self.transport.pid()
    }

    pub fn next_seq(&mut self) -> u8 {
        self.seq = self.seq.wrapping_add(1);
        self.seq
    }

    /// Block until the mouse sends an input report.
    pub fn read_report(&mut self) -> io::Result<Vec<u8>> {
        self.transport.read_report()
    }

    /// Send one setting frame and read the receiver's echo. Byte 3 of the echo is 1 when a wireless
    /// mouse is asleep: the receiver acknowledges the frame but the mouse never gets it.
    pub fn write(&mut self, build: &impl Fn(u8) -> Result<Frame, String>) -> Result<Delivery, String> {
        let seq = self.next_seq();
        let frame = build(seq)?;
        self.write_prebuilt(&frame)
    }

    /// Send an already-built frame (its sequence byte is in the frame) and read the echo.
    pub fn write_prebuilt(&mut self, frame: &Frame) -> Result<Delivery, String> {
        self.transport.set_feature(frame).map_err(|e| format!("write failed: {e}"))?;
        sleep(self.pace);
        Ok(match self.transport.get_feature() {
            Ok(r) if r[2] == frame[2] && r[4] == frame[4] => {
                if r[3] == 0 {
                    Delivery::Applied
                } else {
                    Delivery::Asleep
                }
            }
            _ => Delivery::NotAcked,
        })
    }

    /// Like [`Mouse::write`], but while a wireless mouse is asleep keep resending (about 12 s) so the
    /// frame lands as soon as the user moves it.
    pub fn write_wake(&mut self, build: impl Fn(u8) -> Result<Frame, String>) -> Result<Delivery, String> {
        let mut warned = false;
        for _ in 0..30 {
            match self.write(&build)? {
                Delivery::Asleep => {
                    if !warned {
                        (self.notice)("the mouse is asleep: move it to wake it (waiting up to 12 s)...");
                        warned = true;
                    }
                    sleep(self.retry_pause);
                }
                other => return Ok(other),
            }
        }
        Err("the mouse stayed asleep: the command was NOT applied".into())
    }

    /// Upload prebuilt frames (a macro), waiting for a sleeping mouse to wake up.
    pub fn write_all_wake(&mut self, frames: &[Frame], what: &str) -> Result<(), String> {
        let (mut warned, mut tries) = (false, 0);
        for f in frames {
            loop {
                match self.write_prebuilt(f)? {
                    Delivery::Applied => break,
                    Delivery::Asleep if !warned => {
                        (self.notice)("the mouse is asleep: move it to wake it...");
                        warned = true;
                    }
                    Delivery::Asleep => {}
                    Delivery::NotAcked => return Err(format!("the mouse did not acknowledge the {what}")),
                }
                tries += 1;
                if tries > 30 {
                    return Err(format!("the mouse stayed asleep, so the {what} was NOT stored; move it and retry"));
                }
                sleep(self.retry_pause);
            }
        }
        Ok(())
    }

    /// Ask the mouse for its status. It sometimes needs a second try right after being opened.
    pub fn status(&mut self) -> Result<Status, String> {
        for _ in 0..4 {
            let seq = self.next_seq();
            self.transport.set_feature(&protocol::status_query(seq)).map_err(|e| format!("query failed: {e}"))?;
            sleep(self.pace / 4);
            if let Ok(reply) = self.transport.get_feature() {
                if let Some(s) = protocol::parse_status(&reply) {
                    return Ok(s);
                }
            }
            sleep(self.pace / 2);
        }
        Err("the mouse did not answer (it may be asleep: move it and try again)".into())
    }

    pub fn raw_exchange(&mut self, frame: &Frame) -> Result<Frame, String> {
        self.transport.set_feature(frame).map_err(|e| format!("write failed: {e}"))?;
        sleep(self.pace);
        self.transport.get_feature().map_err(|e| format!("read failed: {e}"))
    }
}
