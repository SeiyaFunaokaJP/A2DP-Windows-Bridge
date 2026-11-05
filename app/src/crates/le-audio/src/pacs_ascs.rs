//! PACS and ASCS values: PAC records, ASE characteristic values and the
//! ASE Control Point PDUs (v1 le_audio_device.h / le_audio_util.cpp, and the
//! PDUs le_audio_bap.cpp builds inline).

use crate::defs::{
    ASE_OP_CONFIG_CODEC, ASE_OP_CONFIG_QOS, AseState, CAP_CHANNEL_COUNTS, CAP_FRAME_DURATIONS, CAP_MAX_FRAMES_PER_SDU,
    CAP_OCTETS_PER_FRAME, CAP_SAMPLING_FREQUENCIES, CODING_FORMAT_LC3, META_CCID_LIST, META_PREFERRED_CONTEXTS,
    META_STREAMING_CONTEXTS,
};
use crate::ltv;

// ---- PACS ----

/// The LC3 Codec Specific Capabilities of a PAC record
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lc3Capabilities {
    /// SF_BIT_*
    pub sampling_frequencies: u16,
    /// FD_BIT_*
    pub frame_durations: u8,
    /// Bit n = n+1 channels; default 1 channel (0x01)
    pub channel_counts: u8,
    pub min_octets: u16,
    pub max_octets: u16,
    /// Default 1
    pub max_frames_per_sdu: u8,
}

impl Default for Lc3Capabilities {
    /// v1's defaults: 1 channel, 1 frame per SDU, nothing else supported
    fn default() -> Self {
        Lc3Capabilities {
            sampling_frequencies: 0,
            frame_durations: 0,
            channel_counts: 0x01,
            min_octets: 0,
            max_octets: 0,
            max_frames_per_sdu: 1,
        }
    }
}

/// One PAC record of a Sink PAC characteristic
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PacRecord {
    /// Coding format, company, vendor codec
    pub codec_id: [u8; 5],
    /// Coding format LC3
    pub lc3: bool,
    /// Valid when `lc3`
    pub caps: Lc3Capabilities,
    /// Raw Codec Specific Capabilities LTVs
    pub capabilities: Vec<u8>,
    /// Raw Metadata LTVs
    pub metadata: Vec<u8>,
    /// Metadata Preferred Audio Contexts (0x01), 0 if absent
    pub preferred_contexts: u16,
}

/// Parses a PAC characteristic value (Number_of_PAC_records + records);
/// None if it is malformed (v1 parse_pac_records)
pub fn parse_pac_records(data: &[u8]) -> Option<Vec<PacRecord>> {
    let (&count, _) = data.split_first()?;
    let len = data.len();
    let mut pos = 1usize;
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        if pos + 6 > len {
            tracing::debug!("PACS: PAC record header past the end at {pos} of {len}: {data:02X?}");
            return None;
        }
        let mut r = PacRecord::default();
        r.codec_id.copy_from_slice(&data[pos..pos + 5]);
        pos += 5;
        let cap_len = data[pos] as usize;
        pos += 1;
        if pos + cap_len + 1 > len {
            tracing::debug!("PACS: PAC capabilities of {cap_len} bytes past the end at {pos} of {len}: {data:02X?}");
            return None;
        }
        r.capabilities = data[pos..pos + cap_len].to_vec();
        pos += cap_len;
        let meta_len = data[pos] as usize;
        pos += 1;
        if pos + meta_len > len {
            tracing::debug!("PACS: PAC metadata of {meta_len} bytes past the end at {pos} of {len}: {data:02X?}");
            return None;
        }
        r.metadata = data[pos..pos + meta_len].to_vec();
        pos += meta_len;

        r.lc3 = r.codec_id[0] == CODING_FORMAT_LC3;
        if r.lc3 {
            let caps = &r.capabilities;
            if let Some(v) = ltv::find(caps, CAP_SAMPLING_FREQUENCIES).filter(|v| v.len() >= 2) {
                r.caps.sampling_frequencies = rd16(v);
            }
            if let Some(v) = ltv::find(caps, CAP_FRAME_DURATIONS).filter(|v| !v.is_empty()) {
                r.caps.frame_durations = v[0];
            }
            if let Some(v) = ltv::find(caps, CAP_CHANNEL_COUNTS).filter(|v| !v.is_empty()) {
                r.caps.channel_counts = v[0];
            }
            if let Some(v) = ltv::find(caps, CAP_OCTETS_PER_FRAME).filter(|v| v.len() >= 4) {
                r.caps.min_octets = rd16(v);
                r.caps.max_octets = rd16(&v[2..]);
            }
            if let Some(v) = ltv::find(caps, CAP_MAX_FRAMES_PER_SDU).filter(|v| !v.is_empty()) {
                r.caps.max_frames_per_sdu = v[0];
            }
        }
        if let Some(v) = ltv::find(&r.metadata, META_PREFERRED_CONTEXTS).filter(|v| v.len() >= 2) {
            r.preferred_contexts = rd16(v);
        }
        tracing::debug!(
            "PACS: record codec {:02X?}, LC3 {}, caps {:?}, preferred contexts 0x{:04X}, raw caps {:02X?}, metadata {:02X?}",
            r.codec_id,
            r.lc3,
            r.caps,
            r.preferred_contexts,
            r.capabilities,
            r.metadata
        );
        out.push(r);
    }
    Some(out)
}

/// Short summary of the sink PAC records for the log and bt-host's
/// `LeDeviceInfo::pacs` (v1 DeviceInfo::pacs)
pub fn describe_pac_records(records: &[PacRecord]) -> String {
    let parts: Vec<String> = records
        .iter()
        .map(|r| {
            if r.lc3 {
                let c = &r.caps;
                format!(
                    "LC3 rates 0x{:04x} durations 0x{:02x} channels 0x{:02x} octets {}-{} frames/SDU {}",
                    c.sampling_frequencies,
                    c.frame_durations,
                    c.channel_counts,
                    c.min_octets,
                    c.max_octets,
                    c.max_frames_per_sdu
                )
            } else {
                format!("codec 0x{:02x}", r.codec_id[0])
            }
        })
        .collect();
    if parts.is_empty() { "none".into() } else { parts.join("; ") }
}

// ---- ASCS ----

/// One ASE as the client knows it (v1 struct Ase)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ase {
    pub id: u8,
    /// Sink ASE (we send audio to it)
    pub sink: bool,
    pub value_handle: u16,
    pub state: AseState,
    // Codec Configured: the server's preferences
    pub pref_framing: u8,
    pub pref_phy: u8,
    pub pref_rtn: u8,
    pub pref_max_latency_ms: u16,
    pub pd_min_us: u32,
    pub pd_max_us: u32,
    pub pref_pd_min_us: u32,
    pub pref_pd_max_us: u32,
    // QoS Configured and later
    pub cig_id: u8,
    pub cis_id: u8,
    /// Codec Specific Configuration LTVs as configured
    pub codec_config: Vec<u8>,
}

impl Default for Ase {
    fn default() -> Self {
        Ase {
            id: 0,
            sink: true,
            value_handle: 0,
            state: AseState::Unknown,
            pref_framing: 0,
            pref_phy: 0,
            pref_rtn: 0,
            pref_max_latency_ms: 0,
            pd_min_us: 0,
            pd_max_us: 0,
            pref_pd_min_us: 0,
            pref_pd_max_us: 0,
            cig_id: 0,
            cis_id: 0,
            codec_config: Vec::new(),
        }
    }
}

/// Parses an ASE characteristic value into `ase` (id, state and the
/// parameters of that state); false if malformed (v1 parse_ase_value).
/// `sink` and `value_handle` are left as they are.
pub fn parse_ase_value(data: &[u8], ase: &mut Ase) -> bool {
    if data.len() < 2 {
        tracing::debug!("ASCS: ASE value too short: {data:02X?}");
        return false;
    }
    ase.id = data[0];
    ase.state = AseState::from_u8(data[1]);
    let p = &data[2..];
    let n = p.len();
    match ase.state {
        AseState::CodecConfigured => {
            if n < 23 {
                return false;
            }
            ase.pref_framing = p[0];
            ase.pref_phy = p[1];
            ase.pref_rtn = p[2];
            ase.pref_max_latency_ms = rd16(&p[3..]);
            ase.pd_min_us = rd24(&p[5..]);
            ase.pd_max_us = rd24(&p[8..]);
            ase.pref_pd_min_us = rd24(&p[11..]);
            ase.pref_pd_max_us = rd24(&p[14..]);
            // codec id p[17..22], Codec_Specific_Configuration length p[22]
            let cfg_len = p[22] as usize;
            if n < 23 + cfg_len {
                return false;
            }
            ase.codec_config = p[23..23 + cfg_len].to_vec();
        }
        AseState::QosConfigured => {
            if n < 15 {
                return false;
            }
            ase.cig_id = p[0];
            ase.cis_id = p[1];
        }
        AseState::Enabling | AseState::Streaming | AseState::Disabling => {
            if n < 3 {
                return false;
            }
            ase.cig_id = p[0];
            ase.cis_id = p[1];
        }
        AseState::Idle | AseState::Releasing => {}
        AseState::Unknown => {
            tracing::debug!("ASCS: ASE {} in unknown state 0x{:02X}", data[0], data[1]);
            return false;
        }
    }
    true
}

// ---- ASE Control Point PDUs (client to server) ----

/// One ASE of a Config Codec operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigCodec {
    pub ase_id: u8,
    /// TARGET_LATENCY_*
    pub target_latency: u8,
    /// PHY_* (Target_PHY)
    pub target_phy: u8,
    /// Coding format, company, vendor codec
    pub codec_id: [u8; 5],
    /// Codec Specific Configuration LTVs
    pub config: Vec<u8>,
}

/// One ASE of a Config QoS operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigQos {
    pub ase_id: u8,
    pub cig_id: u8,
    pub cis_id: u8,
    /// 24 bits
    pub sdu_interval_us: u32,
    pub framing: u8,
    pub phy: u8,
    pub max_sdu: u16,
    pub rtn: u8,
    pub max_transport_latency_ms: u16,
    /// 24 bits
    pub presentation_delay_us: u32,
}

/// Config Codec (opcode 0x01) for several ASEs of one server
pub fn cp_config_codec(ases: &[ConfigCodec]) -> Vec<u8> {
    let mut v = vec![ASE_OP_CONFIG_CODEC, ases.len() as u8];
    for a in ases {
        v.extend_from_slice(&[a.ase_id, a.target_latency, a.target_phy]);
        v.extend_from_slice(&a.codec_id);
        v.push(a.config.len() as u8);
        v.extend_from_slice(&a.config);
    }
    v
}

/// Config QoS (opcode 0x02) for several ASEs of one server
pub fn cp_config_qos(ases: &[ConfigQos]) -> Vec<u8> {
    let mut v = vec![ASE_OP_CONFIG_QOS, ases.len() as u8];
    for a in ases {
        v.extend_from_slice(&[a.ase_id, a.cig_id, a.cis_id]);
        put24(&mut v, a.sdu_interval_us);
        v.extend_from_slice(&[a.framing, a.phy]);
        v.extend_from_slice(&a.max_sdu.to_le_bytes());
        v.push(a.rtn);
        v.extend_from_slice(&a.max_transport_latency_ms.to_le_bytes());
        put24(&mut v, a.presentation_delay_us);
    }
    v
}

/// Enable (0x03) or Update Metadata (0x07): (ASE id, Metadata LTVs) each
pub fn cp_with_metadata(opcode: u8, ases: &[(u8, Vec<u8>)]) -> Vec<u8> {
    let mut v = vec![opcode, ases.len() as u8];
    for (id, meta) in ases {
        v.push(*id);
        v.push(meta.len() as u8);
        v.extend_from_slice(meta);
    }
    v
}

/// Receiver Start Ready, Disable, Receiver Stop Ready or Release: the
/// opcode and the ASE ids
pub fn cp_ase_ids(opcode: u8, ase_ids: &[u8]) -> Vec<u8> {
    let mut v = vec![opcode, ase_ids.len() as u8];
    v.extend_from_slice(ase_ids);
    v
}

/// The Metadata of Enable: Streaming Audio Contexts (0x02) and, when
/// `ccids` is not empty, the CCID List (0x05) without duplicates
pub fn enable_metadata(contexts: u16, ccids: &[u8]) -> Vec<u8> {
    let mut ids = ccids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    let mut meta = Vec::new();
    ltv::put(&mut meta, META_STREAMING_CONTEXTS, &contexts.to_le_bytes());
    if !ids.is_empty() {
        ltv::put(&mut meta, META_CCID_LIST, &ids);
    }
    meta
}

/// One ASE's entry of an ASE Control Point notification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpResponseEntry {
    pub ase_id: u8,
    /// 0x00 success, else the Response_Code
    pub code: u8,
    pub reason: u8,
}

/// An ASE Control Point notification
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpResponse {
    pub opcode: u8,
    pub entries: Vec<CpResponseEntry>,
}

/// Parses an ASE Control Point notification; None if shorter than its
/// header. Number_of_ASEs 0xFF (the server refused the whole PDU: invalid
/// length or unsupported opcode) gives one entry with ASE id 0x00 (none if
/// the PDU ends before it); otherwise the entries that fit (v1 read as many
/// as there were)
pub fn parse_cp_response(data: &[u8]) -> Option<CpResponse> {
    if data.len() < 2 {
        return None;
    }
    let (opcode, count) = (data[0], data[1]);
    let mut entries = Vec::new();
    if count == 0xFF {
        // The whole PDU was refused: one entry, ASE id 0x00
        if data.len() >= 5 {
            entries.push(CpResponseEntry { ase_id: data[2], code: data[3], reason: data[4] });
        }
    } else {
        // As many entries as there are (v1 read what fitted)
        for e in data[2..].as_chunks::<3>().0.iter().take(count as usize) {
            entries.push(CpResponseEntry { ase_id: e[0], code: e[1], reason: e[2] });
        }
    }
    Some(CpResponse { opcode, entries })
}

fn rd16(p: &[u8]) -> u16 {
    u16::from_le_bytes([p[0], p[1]])
}

fn rd24(p: &[u8]) -> u32 {
    u32::from_le_bytes([p[0], p[1], p[2], 0])
}

fn put24(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes()[..3]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::*;

    /// The PAC record of the emulator's sinks (tools/emu/le_audio/bap.py):
    /// LC3 16/24/32/48 kHz, 7.5 + 10 ms, `channels`, 26-155 octets, 1 frame
    pub(crate) fn emu_pac(channels: u8) -> Vec<u8> {
        let mut caps = Vec::new();
        ltv::put(
            &mut caps,
            CAP_SAMPLING_FREQUENCIES,
            &(SF_BIT_16000 | SF_BIT_24000 | SF_BIT_32000 | SF_BIT_48000).to_le_bytes(),
        );
        ltv::put(&mut caps, CAP_FRAME_DURATIONS, &[FD_BIT_7500US | FD_BIT_10000US]);
        ltv::put(&mut caps, CAP_CHANNEL_COUNTS, &[channels]);
        ltv::put(&mut caps, CAP_OCTETS_PER_FRAME, &[26, 0, 155, 0]);
        ltv::put(&mut caps, CAP_MAX_FRAMES_PER_SDU, &[1]);
        let mut v = vec![1, CODING_FORMAT_LC3, 0, 0, 0, 0, caps.len() as u8];
        v.extend_from_slice(&caps);
        v.push(4);
        ltv::put(&mut v, META_PREFERRED_CONTEXTS, &CONTEXT_MEDIA.to_le_bytes());
        v
    }

    #[test]
    fn pac_records() {
        let recs = parse_pac_records(&emu_pac(0x03)).unwrap();
        assert_eq!(recs.len(), 1);
        let r = &recs[0];
        assert!(r.lc3);
        assert_eq!(r.caps.sampling_frequencies, 0xB4);
        assert_eq!(r.caps.frame_durations, 0x03);
        assert_eq!(r.caps.channel_counts, 0x03);
        assert_eq!((r.caps.min_octets, r.caps.max_octets), (26, 155));
        assert_eq!(r.caps.max_frames_per_sdu, 1);
        assert_eq!(r.preferred_contexts, CONTEXT_MEDIA);
        assert_eq!(
            describe_pac_records(&recs),
            "LC3 rates 0x00b4 durations 0x03 channels 0x03 octets 26-155 frames/SDU 1"
        );

        // A vendor codec record without capabilities, defaults kept
        let v = [2, 0xFF, 0x5D, 0x00, 0x01, 0x00, 0, 0, CODING_FORMAT_LC3, 0, 0, 0, 0, 0, 0];
        let recs = parse_pac_records(&v).unwrap();
        assert!(!recs[0].lc3);
        assert!(recs[1].lc3);
        assert_eq!(recs[1].caps, Lc3Capabilities::default());
        assert_eq!(
            describe_pac_records(&recs),
            "codec 0xff; LC3 rates 0x0000 durations 0x00 channels 0x01 octets 0-0 frames/SDU 1"
        );
        assert_eq!(describe_pac_records(&[]), "none");

        // Malformed
        assert_eq!(parse_pac_records(&[]), None);
        assert_eq!(parse_pac_records(&[1, 6, 0, 0, 0, 0]), None);
        assert_eq!(parse_pac_records(&[1, 6, 0, 0, 0, 0, 3, 1, 2]), None);
        assert_eq!(parse_pac_records(&[1, 6, 0, 0, 0, 0, 0, 2, 1]), None);
        assert_eq!(parse_pac_records(&[0]), Some(vec![]));
    }

    #[test]
    fn ase_values() {
        let mut ase = Ase { sink: false, value_handle: 0x42, ..Ase::default() };
        // Codec Configured: framing 0, PHY 2M, RTN 13, 100 ms, PD 20-40 ms,
        // preferred 20-40 ms, LC3, 3 octets of configuration
        let mut v = vec![3, 0x01, 0x00, 0x02, 13, 100, 0];
        v.extend_from_slice(&[0x20, 0x4E, 0x00, 0x40, 0x9C, 0x00, 0x20, 0x4E, 0x00, 0x40, 0x9C, 0x00]);
        v.extend_from_slice(&[CODING_FORMAT_LC3, 0, 0, 0, 0, 3, 0x02, 0x01, 0x08]);
        assert!(parse_ase_value(&v, &mut ase));
        assert_eq!((ase.id, ase.state), (3, AseState::CodecConfigured));
        assert_eq!((ase.pref_framing, ase.pref_phy, ase.pref_rtn, ase.pref_max_latency_ms), (0, 2, 13, 100));
        assert_eq!((ase.pd_min_us, ase.pd_max_us), (20000, 40000));
        assert_eq!((ase.pref_pd_min_us, ase.pref_pd_max_us), (20000, 40000));
        assert_eq!(ase.codec_config, [0x02, 0x01, 0x08]);
        assert_eq!((ase.sink, ase.value_handle), (false, 0x42));
        // Configuration longer than the value
        v.pop();
        assert!(!parse_ase_value(&v, &mut ase));

        let mut qos = vec![3, 0x02, 1, 0];
        qos.extend_from_slice(&[0; 13]);
        assert!(parse_ase_value(&qos, &mut ase));
        assert_eq!((ase.state, ase.cig_id, ase.cis_id), (AseState::QosConfigured, 1, 0));
        assert!(!parse_ase_value(&qos[..16], &mut ase));

        assert!(parse_ase_value(&[3, 0x04, 1, 2, 0], &mut ase));
        assert_eq!((ase.state, ase.cig_id, ase.cis_id), (AseState::Streaming, 1, 2));
        assert!(!parse_ase_value(&[3, 0x03, 1, 2], &mut ase));
        assert!(parse_ase_value(&[3, 0x00], &mut ase));
        assert_eq!(ase.state, AseState::Idle);
        assert!(parse_ase_value(&[3, 0x06], &mut ase));
        assert!(!parse_ase_value(&[3, 0x09], &mut ase));
        assert_eq!(ase.state, AseState::Unknown);
        assert!(!parse_ase_value(&[3], &mut ase));
    }

    #[test]
    fn control_point_pdus() {
        let cfg = vec![0x02, 0x01, 0x08];
        let pdu = cp_config_codec(&[ConfigCodec {
            ase_id: 1,
            target_latency: TARGET_LATENCY_HIGH_RELIABILITY,
            target_phy: PHY_2M,
            codec_id: [CODING_FORMAT_LC3, 0, 0, 0, 0],
            config: cfg,
        }]);
        assert_eq!(pdu, [0x01, 1, 1, 0x03, 0x02, 0x06, 0, 0, 0, 0, 3, 0x02, 0x01, 0x08]);

        let qos = ConfigQos {
            ase_id: 2,
            cig_id: 1,
            cis_id: 1,
            sdu_interval_us: 7500,
            framing: 0,
            phy: PHY_2M,
            max_sdu: 90,
            rtn: 1,
            max_transport_latency_ms: 15,
            presentation_delay_us: 10000,
        };
        assert_eq!(
            cp_config_qos(&[qos]),
            [0x02, 1, 2, 1, 1, 0x4C, 0x1D, 0x00, 0, 0x02, 90, 0, 1, 15, 0, 0x10, 0x27, 0x00]
        );

        let meta = enable_metadata(CONTEXT_MEDIA, &[1, 1]);
        assert_eq!(meta, [0x03, 0x02, 0x04, 0x00, 0x02, 0x05, 0x01]);
        assert_eq!(
            enable_metadata(CONTEXT_MEDIA | CONTEXT_CONVERSATIONAL, &[2, 1]),
            [0x03, 0x02, 0x06, 0x00, 0x03, 0x05, 0x01, 0x02]
        );
        assert_eq!(enable_metadata(CONTEXT_GAME, &[]), [0x03, 0x02, 0x08, 0x00]);
        assert_eq!(
            cp_with_metadata(ASE_OP_ENABLE, &[(1, meta.clone()), (2, meta)]),
            [0x03, 2, 1, 7, 3, 2, 4, 0, 2, 5, 1, 2, 7, 3, 2, 4, 0, 2, 5, 1]
        );
        assert_eq!(cp_ase_ids(ASE_OP_RELEASE, &[1, 2]), [0x08, 2, 1, 2]);
        assert_eq!(cp_ase_ids(ASE_OP_DISABLE, &[]), [0x05, 0]);
    }

    #[test]
    fn control_point_responses() {
        let r = parse_cp_response(&[0x01, 2, 1, 0, 0, 2, 0x03, 0x04]).unwrap();
        assert_eq!(r.opcode, 0x01);
        assert_eq!(
            r.entries,
            [CpResponseEntry { ase_id: 1, code: 0, reason: 0 }, CpResponseEntry { ase_id: 2, code: 3, reason: 4 }]
        );
        // Truncated: what fits
        let r = parse_cp_response(&[0x02, 2, 1, 0, 0, 2]).unwrap();
        assert_eq!(r.entries.len(), 1);
        // Number_of_ASEs 0xFF: the whole PDU
        let r = parse_cp_response(&[0x09, 0xFF, 0, 0x01, 0]).unwrap();
        assert_eq!(r.entries, [CpResponseEntry { ase_id: 0, code: 1, reason: 0 }]);
        assert_eq!(parse_cp_response(&[0x09, 0xFF]).unwrap().entries, []);
        assert_eq!(parse_cp_response(&[0x01]), None);
    }
}
