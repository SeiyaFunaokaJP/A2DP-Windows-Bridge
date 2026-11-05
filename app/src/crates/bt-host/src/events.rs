//! Reading BTstack's events: the offsets of btstack_event.h's getters, with
//! bounds checks. Addresses come out in BTstack's order (most significant
//! byte first, as printed).

pub fn u8_at(p: &[u8], i: usize) -> Option<u8> {
    p.get(i).copied()
}

pub fn le16(p: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*p.get(i)?, *p.get(i + 1)?]))
}

pub fn le24(p: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*p.get(i)?, *p.get(i + 1)?, *p.get(i + 2)?, 0]))
}

/// A bd_addr_t stored little-endian in the event (reverse_bytes in BTstack)
pub fn addr_at(p: &[u8], i: usize) -> Option<[u8; 6]> {
    let b = p.get(i..i + 6)?;
    Some([b[5], b[4], b[3], b[2], b[1], b[0]])
}

pub fn event_type(p: &[u8]) -> Option<u8> {
    p.first().copied()
}

/// Subevent code of a meta event (A2DP / AVRCP / AVDTP)
pub fn subevent(p: &[u8]) -> Option<u8> {
    p.get(2).copied()
}

/// "XX:XX:XX:XX:XX:XX"
pub fn addr_str(a: &[u8; 6]) -> String {
    a.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":")
}

/// Parses "XX:XX:XX:XX:XX:XX" (or with '-')
pub fn parse_addr(s: &str) -> Option<[u8; 6]> {
    let parts: Vec<&str> = s.trim().split([':', '-']).collect();
    if parts.len() != 6 {
        return None;
    }
    let mut a = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        if part.len() != 2 {
            return None;
        }
        a[i] = u8::from_str_radix(part, 16).ok()?;
    }
    Some(a)
}

/// HCI status / reason codes A2DPWB reports in the log
pub fn hci_error_string(status: u8) -> &'static str {
    match status {
        0x00 => "Success",
        0x01 => "Unknown HCI Command",
        0x02 => "Unknown Connection Identifier",
        0x04 => "Page Timeout",
        0x05 => "Authentication Failure",
        0x06 => "PIN or Key Missing",
        0x07 => "Memory Capacity Exceeded",
        0x08 => "Connection Timeout",
        0x09 => "Connection Limit Exceeded",
        0x0B => "ACL Connection Already Exists",
        0x0C => "Command Disallowed",
        0x0D => "Connection Rejected (Limited Resources)",
        0x0E => "Connection Rejected (Security)",
        0x0F => "Connection Rejected (Unacceptable BD_ADDR)",
        0x10 => "Connection Accept Timeout Exceeded",
        0x11 => "Unsupported Feature or Parameter Value",
        0x12 => "Invalid HCI Command Parameters",
        0x13 => "Remote User Terminated Connection",
        0x14 => "Remote Device Terminated Connection due to Low Resources",
        0x15 => "Remote Device Terminated Connection due to Power Off",
        0x16 => "Connection Terminated By Local Host",
        0x17 => "Repeated Attempts",
        0x18 => "Pairing Not Allowed",
        0x1A => "Unsupported Remote Feature",
        0x1F => "Unspecified Error",
        0x22 => "LMP Response Timeout / LL Response Timeout",
        0x23 => "LMP Error Transaction Collision",
        0x24 => "LMP PDU Not Allowed",
        0x25 => "Encryption Mode Not Acceptable",
        0x29 => "Pairing With Unit Key Not Supported",
        0x2F => "Insufficient Security",
        0x3D => "Connection Terminated due to MIC Failure",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses() {
        let a = parse_addr("00:a2:D0:00:00:02").unwrap();
        assert_eq!(a, [0x00, 0xA2, 0xD0, 0x00, 0x00, 0x02]);
        assert_eq!(addr_str(&a), "00:A2:D0:00:00:02");
        assert_eq!(parse_addr("00:A2:D0:00:00"), None);
        let event = [0x03, 11, 0x00, 0x01, 0x00, 0x02, 0x00, 0x00, 0xD0, 0xA2, 0x00, 0x01, 0x00];
        assert_eq!(addr_at(&event, 5), Some(a));
    }
}
