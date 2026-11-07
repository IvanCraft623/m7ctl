//! An in-memory mouse for tests: it records every frame written and acknowledges it like a real one.

use super::Transport;
use crate::protocol::{Frame, REPORT_ID, REPORT_LEN};
use std::io;
use std::sync::{Arc, Mutex};

/// Everything the fake saw, shared with the test that created it.
#[derive(Default)]
pub struct Log {
    pub frames: Vec<Frame>,
}

pub struct FakeTransport {
    log: Arc<Mutex<Log>>,
    last: Frame,
    asleep: bool,
    status: Frame,
}

impl FakeTransport {
    /// A fake mouse plus a handle to inspect what was sent to it.
    pub fn new() -> (FakeTransport, Arc<Mutex<Log>>) {
        let log = Arc::new(Mutex::new(Log::default()));
        (FakeTransport::logging_to(log.clone()), log)
    }

    /// A fake mouse that records into an existing log (several commands, one log).
    pub fn logging_to(log: Arc<Mutex<Log>>) -> FakeTransport {
        let mut status = [0u8; REPORT_LEN];
        status[0] = REPORT_ID;
        status[1] = 1;
        status[2] = 0x20;
        status[6] = 0x10;
        status[8..12].copy_from_slice(b"M263");
        status[15] = 0x95;
        status[18] = 80;
        status[19] = 0;
        status[21] = 1;
        FakeTransport { log, last: [0; REPORT_LEN], asleep: false, status }
    }

    /// Make the fake behave like a sleeping wireless mouse (echo byte 3 = 1).
    pub fn asleep(mut self, asleep: bool) -> FakeTransport {
        self.asleep = asleep;
        self
    }
}

impl Transport for FakeTransport {
    fn set_feature(&mut self, frame: &Frame) -> io::Result<()> {
        self.last = *frame;
        self.log.lock().unwrap().frames.push(*frame);
        Ok(())
    }

    fn get_feature(&mut self) -> io::Result<Frame> {
        if self.last[2] == 0x20 {
            let mut s = self.status;
            s[4] = self.last[4];
            return Ok(s);
        }
        let mut echo = self.last;
        echo[3] = u8::from(self.asleep);
        Ok(echo)
    }

    fn read_report(&mut self) -> io::Result<Vec<u8>> {
        Err(io::Error::new(io::ErrorKind::UnexpectedEof, "the fake mouse sends no reports"))
    }

    fn path(&self) -> &str {
        "/dev/fake"
    }

    fn pid(&self) -> u16 {
        super::PID_WIRED
    }
}
