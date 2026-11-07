//! Status query and reply.

use super::*;

/// Status query: the only command the mouse answers with data.
pub fn status_query(seq: u8) -> Frame {
    let mut f = frame(REG_STATUS, seq, &[]);
    f[5] = 0;
    f
}

#[derive(Debug, PartialEq, Eq)]
pub enum Charge {
    Discharging,
    Charging,
    Full,
}

#[derive(Debug)]
pub struct Status {
    pub model: String,
    pub variant: u8,
    pub battery: u8,
    pub charge: Charge,
    pub online: bool,
    /// Bytes 12..15 of the reply (meaning unknown).
    pub identity: [u8; 3],
}

/// Parse the reply to [`status_query`]. Returns `None` while the mouse has not filled it in yet.
pub fn parse_status(reply: &Frame) -> Option<Status> {
    if reply[2] != REG_STATUS || reply[3] != 0 || reply[6] == 0 {
        return None;
    }
    if &reply[8..12] != b"M263" {
        return None;
    }
    Some(Status {
        model: String::from_utf8_lossy(&reply[8..12]).into_owned(),
        variant: reply[15],
        battery: reply[18],
        charge: match reply[19] {
            1 => Charge::Charging,
            2 => Charge::Full,
            _ => Charge::Discharging,
        },
        online: reply[21] != 0,
        identity: [reply[12], reply[13], reply[14]],
    })
}
