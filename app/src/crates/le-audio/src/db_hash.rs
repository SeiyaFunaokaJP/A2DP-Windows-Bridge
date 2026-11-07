//! The GATT Database Hash of the PC's GATT server (Core Vol 3 Part G
//! 7.3; v1 GattServer::compute_db_hash in le_audio_gatt_server.cpp).
//!
//! AES-CMAC with a zero key over, per attribute in handle order: for
//! service, include and characteristic declarations and Characteristic
//! Extended Properties the handle, the type and the value; for the other
//! descriptors (User Description, CCC, SCC, Presentation / Aggregate
//! Format) the handle and the type; nothing for characteristic values (Core
//! Vol 3 Part G 7.3.1, BTstack's att_db_util_hash_*). v1 took that message
//! from BTstack's att_db_util (att_db_util_hash_*: [`hash_input_from_att_db`]
//! here does the same on the blob) and patched the hash into the Database
//! Hash value in the att_db blob ([`patch_att_db`]). A stored hash that
//! differs from the current one drops the CCC values stored for bonded
//! clients and makes a Service Changed indication due (bt-host gatt_server).

use crate::csip_crypto::aes_cmac;

/// An attribute type: a 16-bit UUID or a 128-bit one (little-endian, as on
/// the air)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttUuid {
    Uuid16(u16),
    Uuid128([u8; 16]),
}

/// One attribute of the database, as the hash sees it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashAttr {
    pub handle: u16,
    pub uuid: AttUuid,
    /// The value for declarations and Characteristic Extended Properties;
    /// ignored for other descriptors and characteristic values
    pub value: Vec<u8>,
}

/// «Primary Service», «Secondary Service», «Included Service»,
/// «Characteristic», «Characteristic Extended Properties»: handle, type and
/// value
fn hashed_with_value(uuid16: u16) -> bool {
    matches!(uuid16, 0x2800 | 0x2801 | 0x2802 | 0x2803 | 0x2900)
}

/// «Characteristic User Description», «Client Characteristic
/// Configuration», «Server Characteristic Configuration», «Characteristic
/// Presentation Format», «Characteristic Aggregate Format»: handle and type
fn hashed_without_value(uuid16: u16) -> bool {
    matches!(uuid16, 0x2901..=0x2905)
}

/// The hash input of Core Vol 3 Part G 7.3.1 from the attributes (in
/// handle order)
pub fn hash_input(attrs: &[HashAttr]) -> Vec<u8> {
    let mut out = Vec::new();
    for a in attrs {
        let AttUuid::Uuid16(t) = a.uuid else { continue };
        if hashed_with_value(t) || hashed_without_value(t) {
            out.extend_from_slice(&a.handle.to_le_bytes());
            out.extend_from_slice(&t.to_le_bytes());
            if hashed_with_value(t) {
                out.extend_from_slice(&a.value);
            }
        }
    }
    out
}

/// BTstack's ATT_PROPERTY_UUID128 flag of an att_db entry
const FLAG_UUID128: u16 = 0x200;

/// The entries of a BTstack att_db blob: (offset of the entry, size, flags,
/// handle); None if the blob is malformed
fn att_db_entries(db: &[u8]) -> Option<Vec<(usize, usize, u16, u16)>> {
    let rd16 = |p: usize| -> Option<u16> { Some(u16::from_le_bytes([*db.get(p)?, *db.get(p + 1)?])) };
    let mut out = Vec::new();
    let mut pos = 1usize;
    loop {
        let size = rd16(pos)? as usize;
        if size == 0 {
            return Some(out);
        }
        let flags = rd16(pos + 2)?;
        let uuid_len = if flags & FLAG_UUID128 != 0 { 16 } else { 2 };
        if size < 6 + uuid_len || pos + size > db.len() {
            return None;
        }
        out.push((pos, size, flags, rd16(pos + 4)?));
        pos += size;
    }
}

/// The hash input taken from a BTstack att_db blob (version octet, then
/// per attribute: size, flags, handle, UUID, value; size 0 ends) as
/// BTstack's att_db_util_hash_get_next() reads it; None if malformed
pub fn hash_input_from_att_db(db: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    for (pos, size, flags, _) in att_db_entries(db)? {
        if flags & FLAG_UUID128 != 0 {
            continue;
        }
        let uuid16 = u16::from_le_bytes([db[pos + 6], db[pos + 7]]);
        if hashed_with_value(uuid16) {
            out.extend_from_slice(&db[pos + 4..pos + size]);
        } else if hashed_without_value(uuid16) {
            out.extend_from_slice(&db[pos + 4..pos + 8]);
        }
    }
    Some(out)
}

/// AES-CMAC(zero key, input), least significant octet first as the
/// Database Hash characteristic sends it (and BTstack's compile_gatt.py
/// stores it)
pub fn database_hash(input: &[u8]) -> [u8; 16] {
    let mut mac = aes_cmac(&[0; 16], input);
    mac.reverse();
    mac
}

/// Writes `hash` into the value of the attribute with `value_handle` in a
/// BTstack att_db blob (version octet, then per attribute: size, flags,
/// handle, UUID, value; size 0 ends). False if the attribute is not there
/// or its value is not 16 octets.
pub fn patch_att_db(db: &mut [u8], value_handle: u16, hash: &[u8; 16]) -> bool {
    let Some(entries) = att_db_entries(db) else { return false };
    let Some(&(pos, size, flags, _)) = entries.iter().find(|e| e.3 == value_handle) else { return false };
    let uuid_len = if flags & FLAG_UUID128 != 0 { 16 } else { 2 };
    if size != 6 + uuid_len + 16 {
        return false;
    }
    db[pos + 6 + uuid_len..pos + size].copy_from_slice(hash);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: u8 = 0x02;
    const WWR: u8 = 0x04;
    const W: u8 = 0x08;
    const N: u8 = 0x10;
    const I: u8 = 0x20;

    /// Builds the blob as BTstack's att_db_util does (uuid16 attributes,
    /// characteristics with a CCC when they notify / indicate) and the
    /// same database as HashAttrs
    struct Db {
        blob: Vec<u8>,
        attrs: Vec<HashAttr>,
        next: u16,
    }

    impl Db {
        fn new() -> Db {
            Db { blob: vec![1, 0, 0], attrs: Vec::new(), next: 1 }
        }
        fn attr(&mut self, flags: u16, uuid: AttUuid, value: &[u8]) -> u16 {
            let h = self.next;
            self.next += 1;
            self.blob.truncate(self.blob.len() - 2);
            let uuid_bytes = match uuid {
                AttUuid::Uuid16(u) => u.to_le_bytes().to_vec(),
                AttUuid::Uuid128(u) => u.to_vec(),
            };
            let flags = flags | if uuid_bytes.len() == 16 { FLAG_UUID128 } else { 0 };
            let size = (6 + uuid_bytes.len() + value.len()) as u16;
            self.blob.extend_from_slice(&size.to_le_bytes());
            self.blob.extend_from_slice(&flags.to_le_bytes());
            self.blob.extend_from_slice(&h.to_le_bytes());
            self.blob.extend_from_slice(&uuid_bytes);
            self.blob.extend_from_slice(value);
            self.blob.extend_from_slice(&[0, 0]);
            self.attrs.push(HashAttr { handle: h, uuid, value: value.to_vec() });
            h
        }
        fn service(&mut self, uuid16: u16) {
            self.attr(0x02, AttUuid::Uuid16(0x2800), &uuid16.to_le_bytes());
        }
        /// v1 GattServer::add_characteristic (dynamic value)
        fn chr(&mut self, uuid: AttUuid, props: u8) -> u16 {
            self.chr_value(uuid, props, 0x100, &[])
        }
        fn chr_value(&mut self, uuid: AttUuid, props: u8, flags: u16, value: &[u8]) -> u16 {
            let mut decl = vec![props];
            decl.extend_from_slice(&(self.next + 1).to_le_bytes());
            match uuid {
                AttUuid::Uuid16(u) => decl.extend_from_slice(&u.to_le_bytes()),
                AttUuid::Uuid128(u) => decl.extend_from_slice(&u),
            }
            self.attr(0x02, AttUuid::Uuid16(0x2803), &decl);
            let h = self.attr(flags | props as u16, uuid, value);
            if props & (N | I) != 0 {
                self.attr(0x10e, AttUuid::Uuid16(0x2902), &[0, 0]);
            }
            h
        }
    }

    /// v1's GATT database (le_audio_gatt_server.cpp build_base(), then the
    /// modules' add_gatt_services() in the order of le_audio_modules.cpp:
    /// MCP's GMCS, CCP's GTBS, TMAP/GMAP's TMAS and GMAS): the one the
    /// emulator case mcp-read checks. Returns it and the Database Hash value
    /// handle.
    fn v1_db() -> (Db, u16) {
        let u = AttUuid::Uuid16;
        let mut db = Db::new();
        db.service(0x1800);
        db.chr(u(0x2A00), R);
        db.chr(u(0x2A01), R);
        db.service(0x1801);
        db.chr(u(0x2A05), I);
        let hash = db.chr_value(u(0x2B2A), R, 0, &[0; 16]);
        db.service(0x1849);
        for (c, p) in [
            (0x2B93, R | N),
            (0x2B96, N),
            (0x2B97, R | N),
            (0x2B98, R | N),
            (0x2B99, R | W | WWR | N),
            (0x2B9A, R | N),
            (0x2B9B, R | N),
            (0x2BA3, R | N),
            (0x2BA4, W | WWR | N),
            (0x2BA5, R | N),
            (0x2BBA, R),
        ] {
            db.chr(u(c), p);
        }
        db.service(0x184C);
        for (c, p) in [
            (0x2BB3, R | N),
            (0x2BB4, R),
            (0x2BB5, R | N),
            (0x2BB6, R | N),
            (0x2BB7, R | N),
            (0x2BB8, R | W | WWR),
            (0x2BB9, R | N),
            (0x2BBA, R),
            (0x2BBB, R | N),
            (0x2BBC, R | N),
            (0x2BBD, R | N),
            (0x2BBE, W | WWR | N),
            (0x2BBF, R),
            (0x2BC0, N),
            (0x2BC1, R | N),
            (0x2BC2, R | N),
        ] {
            db.chr(u(c), p);
        }
        db.service(0x1855);
        db.chr(u(0x2B51), R);
        db.service(0x1858);
        db.chr(u(0x2C00), R);
        db.chr(u(0x2C01), R);
        (db, hash)
    }

    #[test]
    fn v1_database_hash() {
        let (mut db, hash_handle) = v1_db();
        assert_eq!((db.attrs.len(), hash_handle), (97, 11));
        let input = hash_input(&db.attrs);
        assert_eq!(input.len(), 434);
        assert_eq!(hash_input_from_att_db(&db.blob).unwrap(), input);
        // Computed independently with Python (cryptography's AES-CMAC over
        // the message tools/emu/le_audio/mcp.py check_db_hash builds from
        // the same attributes)
        let hash = database_hash(&input);
        assert_eq!(hash, *b"\x6b\x40\x8a\x84\x7f\xa4\xfc\x0f\x97\x54\x01\x9e\x98\x6b\xc1\xd4");
        // Into the blob; the value is not hashed, so the hash stays
        assert!(patch_att_db(&mut db.blob, hash_handle, &hash));
        let at = db.blob.windows(16).position(|w| w == hash).unwrap();
        assert_eq!(db.blob[at - 4..at], [11, 0, 0x2A, 0x2B]);
        assert_eq!(database_hash(&hash_input_from_att_db(&db.blob).unwrap()), hash);
        // Not a 16-octet value / no such handle
        assert!(!patch_att_db(&mut db.blob, 3, &hash));
        assert!(!patch_att_db(&mut db.blob, 200, &hash));
    }

    #[test]
    fn input_layout() {
        let attrs = [
            HashAttr { handle: 1, uuid: AttUuid::Uuid16(0x2800), value: vec![0x00, 0x18] },
            HashAttr { handle: 2, uuid: AttUuid::Uuid16(0x2803), value: vec![0x02, 0x03, 0x00, 0x00, 0x2A] },
            // a value: not hashed
            HashAttr { handle: 3, uuid: AttUuid::Uuid16(0x2A00), value: b"BlueCaster".to_vec() },
            HashAttr { handle: 4, uuid: AttUuid::Uuid16(0x2902), value: vec![1, 0] },
            HashAttr { handle: 5, uuid: AttUuid::Uuid16(0x2900), value: vec![1, 0] },
            HashAttr { handle: 6, uuid: AttUuid::Uuid128([7; 16]), value: vec![1] },
            HashAttr { handle: 7, uuid: AttUuid::Uuid16(0x2905), value: vec![1, 2] },
        ];
        assert_eq!(
            hash_input(&attrs),
            [
                1, 0, 0x00, 0x28, 0x00, 0x18, //
                2, 0, 0x03, 0x28, 0x02, 0x03, 0x00, 0x00, 0x2A, //
                4, 0, 0x02, 0x29, //
                5, 0, 0x00, 0x29, 1, 0, //
                7, 0, 0x05, 0x29,
            ]
        );
    }

    #[test]
    fn uuid128_attributes_are_skipped() {
        let mut db = Db::new();
        db.service(0x1800);
        let v = db.chr(AttUuid::Uuid128([0x11; 16]), R | N);
        let plain = hash_input(&db.attrs);
        assert_eq!(hash_input_from_att_db(&db.blob).unwrap(), plain);
        // declaration (with the 128-bit UUID in its value) and CCC hashed,
        // the 128-bit value not
        assert_eq!(plain.len(), 6 + (4 + 19) + 4);
        assert!(!patch_att_db(&mut db.blob, v, &[0; 16]));
    }

    #[test]
    fn database_hash_is_the_reversed_cmac() {
        // The empty message under the zero key: AES-CMAC
        // 4387c14b46ef7e176dceefa862d72ff9 (Python's cryptography)
        let mut want = *b"\x43\x87\xc1\x4b\x46\xef\x7e\x17\x6d\xce\xef\xa8\x62\xd7\x2f\xf9";
        assert_eq!(aes_cmac(&[0; 16], &[]), want);
        want.reverse();
        assert_eq!(database_hash(&[]), want);
    }

    #[test]
    fn malformed_blobs() {
        assert_eq!(hash_input_from_att_db(&[1]), None);
        assert_eq!(hash_input_from_att_db(&[1, 0, 0]), Some(vec![]));
        // size beyond the blob
        assert_eq!(hash_input_from_att_db(&[1, 20, 0, 2, 0, 1, 0, 0, 0x28, 0, 0]), None);
        assert!(!patch_att_db(&mut [1, 20, 0, 2, 0], 1, &[0; 16]));
    }
}
