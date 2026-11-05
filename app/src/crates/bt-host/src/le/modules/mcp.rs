//! MCP: the Media Control Server (GMCS) on the PC (v1 le_audio_mcp.cpp;
//! WP5).
//!
//! The PC is the Media Control Server; the headset (Media Control Client)
//! reads the player state from the Generic Media Control Service and sends
//! play / pause / stop / next / previous / move relative through the Media
//! Control Point.
//!
//! GMCS (0x1849, all characteristics need an encrypted link):
//! - Media Player Name (0x2B93), read, notify: the Windows player
//!   ("Windows" without one)
//! - Track Changed (0x2B96), notify: new track
//! - Track Title (0x2B97), read, notify: UTF-8, "" when unknown
//! - Track Duration (0x2B98), read, notify: int32 0.01 s, -1 unknown
//! - Track Position (0x2B99), read, write, write without response, notify:
//!   a write seeks
//! - Playback Speed (0x2B9A), read, notify: 64 * log2(rate)
//! - Seeking Speed (0x2B9B), read, notify: always 0 (no seeking state)
//! - Media State (0x2BA3), read, notify: inactive / playing / paused
//! - Media Control Point (0x2BA4), write, write without response, notify:
//!   (opcode, result)
//! - Opcodes Supported (0x2BA5), read, notify: Play, Pause, Stop, Previous /
//!   Next Track, + Move Relative if seekable
//! - Content Control ID (0x2BBA), read: `CCID_GMCS`, also in the CCID List
//!   of a media stream's Enable metadata (`add_ccids`)
//!
//! Long values: the GATT server serves reads with offsets (Read Blob) from
//! the whole value; a long read of the player name or track title whose
//! value changes between its parts fails with "Value Changed During Read
//! Long" (ATT 0x80, an MCS application error). Notifications are cut to
//! ATT_MTU - 3, text values at a UTF-8 character boundary.
//!
//! The Windows side is `LeModuleSetup::media` (a
//! `le_audio::mcp_model::MediaBackend`, core's SMTC backend). Commands are
//! refused (Command Cannot Be Completed) while the "media keys" setting is
//! off (`ctx.media_keys()`), as AVRCP buttons are ignored then. Without a
//! backend (`--mcp-dry-run`, implied by `--hci-tcp`) the simulated player
//! answers and commands are only logged ("MCP: GMCS on the PC (CCID 1), dry
//! run", "MCP: dry run: <OP>"). Backend changes and command results reach
//! the BTstack thread through the setup's poster, so the result
//! notification goes out after the ATT write response (v1 reply_later).

use std::collections::HashMap;
use std::sync::Arc;

use btstack_sys::consts::ATT_ERROR_INVALID_ATTRIBUTE_VALUE_LENGTH;
use le_audio::defs::{CCID_GMCS, CONTEXT_CONVERSATIONAL, UUID_CHR_CCID, UUID_SERVICE_GMCS};
use le_audio::mcp_model::{
    ATT_ERROR_VALUE_CHANGED_DURING_READ_LONG, MAX_TEXT, MediaBackend, MediaInfo, RESULT_COMMAND_CANNOT_BE_COMPLETED,
    SimulatedPlayer, UUID_CHR_MEDIA_CONTROL_POINT, UUID_CHR_MEDIA_PLAYER_NAME, UUID_CHR_MEDIA_STATE,
    UUID_CHR_OPCODES_SUPPORTED, UUID_CHR_PLAYBACK_SPEED, UUID_CHR_SEEKING_SPEED, UUID_CHR_TRACK_CHANGED,
    UUID_CHR_TRACK_DURATION, UUID_CHR_TRACK_POSITION, UUID_CHR_TRACK_TITLE, control_point_check, cut_text, now_ms,
    opcode_name, result_name, seek_target, text_value,
};

use super::super::LeDeviceId;
use super::super::core::{CON_HANDLE_INVALID, LeCtx, LePoster};
use super::super::gatt_server::{
    ATT_PROPERTY_NOTIFY, ATT_PROPERTY_READ, ATT_PROPERTY_WRITE, ATT_PROPERTY_WRITE_WITHOUT_RESPONSE,
    ATT_SECURITY_ENCRYPTED, AttRead, AttUuid, AttWrite, GattServer, GattServerBuilder,
};
use super::super::module::{
    ATT_ERROR_READ_NOT_PERMITTED, ATT_ERROR_WRITE_NOT_PERMITTED, LeModule, LeModuleEntry, LeModuleSetup,
};

/// Sends a control point result (con, opcode, result) in a later run loop
/// turn; any thread
type ReplyFn = Arc<dyn Fn(u16, u8, u8) + Send + Sync>;

/// Value handles of the GMCS characteristics (0: not added)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Handles {
    name: u16,
    track_changed: u16,
    title: u16,
    duration: u16,
    position: u16,
    speed: u16,
    seeking_speed: u16,
    state: u16,
    cp: u16,
    opcodes: u16,
    ccid: u16,
}

/// A notification of [`McpModule::publish_values`]: value handle, value,
/// text (cut at a UTF-8 character boundary)
type Notification = (u16, Vec<u8>, bool);

/// BTstack-thread part (no worker hooks)
pub struct McpModule {
    backend: Box<dyn MediaBackend>,
    dry_run: bool,
    poster: LePoster,
    /// Control point results (through the poster)
    reply_later: ReplyFn,
    h: Handles,
    /// What clients were told
    last: MediaInfo,
    /// Long reads: the value given at offset 0, per (con, value handle)
    long_reads: HashMap<(u16, u16), Vec<u8>>,
}

fn le32(v: i32) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

impl McpModule {
    fn new(backend: Box<dyn MediaBackend>, poster: LePoster) -> McpModule {
        let p = poster.clone();
        let reply_later: ReplyFn = Arc::new(move |con, op, result| {
            p.post(move |ctx| reply(ctx, con, op, result));
        });
        McpModule {
            dry_run: backend.dry_run(),
            backend,
            poster,
            reply_later,
            h: Handles::default(),
            last: MediaInfo::default(),
            long_reads: HashMap::new(),
        }
    }

    /// The value of a readable characteristic (v1's read callbacks)
    fn read_value(&mut self, con: u16, handle: u16, offset: u16) -> Result<Vec<u8>, u8> {
        let h = self.h;
        if handle == 0 {
            return Err(ATT_ERROR_READ_NOT_PERMITTED);
        }
        let mi = self.backend.info();
        if handle == h.name {
            return self.serve(con, handle, offset, mi.player_name_value());
        }
        if handle == h.title {
            return self.serve(con, handle, offset, text_value(&mi.title, MAX_TEXT));
        }
        Ok(match handle {
            _ if handle == h.duration => le32(mi.duration),
            _ if handle == h.position => le32(mi.position_now(now_ms())),
            _ if handle == h.speed => vec![mi.playback_speed() as u8],
            _ if handle == h.seeking_speed => vec![0],
            _ if handle == h.state => vec![mi.state],
            _ if handle == h.opcodes => mi.opcodes_supported().to_le_bytes().to_vec(),
            _ if handle == h.ccid => vec![CCID_GMCS],
            _ => return Err(ATT_ERROR_READ_NOT_PERMITTED),
        })
    }

    /// A value that may be longer than one ATT PDU: remember what offset 0
    /// returned; a later part (Read Blob) of a changed value fails with
    /// Value Changed During Read Long
    fn serve(&mut self, con: u16, handle: u16, offset: u16, value: Vec<u8>) -> Result<Vec<u8>, u8> {
        if offset == 0 {
            self.long_reads.insert((con, handle), value.clone());
        } else if self.long_reads.get(&(con, handle)).is_some_and(|first| *first != value) {
            tracing::info!("MCP: 0x{handle:04x} changed during a long read by 0x{con:04x}");
            return Err(ATT_ERROR_VALUE_CHANGED_DURING_READ_LONG);
        }
        Ok(value)
    }

    /// What to notify since the clients were last told (v1 publish)
    fn publish_values(&mut self, now: u64) -> Vec<Notification> {
        let mi = self.backend.info();
        let c = mi.changes_since(&self.last, now);
        let h = self.h;
        let mut out: Vec<Notification> = Vec::new();
        if c.player_name {
            out.push((h.name, mi.player_name_value(), true));
        }
        if c.track_changed {
            out.push((h.track_changed, Vec::new(), false));
        }
        if c.track_title {
            out.push((h.title, text_value(&mi.title, MAX_TEXT), true));
        }
        if c.track_duration {
            out.push((h.duration, le32(mi.duration), false));
        }
        if c.media_state {
            out.push((h.state, vec![mi.state], false));
        }
        if c.track_position {
            out.push((h.position, le32(mi.position_now(now)), false));
        }
        if c.playback_speed {
            out.push((h.speed, vec![mi.playback_speed() as u8], false));
        }
        if c.opcodes_supported {
            out.push((h.opcodes, mi.opcodes_supported().to_le_bytes().to_vec(), false));
        }
        self.last = mi;
        out
    }

    /// A Media Control Point write (v1 on_control_point): carried out even
    /// if the client did not enable control point notifications; only the
    /// result notification is left out then ([`reply`])
    fn on_control_point(&mut self, con: u16, value: &[u8], media_keys: bool) -> Result<(), u8> {
        let Some(&op) = value.first() else { return Err(ATT_ERROR_INVALID_ATTRIBUTE_VALUE_LENGTH) };
        let mi = self.backend.info();
        tracing::info!("MCP: control point {} (0x{op:02x}) from 0x{con:04x}", opcode_name(op));
        tracing::debug!("MCP: control point value {value:02X?}, media state 0x{:02X}", mi.state);
        let param = match control_point_check(&mi, value) {
            Ok((_, param)) => param,
            Err(result) => {
                tracing::debug!(
                    "MCP: control point {} refused by the check ({})",
                    opcode_name(op),
                    result_name(result)
                );
                (self.reply_later)(con, op, result);
                return Ok(());
            }
        };
        if !self.dry_run && !media_keys {
            tracing::info!("MCP: {} ignored (media keys off)", opcode_name(op));
            (self.reply_later)(con, op, RESULT_COMMAND_CANNOT_BE_COMPLETED);
            return Ok(());
        }
        // done: from the backend thread (the simulated player: at once)
        let reply_later = self.reply_later.clone();
        self.backend.command(op, param, Box::new(move |result| reply_later(con, op, result)));
        Ok(())
    }

    /// A Track Position write (v1 on_position_write): seek
    fn on_position_write(&mut self, value: &[u8], media_keys: bool) -> Result<(), u8> {
        let Ok(raw) = <[u8; 4]>::try_from(value) else { return Err(ATT_ERROR_INVALID_ATTRIBUTE_VALUE_LENGTH) };
        let mi = self.backend.info();
        // Ignored without an active seekable player
        let Some(p) = seek_target(&mi, i32::from_le_bytes(raw)) else {
            tracing::debug!("MCP: Track Position write {} ignored (no seekable player)", i32::from_le_bytes(raw));
            return Ok(());
        };
        if !self.dry_run && !media_keys {
            tracing::debug!("MCP: Track Position write ignored (media keys off)");
            return Ok(());
        }
        tracing::info!("MCP: Track Position write {p}");
        self.backend.seek(p);
        Ok(())
    }
}

/// The part of `value` that fits one notification of a connection with
/// this ATT MTU: text cut at a character boundary
fn fit(value: &[u8], mtu: u16, text: bool) -> &[u8] {
    let room = if mtu > 3 { mtu as usize - 3 } else { 20 };
    if value.len() <= room {
        value
    } else if text {
        cut_text(value, room)
    } else {
        &value[..room]
    }
}

/// Notification to every client that enabled it, cut to its MTU
fn send(server: &mut GattServer, handle: u16, value: &[u8], text: bool) {
    for con in server.clients() {
        let mtu = server.mtu(con);
        server.notify(con, handle, fit(value, mtu, text));
    }
}

/// BTstack thread: tell the clients what changed since the last call
fn publish(ctx: &mut LeCtx<'_>) {
    let Some(m) = ctx.module_mut::<McpModule>() else { return };
    let notes = m.publish_values(now_ms());
    if !notes.is_empty() {
        tracing::debug!("MCP: publishing {:04X?}", notes.iter().map(|(h, _, _)| *h).collect::<Vec<_>>());
    }
    let server = ctx.gatt_server();
    for (handle, value, text) in notes {
        send(server, handle, &value, text);
    }
}

/// BTstack thread: the control point result (a client without control
/// point notifications gets none: MCS, the write is still carried out)
fn reply(ctx: &mut LeCtx<'_>, con: u16, op: u8, result: u8) {
    let Some(cp) = ctx.module_mut::<McpModule>().map(|m| m.h.cp) else { return };
    let sent = ctx.gatt_server().notify(con, cp, &[op, result]);
    tracing::info!(
        "MCP: control point {} (0x{op:02x}) -> {}{}",
        opcode_name(op),
        result_name(result),
        if sent { "" } else { " (not notified: the client did not enable it)" }
    );
}

impl LeModule for McpModule {
    fn name(&self) -> &'static str {
        "MCP"
    }

    fn on_stack_init(&mut self, _ctx: &mut LeCtx<'_>) {
        self.last = MediaInfo::default();
        self.long_reads.clear();
        let poster = self.poster.clone();
        self.backend.start(Box::new(move || poster.post(publish)));
    }

    fn on_stack_deinit(&mut self) {
        self.backend.stop();
        self.last = MediaInfo::default();
        self.long_reads.clear();
    }

    fn on_disconnected(&mut self, ctx: &mut LeCtx<'_>, _dev: LeDeviceId, _reason: u8) {
        // The device's acl is already invalid: keep the long reads of the
        // links still up
        let up: Vec<u16> = ctx
            .device_ids()
            .into_iter()
            .filter_map(|id| ctx.device(id).map(|d| d.acl))
            .filter(|&acl| acl != CON_HANDLE_INVALID)
            .collect();
        self.long_reads.retain(|(con, _), _| up.contains(con));
    }

    fn add_gatt_services(&mut self, server: &mut GattServerBuilder<'_>) {
        let enc = ATT_SECURITY_ENCRYPTED;
        let read_notify = ATT_PROPERTY_READ | ATT_PROPERTY_NOTIFY;
        let write = ATT_PROPERTY_WRITE | ATT_PROPERTY_WRITE_WITHOUT_RESPONSE;
        server.begin_service(AttUuid::Uuid16(UUID_SERVICE_GMCS), true);
        let mut chr =
            |uuid: u16, properties: u16| server.add_characteristic(AttUuid::Uuid16(uuid), properties, enc, enc);
        self.h = Handles {
            name: chr(UUID_CHR_MEDIA_PLAYER_NAME, read_notify),
            track_changed: chr(UUID_CHR_TRACK_CHANGED, ATT_PROPERTY_NOTIFY),
            title: chr(UUID_CHR_TRACK_TITLE, read_notify),
            duration: chr(UUID_CHR_TRACK_DURATION, read_notify),
            position: chr(UUID_CHR_TRACK_POSITION, read_notify | write),
            speed: chr(UUID_CHR_PLAYBACK_SPEED, read_notify),
            seeking_speed: chr(UUID_CHR_SEEKING_SPEED, read_notify),
            state: chr(UUID_CHR_MEDIA_STATE, read_notify),
            cp: chr(UUID_CHR_MEDIA_CONTROL_POINT, write | ATT_PROPERTY_NOTIFY),
            opcodes: chr(UUID_CHR_OPCODES_SUPPORTED, read_notify),
            ccid: chr(UUID_CHR_CCID, ATT_PROPERTY_READ),
        };
        server.end_service();
        tracing::info!(
            "MCP: GMCS on the PC (CCID {CCID_GMCS}){}",
            if self.dry_run { ", dry run: media commands are only logged" } else { "" }
        );
    }

    fn add_ccids(&self, contexts: u16, ccids: &mut Vec<u8>) {
        // Every stream but a call is the PC's playback the player controls
        if contexts & !CONTEXT_CONVERSATIONAL != 0 {
            ccids.push(CCID_GMCS);
        }
    }

    fn on_att_read(&mut self, _ctx: &mut LeCtx<'_>, read: &AttRead) -> Result<Vec<u8>, u8> {
        let r = self.read_value(read.con, read.handle, read.offset);
        tracing::debug!(
            "MCP: read 0x{:04X} offset {} by 0x{:04X}: {:02X?}",
            read.handle,
            read.offset,
            read.con,
            r.as_deref()
        );
        r
    }

    fn on_att_write(&mut self, ctx: &mut LeCtx<'_>, write: &AttWrite<'_>) -> Result<(), u8> {
        if write.handle == 0 {
            return Err(ATT_ERROR_WRITE_NOT_PERMITTED);
        }
        if write.handle == self.h.cp {
            self.on_control_point(write.con, write.value, ctx.media_keys())
        } else if write.handle == self.h.position {
            self.on_position_write(write.value, ctx.media_keys())
        } else {
            Err(ATT_ERROR_WRITE_NOT_PERMITTED)
        }
    }
}

pub fn create(setup: &mut LeModuleSetup) -> LeModuleEntry {
    // None: the simulated player (dry run)
    let backend = setup.media.take().unwrap_or_else(|| Box::new(SimulatedPlayer::new()));
    LeModuleEntry { module: Box::new(McpModule::new(backend, setup.poster.clone())), hooks: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use le_audio::mcp_model::{
        OP_FAST_REWIND, OP_MOVE_RELATIVE, OP_NEXT_TRACK, OP_PLAY, RESULT_MEDIA_PLAYER_INACTIVE,
        RESULT_OPCODE_NOT_SUPPORTED, RESULT_SUCCESS, SIM_DURATION, SIM_PLAYER, STATE_PAUSED, STATE_PLAYING,
    };
    use std::sync::Mutex;

    type Replies = Arc<Mutex<Vec<(u16, u8, u8)>>>;

    /// A module with the simulated player, started, its GMCS added to a
    /// server; control point results recorded instead of posted
    fn module() -> (McpModule, Replies, GattServer) {
        let mut m = McpModule::new(Box::new(SimulatedPlayer::new()), LePoster::detached());
        let replies: Replies = Arc::default();
        let r = replies.clone();
        m.reply_later = Arc::new(move |con, op, result| r.lock().unwrap().push((con, op, result)));
        let mut server = GattServer::new();
        server.build_base("BlueCaster");
        m.add_gatt_services(&mut server.builder(2));
        m.backend.start(Box::new(|| {}));
        (m, replies, server)
    }

    #[test]
    fn gmcs_layout_and_reads() {
        let (mut m, _, server) = module();
        assert!(m.dry_run);
        // After GAP and GATT (handles 1-11): service 12, then declaration +
        // value (+ CCC for notify) per characteristic
        assert_eq!(m.h.name, 14);
        assert_eq!(m.h.track_changed, 17);
        assert_eq!(m.h.title, 20);
        assert_eq!(m.h.cp, 38);
        assert_eq!(m.h.ccid, 44);
        assert_eq!(server.owner(m.h.ccid), Some(2));
        let con = 0x40;
        assert_eq!(m.read_value(con, m.h.name, 0), Ok(SIM_PLAYER.as_bytes().to_vec()));
        assert_eq!(m.read_value(con, m.h.duration, 0), Ok(SIM_DURATION.to_le_bytes().to_vec()));
        assert_eq!(m.read_value(con, m.h.position, 0), Ok(vec![0, 0, 0, 0]));
        assert_eq!(m.read_value(con, m.h.speed, 0), Ok(vec![0]));
        assert_eq!(m.read_value(con, m.h.seeking_speed, 0), Ok(vec![0]));
        assert_eq!(m.read_value(con, m.h.state, 0), Ok(vec![STATE_PAUSED]));
        assert_eq!(m.read_value(con, m.h.opcodes, 0), Ok(0x1833u32.to_le_bytes().to_vec()));
        assert_eq!(m.read_value(con, m.h.ccid, 0), Ok(vec![CCID_GMCS]));
        assert!(m.read_value(con, m.h.title, 0).unwrap().starts_with(b"Test track 1"));
        assert_eq!(m.read_value(con, m.h.cp, 0), Err(ATT_ERROR_READ_NOT_PERMITTED));
        assert_eq!(m.read_value(con, 3, 0), Err(ATT_ERROR_READ_NOT_PERMITTED));
    }

    #[test]
    fn long_read_of_a_changed_title_fails() {
        let (mut m, replies, _) = module();
        let (a, b) = (0x40, 0x41);
        let first = m.read_value(a, m.h.title, 0).unwrap();
        assert!(first.len() > 61);
        // The same value: the rest is served
        assert_eq!(m.read_value(a, m.h.title, 61), Ok(first.clone()));
        m.read_value(b, m.h.title, 0).unwrap();
        // Next Track between the parts
        m.on_control_point(a, &[OP_NEXT_TRACK], true).unwrap();
        assert_eq!(replies.lock().unwrap()[..], [(a, OP_NEXT_TRACK, RESULT_SUCCESS)]);
        assert_eq!(m.read_value(a, m.h.title, 61), Err(ATT_ERROR_VALUE_CHANGED_DURING_READ_LONG));
        // A new read from offset 0 starts again
        let second = m.read_value(a, m.h.title, 0).unwrap();
        assert!(second.starts_with(b"Test track 2"));
        assert_eq!(m.read_value(a, m.h.title, 61), Ok(second));
        // b's long read still fails: it started on track 1
        assert_eq!(m.read_value(b, m.h.title, 61), Err(ATT_ERROR_VALUE_CHANGED_DURING_READ_LONG));
        // Duration is not a long value
        assert!(m.read_value(a, m.h.duration, 2).is_ok());
    }

    #[test]
    fn control_point_results() {
        let (mut m, replies, _) = module();
        let con = 0x40;
        assert_eq!(m.on_control_point(con, &[], true), Err(ATT_ERROR_INVALID_ATTRIBUTE_VALUE_LENGTH));
        m.on_control_point(con, &[OP_FAST_REWIND], true).unwrap();
        m.on_control_point(con, &[0x77], true).unwrap();
        m.on_control_point(con, &[OP_PLAY, 1], true).unwrap();
        m.on_control_point(con, &[OP_MOVE_RELATIVE, 1], true).unwrap();
        m.on_control_point(con, &[OP_PLAY], false).unwrap(); // dry run: media keys do not matter
        assert_eq!(m.backend.info().state, STATE_PLAYING);
        m.on_control_point(con, &[OP_MOVE_RELATIVE, 0xDC, 0x05, 0, 0], true).unwrap();
        assert!(m.backend.info().position_now(now_ms()) >= 1500);
        assert_eq!(
            replies.lock().unwrap()[..],
            [
                (con, OP_FAST_REWIND, RESULT_OPCODE_NOT_SUPPORTED),
                (con, 0x77, RESULT_OPCODE_NOT_SUPPORTED),
                (con, OP_PLAY, RESULT_COMMAND_CANNOT_BE_COMPLETED),
                (con, OP_MOVE_RELATIVE, RESULT_COMMAND_CANNOT_BE_COMPLETED),
                (con, OP_PLAY, RESULT_SUCCESS),
                (con, OP_MOVE_RELATIVE, RESULT_SUCCESS),
            ]
        );
        // Without a player: Move Relative is Media Player Inactive
        m.backend.stop();
        replies.lock().unwrap().clear();
        m.on_control_point(con, &[OP_MOVE_RELATIVE, 0, 0, 0, 0], true).unwrap();
        assert_eq!(replies.lock().unwrap()[..], [(con, OP_MOVE_RELATIVE, RESULT_MEDIA_PLAYER_INACTIVE)]);
    }

    #[test]
    fn media_keys_off_refuses_real_commands() {
        let (mut m, replies, _) = module();
        m.dry_run = false;
        m.on_control_point(0x40, &[OP_PLAY], false).unwrap();
        assert_eq!(replies.lock().unwrap()[..], [(0x40, OP_PLAY, RESULT_COMMAND_CANNOT_BE_COMPLETED)]);
        assert_eq!(m.backend.info().state, STATE_PAUSED);
        m.on_position_write(&1000i32.to_le_bytes(), false).unwrap();
        assert_eq!(m.backend.info().position, 0);
    }

    #[test]
    fn track_position_writes() {
        let (mut m, _, _) = module();
        assert_eq!(m.on_position_write(&[1, 2], true), Err(ATT_ERROR_INVALID_ATTRIBUTE_VALUE_LENGTH));
        m.on_position_write(&1000i32.to_le_bytes(), true).unwrap();
        assert_eq!(m.backend.info().position, 1000);
        // Negative: from the end
        m.on_position_write(&(-500i32).to_le_bytes(), true).unwrap();
        assert_eq!(m.backend.info().position, SIM_DURATION - 500);
    }

    #[test]
    fn publish_notifies_what_changed() {
        let (mut m, _, _) = module();
        let h = m.h;
        let handles = |n: &[Notification]| n.iter().map(|(h, _, _)| *h).collect::<Vec<_>>();
        // First: everything the clients have not been told
        let first = m.publish_values(now_ms());
        assert_eq!(handles(&first), [h.name, h.track_changed, h.title, h.duration, h.state, h.position, h.opcodes]);
        assert!(first[0].2 && first[2].2 && !first[3].2);
        assert!(m.publish_values(now_ms()).is_empty());
        m.on_control_point(0x40, &[OP_PLAY], true).unwrap();
        let play = m.publish_values(now_ms());
        assert_eq!(handles(&play), [h.state, h.position]);
        assert_eq!(play[0].1, [STATE_PLAYING]);
        m.on_control_point(0x40, &[OP_NEXT_TRACK], true).unwrap();
        let next = m.publish_values(now_ms());
        assert_eq!(handles(&next), [h.track_changed, h.title, h.duration, h.position]);
        assert!(next[1].1.starts_with(b"Test track 2"));
    }

    #[test]
    fn notifications_fit_the_mtu() {
        let title = "Test track 1 \u{266A} \u{DC}n\u{EF}c\u{F6}d\u{E9}".as_bytes();
        let cut = fit(title, 23, true);
        assert!(cut.len() <= 20 && std::str::from_utf8(cut).is_ok());
        assert_eq!(cut, &title[..cut.len()]);
        // The note (3 octets) would be split at 20 octets
        let s = "Test track 10 abcd\u{266A}x".as_bytes();
        assert_eq!(fit(s, 23, true), &s[..18]);
        assert_eq!(fit(s, 23, false), &s[..20]);
        assert_eq!(fit(s, 64, true), s);
        assert_eq!(fit(&[1, 2, 3, 4], 0, false), [1, 2, 3, 4]);
    }

    #[test]
    fn ccid_for_every_stream_but_a_call() {
        let (m, _, _) = module();
        let ccids = |contexts: u16| {
            let mut v = Vec::new();
            m.add_ccids(contexts, &mut v);
            v
        };
        assert_eq!(ccids(le_audio::defs::CONTEXT_MEDIA), [CCID_GMCS]);
        assert_eq!(ccids(le_audio::defs::CONTEXT_MEDIA | CONTEXT_CONVERSATIONAL), [CCID_GMCS]);
        assert!(ccids(CONTEXT_CONVERSATIONAL).is_empty());
    }
}
