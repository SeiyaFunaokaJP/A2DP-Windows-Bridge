//! MCP: the media model of the GMCS server on the PC (v1
//! le_audio_mcp.cpp, le_audio_mcp_media.{h,cpp}).
//!
//! The PC is the Media Control Server; the headset reads the player state
//! from GMCS and sends play / pause / stop / next / previous / move relative
//! through the Media Control Point. [`MediaBackend`] is the Windows side
//! (System Media Transport Controls, media keys as a fallback): core
//! implements it with win-sys and hands it to bt-host in `LeConfig::media`.
//! Without one (`--mcp-dry-run`, implied by `--hci-tcp`) the
//! [`SimulatedPlayer`] answers and commands are only logged (`MCP: dry run:
//! <OP>`).
//!
//! The pure parts of v1's module and Windows backend are here too, so that
//! both sides behave as v1 did: the SMTC playback status mapping
//! ([`state_from_playback_status`]), the player name of an AUMID
//! ([`name_from_aumid`]), the media key of a command ([`media_key`]), the
//! control point checks ([`control_point_check`]), Track Position writes
//! ([`seek_target`]) and which characteristics to notify after a change
//! ([`MediaInfo::changes_since`]).
//!
//! Times: [`MediaInfo::updated_ms`] and every `now_ms` argument are
//! milliseconds of [`now_ms`], one monotonic clock for the backends and the
//! module.

use std::fmt;
use std::sync::OnceLock;
use std::time::Instant;

// GMCS characteristics
pub const UUID_CHR_MEDIA_PLAYER_NAME: u16 = 0x2B93;
pub const UUID_CHR_TRACK_CHANGED: u16 = 0x2B96;
pub const UUID_CHR_TRACK_TITLE: u16 = 0x2B97;
pub const UUID_CHR_TRACK_DURATION: u16 = 0x2B98;
pub const UUID_CHR_TRACK_POSITION: u16 = 0x2B99;
pub const UUID_CHR_PLAYBACK_SPEED: u16 = 0x2B9A;
pub const UUID_CHR_SEEKING_SPEED: u16 = 0x2B9B;
pub const UUID_CHR_MEDIA_STATE: u16 = 0x2BA3;
pub const UUID_CHR_MEDIA_CONTROL_POINT: u16 = 0x2BA4;
pub const UUID_CHR_OPCODES_SUPPORTED: u16 = 0x2BA5;

// Media State (MCS 3.17)
pub const STATE_INACTIVE: u8 = 0x00;
pub const STATE_PLAYING: u8 = 0x01;
pub const STATE_PAUSED: u8 = 0x02;
pub const STATE_SEEKING: u8 = 0x03;

// Media Control Point opcodes (MCS 3.18) A2DPWB knows
pub const OP_PLAY: u8 = 0x01;
pub const OP_PAUSE: u8 = 0x02;
pub const OP_FAST_REWIND: u8 = 0x03;
pub const OP_FAST_FORWARD: u8 = 0x04;
pub const OP_STOP: u8 = 0x05;
pub const OP_MOVE_RELATIVE: u8 = 0x10;
pub const OP_PREVIOUS_TRACK: u8 = 0x30;
pub const OP_NEXT_TRACK: u8 = 0x31;

// Media Control Point result codes (MCS 3.18.2)
pub const RESULT_SUCCESS: u8 = 0x01;
pub const RESULT_OPCODE_NOT_SUPPORTED: u8 = 0x02;
pub const RESULT_MEDIA_PLAYER_INACTIVE: u8 = 0x03;
pub const RESULT_COMMAND_CANNOT_BE_COMPLETED: u8 = 0x04;

/// MCS application error: the value changed between the parts of a long
/// read
pub const ATT_ERROR_VALUE_CHANGED_DURING_READ_LONG: u8 = 0x80;
/// Longest attribute value (ATT)
pub const MAX_TEXT: usize = 512;
/// 2 s off the expected position: Track Position is notified
pub const POSITION_JUMP: i32 = 200;

/// Milliseconds of a monotonic clock that starts with the process: the
/// clock of [`MediaInfo::updated_ms`] (v1 GetTickCount64())
pub fn now_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// Name for the log, e.g. "PLAY" (the emulator checks `MCP: dry run: <OP>`)
pub fn opcode_name(opcode: u8) -> &'static str {
    match opcode {
        OP_PLAY => "PLAY",
        OP_PAUSE => "PAUSE",
        OP_FAST_REWIND => "FAST_REWIND",
        OP_FAST_FORWARD => "FAST_FORWARD",
        OP_STOP => "STOP",
        OP_MOVE_RELATIVE => "MOVE_RELATIVE",
        0x20 => "PREVIOUS_SEGMENT",
        0x21 => "NEXT_SEGMENT",
        0x22 => "FIRST_SEGMENT",
        0x23 => "LAST_SEGMENT",
        0x24 => "GOTO_SEGMENT",
        OP_PREVIOUS_TRACK => "PREVIOUS_TRACK",
        OP_NEXT_TRACK => "NEXT_TRACK",
        0x32 => "FIRST_TRACK",
        0x33 => "LAST_TRACK",
        0x34 => "GOTO_TRACK",
        0x40 => "PREVIOUS_GROUP",
        0x41 => "NEXT_GROUP",
        0x42 => "FIRST_GROUP",
        0x43 => "LAST_GROUP",
        0x44 => "GOTO_GROUP",
        _ => "unknown",
    }
}

/// Name for the log
pub fn state_name(state: u8) -> &'static str {
    match state {
        STATE_INACTIVE => "inactive",
        STATE_PLAYING => "playing",
        STATE_PAUSED => "paused",
        STATE_SEEKING => "seeking",
        _ => "?",
    }
}

/// Name for the log
pub fn result_name(result: u8) -> &'static str {
    match result {
        RESULT_SUCCESS => "Success",
        RESULT_OPCODE_NOT_SUPPORTED => "Opcode Not Supported",
        RESULT_MEDIA_PLAYER_INACTIVE => "Media Player Inactive",
        RESULT_COMMAND_CANNOT_BE_COMPLETED => "Command Cannot Be Completed",
        _ => "?",
    }
}

/// Media Control Point Opcodes Supported bit of an opcode (MCS 3.19), 0 if
/// unknown
pub fn opcode_bit(opcode: u8) -> u32 {
    match opcode {
        OP_PLAY => 1 << 0,
        OP_PAUSE => 1 << 1,
        OP_FAST_REWIND => 1 << 2,
        OP_FAST_FORWARD => 1 << 3,
        OP_STOP => 1 << 4,
        OP_MOVE_RELATIVE => 1 << 5,
        OP_PREVIOUS_TRACK => 1 << 11,
        OP_NEXT_TRACK => 1 << 12,
        _ => 0,
    }
}

/// UTF-8 text cut to at most `max` octets without splitting a character
pub fn text_value(s: &str, max: usize) -> Vec<u8> {
    let mut n = s.len().min(max);
    while !s.is_char_boundary(n) {
        n -= 1;
    }
    s.as_bytes()[..n].to_vec()
}

/// [`text_value`] of raw octets (a value cut for a notification): cut at
/// most `max` octets, not inside a UTF-8 sequence
pub fn cut_text(value: &[u8], max: usize) -> &[u8] {
    let mut n = value.len().min(max);
    if n < value.len() {
        while n > 0 && value[n] & 0xC0 == 0x80 {
            n -= 1;
        }
    }
    &value[..n]
}

/// Media State of a Windows media session from its
/// GlobalSystemMediaTransportControlsSessionPlaybackStatus (Closed 0,
/// Opened 1, Changing 2, Stopped 3, Playing 4, Paused 5): Playing ->
/// playing, Closed -> inactive, everything else -> paused. No session at
/// all is inactive ([`MediaInfo::default`]).
pub fn state_from_playback_status(status: i32) -> u8 {
    match status {
        4 => STATE_PLAYING,
        0 => STATE_INACTIVE,
        _ => STATE_PAUSED,
    }
}

/// The player name of a session's AppUserModelId when Windows has no
/// display name for it: "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify" ->
/// "SpotifyMusic", "C:\...\foobar2000.exe" / "Spotify.exe" -> "foobar2000"
/// / "Spotify"; "Windows" if nothing is left
pub fn name_from_aumid(aumid: &str) -> String {
    let mut s = aumid;
    if let Some(bang) = s.find('!') {
        s = &s[..bang];
        s = s.split('_').next().unwrap_or("");
        if let Some(dot) = s.rfind('.')
            && dot + 1 < s.len()
        {
            s = &s[dot + 1..];
        }
    }
    if let Some(slash) = s.rfind(['\\', '/']) {
        s = &s[slash + 1..];
    }
    if s.len() > 4 && s.is_char_boundary(s.len() - 4) && s[s.len() - 4..].eq_ignore_ascii_case(".exe") {
        s = &s[..s.len() - 4];
    }
    if s.is_empty() { "Windows".into() } else { s.into() }
}

/// The media key (Windows virtual-key code) a command becomes when Windows
/// has no media session, as for AVRCP buttons: Play and Pause both the
/// Play/Pause toggle; None for the others (Media Player Inactive)
pub fn media_key(opcode: u8) -> Option<u16> {
    match opcode {
        OP_PLAY | OP_PAUSE => Some(0xB3), // VK_MEDIA_PLAY_PAUSE
        OP_STOP => Some(0xB2),            // VK_MEDIA_STOP
        OP_NEXT_TRACK => Some(0xB0),      // VK_MEDIA_NEXT_TRACK
        OP_PREVIOUS_TRACK => Some(0xB1),  // VK_MEDIA_PREV_TRACK
        _ => None,
    }
}

/// What the headset sees of the Windows media player (v1 mcp::MediaInfo)
#[derive(Debug, Clone, PartialEq)]
pub struct MediaInfo {
    /// A media session (or the simulated player) exists
    pub session: bool,
    /// STATE_*
    pub state: u8,
    /// UTF-8
    pub player: String,
    /// UTF-8
    pub title: String,
    /// 0.01 s, -1 unknown
    pub duration: i32,
    /// 0.01 s at `updated_ms`, -1 unknown
    pub position: i32,
    /// [`now_ms`] of `position`
    pub updated_ms: u64,
    /// Playback rate
    pub rate: f64,
    /// Move Relative / Track Position writes possible
    pub can_seek: bool,
    /// Changes with the track (title, duration, player)
    pub track: u32,
}

impl Default for MediaInfo {
    fn default() -> Self {
        MediaInfo {
            session: false,
            state: STATE_INACTIVE,
            player: String::new(),
            title: String::new(),
            duration: -1,
            position: -1,
            updated_ms: 0,
            rate: 1.0,
            can_seek: false,
            track: 0,
        }
    }
}

/// The GMCS characteristics to notify after a change (v1 McpModule::publish)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MediaChanges {
    /// Media Player Name ([`MediaInfo::player_name_value`])
    pub player_name: bool,
    /// Track Changed (empty value)
    pub track_changed: bool,
    pub track_title: bool,
    pub track_duration: bool,
    pub media_state: bool,
    /// Track Position (the position now)
    pub track_position: bool,
    pub playback_speed: bool,
    pub opcodes_supported: bool,
}

impl MediaInfo {
    /// Position at `now_ms` (playing: advanced since `updated_ms` at
    /// `rate`, at most the duration), -1 unknown
    pub fn position_now(&self, now_ms: u64) -> i32 {
        if self.position < 0 {
            return -1;
        }
        let mut p = self.position as i64;
        if self.state == STATE_PLAYING && self.rate > 0.0 {
            p += (now_ms.saturating_sub(self.updated_ms) as f64 / 10.0 * self.rate) as i64;
        }
        if self.duration > 0 && p > self.duration as i64 {
            p = self.duration as i64;
        }
        p.max(0) as i32
    }

    /// MCS Playback Speed: 64 * log2(rate), -128..127
    pub fn playback_speed(&self) -> i8 {
        if self.rate <= 0.0 {
            return 0;
        }
        (64.0 * self.rate.log2()).round().clamp(-128.0, 127.0) as i8
    }

    /// Opcodes Supported: Play, Pause, Stop, Previous / Next Track, + Move
    /// Relative if seekable
    pub fn opcodes_supported(&self) -> u32 {
        let mut ops = opcode_bit(OP_PLAY)
            | opcode_bit(OP_PAUSE)
            | opcode_bit(OP_STOP)
            | opcode_bit(OP_PREVIOUS_TRACK)
            | opcode_bit(OP_NEXT_TRACK);
        if self.session && self.can_seek {
            ops |= opcode_bit(OP_MOVE_RELATIVE);
        }
        ops
    }

    /// Media Player Name: the player, "Windows" without one
    pub fn player_name_value(&self) -> Vec<u8> {
        let name = if self.session && !self.player.is_empty() { self.player.as_str() } else { "Windows" };
        text_value(name, MAX_TEXT)
    }

    /// What to notify when the clients were last told `old` and the state
    /// is now `self` (v1 McpModule::publish)
    pub fn changes_since(&self, old: &MediaInfo, now_ms: u64) -> MediaChanges {
        let track = self.track != old.track || self.session != old.session;
        let (p, q) = (self.position_now(now_ms), old.position_now(now_ms));
        MediaChanges {
            player_name: self.session != old.session || self.player != old.player,
            track_changed: track,
            track_title: track || self.title != old.title,
            track_duration: track || self.duration != old.duration,
            media_state: self.state != old.state,
            track_position: track
                || self.state != old.state
                || (p < 0) != (q < 0)
                || (p as i64 - q as i64).abs() > POSITION_JUMP as i64,
            playback_speed: self.playback_speed() != old.playback_speed(),
            opcodes_supported: self.opcodes_supported() != old.opcodes_supported(),
        }
    }

    /// True if a backend must call its ChangedFn going from `self` to
    /// `next`: anything but the position advancing as expected (within
    /// [`POSITION_JUMP`]) (v1 MediaBackend::Impl::publish)
    pub fn differs_from(&self, next: &MediaInfo, now_ms: u64) -> bool {
        let (a, b) = (self.position_now(now_ms), next.position_now(now_ms));
        self.session != next.session
            || self.state != next.state
            || self.player != next.player
            || self.title != next.title
            || self.duration != next.duration
            || self.track != next.track
            || self.can_seek != next.can_seek
            || self.playback_speed() != next.playback_speed()
            || (a < 0) != (b < 0)
            || (a as i64 - b as i64).abs() > POSITION_JUMP as i64
    }
}

/// The checks of a Media Control Point write before it goes to the
/// backend (v1 McpModule::on_control_point; `value` is not empty - an
/// empty write is ATT error 0x0D): Ok((opcode, Move Relative offset)) to
/// carry out, or Err(result code) to notify at once: an opcode not in
/// Opcodes Supported (Move Relative without an active seekable player:
/// Media Player Inactive, else Opcode Not Supported), a wrong length
/// (Command Cannot Be Completed). The "media keys" setting is the
/// module's check after this one.
pub fn control_point_check(info: &MediaInfo, value: &[u8]) -> Result<(u8, i32), u8> {
    let Some(&op) = value.first() else { return Err(RESULT_COMMAND_CANNOT_BE_COMPLETED) };
    if info.opcodes_supported() & opcode_bit(op) == 0 {
        let inactive = op == OP_MOVE_RELATIVE && (!info.session || info.state == STATE_INACTIVE);
        tracing::debug!(
            "MCP: {} not supported now (opcodes 0x{:08X}, session {}, state {})",
            opcode_name(op),
            info.opcodes_supported(),
            info.session,
            state_name(info.state)
        );
        return Err(if inactive { RESULT_MEDIA_PLAYER_INACTIVE } else { RESULT_OPCODE_NOT_SUPPORTED });
    }
    if value.len() != if op == OP_MOVE_RELATIVE { 5 } else { 1 } {
        tracing::debug!("MCP: {} with a wrong length: {value:02X?}", opcode_name(op));
        return Err(RESULT_COMMAND_CANNOT_BE_COMPLETED);
    }
    let param = if op == OP_MOVE_RELATIVE { i32::from_le_bytes([value[1], value[2], value[3], value[4]]) } else { 0 };
    Ok((op, param))
}

/// A Track Position write (v1 McpModule::on_position_write; the value is 4
/// octets, else ATT error 0x0D): the position to seek to, a negative one
/// counted from the end; None (ignored) without an active seekable player
pub fn seek_target(info: &MediaInfo, position: i32) -> Option<i32> {
    if !info.session || info.state == STATE_INACTIVE || !info.can_seek {
        tracing::debug!(
            "MCP: seek to {position} ignored (session {}, state {}, seekable {})",
            info.session,
            state_name(info.state),
            info.can_seek
        );
        return None;
    }
    Some(if position >= 0 {
        position
    } else if info.duration > 0 {
        (info.duration as i64 + position as i64).max(0) as i32
    } else {
        0
    })
}

/// Called by a backend (from its own thread) whenever [`MediaBackend::info`]
/// changed, other than the position advancing while playing
pub type ChangedFn = Box<dyn Fn() + Send + Sync>;
/// Called by a backend with the Media Control Point result code (RESULT_*)
pub type DoneFn = Box<dyn FnOnce(u8) + Send>;

/// The Windows side of the media control server (v1 mcp::MediaBackend).
/// Runs a thread of its own, never the BTstack thread: `command` and `seek`
/// return at once and `done` is called later from that thread.
pub trait MediaBackend: Send {
    /// Starts the backend thread
    fn start(&mut self, changed: ChangedFn);
    /// Stops and joins the thread; pending commands are dropped
    fn stop(&mut self);
    /// True for the simulated player (commands are only logged)
    fn dry_run(&self) -> bool;
    /// Latest state
    fn info(&self) -> MediaInfo;
    /// Carries out a Media Control Point opcode (`param`: Move Relative
    /// offset in 0.01 s)
    fn command(&mut self, opcode: u8, param: i32, done: DoneFn);
    /// Track Position write: seek to `position` (0.01 s from the start)
    fn seek(&mut self, position: i32);
}

/// The simulated player's name
pub const SIM_PLAYER: &str = "A2DPWB test player";
/// The simulated tracks' duration: 180 s
pub const SIM_DURATION: i32 = 18000;

/// The dry-run player (v1 MediaBackend with dry_run): a simulated player
/// that answers like a real one, for the emulator tests: "A2DPWB test
/// player", paused, "Test track N ..." with a long non-ASCII title, 180 s,
/// seekable. It has no thread: `command` and `seek` act at once and call
/// `done` / the ChangedFn before they return (the module posts its
/// notifications to a later run loop turn, as v1 did).
#[derive(Default)]
pub struct SimulatedPlayer {
    info: MediaInfo,
    started: bool,
    sim_track: u32,
    changed: Option<ChangedFn>,
}

impl fmt::Debug for SimulatedPlayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SimulatedPlayer")
            .field("info", &self.info)
            .field("started", &self.started)
            .field("sim_track", &self.sim_track)
            .finish_non_exhaustive()
    }
}

impl SimulatedPlayer {
    pub fn new() -> SimulatedPlayer {
        SimulatedPlayer::default()
    }

    /// Long and not ASCII: tests cutting notifications to the MTU at a
    /// character boundary and long reads
    fn set_track(&self, mi: &mut MediaInfo) {
        let mut title = format!("Test track {} \u{266A}", self.sim_track);
        for _ in 0..5 {
            title += " \u{DC}n\u{EF}c\u{F6}d\u{E9} t\u{ED}tl\u{E9}";
        }
        mi.title = title;
        mi.track = self.sim_track;
        mi.position = 0;
    }

    /// New state; the ChangedFn unless only the position moved on as
    /// expected
    fn publish(&mut self, next: MediaInfo) {
        let prev = std::mem::replace(&mut self.info, next);
        if !prev.differs_from(&self.info, now_ms()) {
            return;
        }
        if prev.state != self.info.state || prev.player != self.info.player {
            tracing::info!(
                "MCP: media player \"{}\": {}",
                if self.info.session { self.info.player.as_str() } else { "(none)" },
                state_name(self.info.state)
            );
        }
        if let Some(changed) = &self.changed {
            changed();
        }
    }

    fn execute(&mut self, op: u8, param: i32) -> u8 {
        let mut mi = self.info.clone();
        let now = now_ms();
        mi.position = mi.position_now(now);
        mi.updated_ms = now;
        match op {
            OP_PLAY => mi.state = STATE_PLAYING,
            OP_PAUSE => mi.state = STATE_PAUSED,
            OP_STOP => {
                mi.state = STATE_PAUSED;
                mi.position = 0;
            }
            OP_NEXT_TRACK => {
                self.sim_track += 1;
                self.set_track(&mut mi);
            }
            OP_PREVIOUS_TRACK => {
                if mi.position > 300 || self.sim_track == 1 {
                    mi.position = 0;
                } else {
                    self.sim_track -= 1;
                    self.set_track(&mut mi);
                }
            }
            OP_MOVE_RELATIVE => {
                mi.position = (mi.position as i64 + param as i64).clamp(0, mi.duration.max(0) as i64) as i32;
            }
            _ => return RESULT_OPCODE_NOT_SUPPORTED,
        }
        if op == OP_MOVE_RELATIVE {
            tracing::info!("MCP: dry run: {} {param} (nothing sent to Windows)", opcode_name(op));
        } else {
            tracing::info!("MCP: dry run: {} (nothing sent to Windows)", opcode_name(op));
        }
        self.publish(mi);
        RESULT_SUCCESS
    }
}

impl MediaBackend for SimulatedPlayer {
    fn start(&mut self, changed: ChangedFn) {
        self.stop();
        self.started = true;
        self.changed = Some(changed);
        self.sim_track = 1;
        tracing::info!("MCP: dry run: simulated player, media commands are only logged");
        let mut mi = MediaInfo {
            session: true,
            state: STATE_PAUSED,
            player: SIM_PLAYER.into(),
            duration: SIM_DURATION,
            can_seek: true,
            updated_ms: now_ms(),
            ..MediaInfo::default()
        };
        self.set_track(&mut mi);
        self.publish(mi);
    }

    fn stop(&mut self) {
        self.started = false;
        self.changed = None;
        self.info = MediaInfo::default();
        self.sim_track = 1;
    }

    fn dry_run(&self) -> bool {
        true
    }

    fn info(&self) -> MediaInfo {
        if self.started { self.info.clone() } else { MediaInfo::default() }
    }

    fn command(&mut self, opcode: u8, param: i32, done: DoneFn) {
        if !self.started {
            done(RESULT_MEDIA_PLAYER_INACTIVE);
            return;
        }
        let result = self.execute(opcode, param);
        done(result);
    }

    fn seek(&mut self, position: i32) {
        if !self.started {
            return;
        }
        let mut mi = self.info.clone();
        mi.position = position.clamp(0, mi.duration.max(0));
        mi.updated_ms = now_ms();
        tracing::info!("MCP: dry run: seek to {position} (nothing sent to Windows)");
        self.publish(mi);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};

    #[test]
    fn names_and_bits() {
        assert_eq!(opcode_name(OP_PLAY), "PLAY");
        assert_eq!(opcode_name(OP_NEXT_TRACK), "NEXT_TRACK");
        assert_eq!(opcode_name(0x22), "FIRST_SEGMENT");
        assert_eq!(opcode_name(0x77), "unknown");
        assert_eq!(state_name(STATE_PAUSED), "paused");
        assert_eq!(state_name(9), "?");
        assert_eq!(result_name(RESULT_MEDIA_PLAYER_INACTIVE), "Media Player Inactive");
        assert_eq!(opcode_bit(OP_PREVIOUS_TRACK), 0x800);
        assert_eq!(opcode_bit(0x34), 0);
    }

    #[test]
    fn smtc_state_mapping() {
        // Closed, Opened, Changing, Stopped, Playing, Paused
        let states: Vec<u8> = (0..6).map(state_from_playback_status).collect();
        assert_eq!(states, [STATE_INACTIVE, STATE_PAUSED, STATE_PAUSED, STATE_PAUSED, STATE_PLAYING, STATE_PAUSED]);
        // No session
        assert_eq!(MediaInfo::default().state, STATE_INACTIVE);
    }

    #[test]
    fn aumid_names_and_keys() {
        assert_eq!(name_from_aumid("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"), "SpotifyMusic");
        assert_eq!(name_from_aumid("C:\\Program Files\\foobar2000\\foobar2000.exe"), "foobar2000");
        assert_eq!(name_from_aumid("Spotify.EXE"), "Spotify");
        assert_eq!(name_from_aumid("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"), "ZuneMusic");
        assert_eq!(name_from_aumid("chrome"), "chrome");
        assert_eq!(name_from_aumid(""), "Windows");
        assert_eq!(name_from_aumid("_x!y"), "Windows");
        assert_eq!(media_key(OP_PAUSE), Some(0xB3));
        assert_eq!(media_key(OP_PREVIOUS_TRACK), Some(0xB1));
        assert_eq!(media_key(OP_MOVE_RELATIVE), None);
    }

    #[test]
    fn text_values() {
        assert_eq!(text_value("abc", 2), b"ab");
        assert_eq!(text_value("a\u{E9}", 2), b"a");
        assert_eq!(text_value("a\u{E9}", 3), "a\u{E9}".as_bytes());
        assert_eq!(text_value("\u{266A}x", 2), b"");
        assert_eq!(cut_text("a\u{266A}".as_bytes(), 3), b"a");
        assert_eq!(cut_text(b"abc", 5), b"abc");
    }

    fn playing(position: i32, rate: f64) -> MediaInfo {
        MediaInfo {
            session: true,
            state: STATE_PLAYING,
            duration: 1000,
            position,
            updated_ms: 5000,
            rate,
            can_seek: true,
            ..MediaInfo::default()
        }
    }

    #[test]
    fn position_and_speed() {
        assert_eq!(playing(100, 1.0).position_now(6000), 200);
        assert_eq!(playing(100, 2.0).position_now(6000), 300);
        assert_eq!(playing(100, 1.0).position_now(60000), 1000);
        assert_eq!(playing(100, 1.0).position_now(4000), 100);
        assert_eq!(playing(-1, 1.0).position_now(6000), -1);
        let paused = MediaInfo { state: STATE_PAUSED, ..playing(100, 1.0) };
        assert_eq!(paused.position_now(60000), 100);
        assert_eq!(playing(0, 1.0).playback_speed(), 0);
        assert_eq!(playing(0, 2.0).playback_speed(), 64);
        assert_eq!(playing(0, 0.5).playback_speed(), -64);
        assert_eq!(playing(0, 1.5).playback_speed(), 37);
        assert_eq!(playing(0, 1e6).playback_speed(), 127);
        assert_eq!(playing(0, 0.0).playback_speed(), 0);
        assert_eq!(playing(0, 1.0).opcodes_supported(), 0x1833);
        let no_seek = MediaInfo { can_seek: false, ..playing(0, 1.0) };
        assert_eq!(no_seek.opcodes_supported(), 0x1813);
        assert_eq!(MediaInfo::default().opcodes_supported(), 0x1813);
        assert_eq!(MediaInfo::default().player_name_value(), b"Windows");
        assert_eq!(MediaInfo { player: "VLC".into(), ..playing(0, 1.0) }.player_name_value(), b"VLC");
    }

    #[test]
    fn changes_to_notify() {
        let a = playing(100, 1.0);
        // Position moved on as expected: nothing
        let b = MediaInfo { position: 200, updated_ms: 6000, ..a.clone() };
        assert_eq!(b.changes_since(&a, 6000), MediaChanges::default());
        assert!(!a.differs_from(&b, 6000));
        // Jumped by more than 2 s: position
        let c = MediaInfo { position: 500, ..a.clone() };
        assert_eq!(c.changes_since(&a, 5000), MediaChanges { track_position: true, ..MediaChanges::default() });
        assert!(a.differs_from(&c, 5000));
        // Paused: state + position
        let d = MediaInfo { state: STATE_PAUSED, ..a.clone() };
        let ch = d.changes_since(&a, 5000);
        assert!(ch.media_state && ch.track_position && !ch.track_changed);
        // New track: track changed, title, duration, position
        let e = MediaInfo { track: 2, title: "x".into(), ..a.clone() };
        let ch = e.changes_since(&a, 5000);
        assert!(ch.track_changed && ch.track_title && ch.track_duration && ch.track_position && !ch.player_name);
        // Session gone: everything about the player
        let ch = MediaInfo::default().changes_since(&a, 5000);
        assert!(ch.player_name && ch.track_changed && ch.media_state && ch.opcodes_supported);
        let f = MediaInfo { rate: 2.0, ..a.clone() };
        assert!(f.changes_since(&a, 5000).playback_speed);
    }

    #[test]
    fn control_point_checks() {
        let player = playing(0, 1.0);
        assert_eq!(control_point_check(&player, &[OP_PLAY]), Ok((OP_PLAY, 0)));
        assert_eq!(control_point_check(&player, &[OP_MOVE_RELATIVE, 0xDC, 0x05, 0, 0]), Ok((OP_MOVE_RELATIVE, 1500)));
        assert_eq!(
            control_point_check(&player, &[OP_MOVE_RELATIVE, 0x24, 0xFA, 0xFF, 0xFF]),
            Ok((OP_MOVE_RELATIVE, -1500))
        );
        // mcp-unsupported: fast-rewind, first-segment, 0x77, goto-track,
        // raw 0101 (Play with a parameter)
        for op in [OP_FAST_REWIND, 0x22, 0x77, 0x34] {
            assert_eq!(control_point_check(&player, &[op]), Err(RESULT_OPCODE_NOT_SUPPORTED));
        }
        assert_eq!(control_point_check(&player, &[OP_PLAY, 0x01]), Err(RESULT_COMMAND_CANNOT_BE_COMPLETED));
        assert_eq!(control_point_check(&player, &[OP_MOVE_RELATIVE, 0]), Err(RESULT_COMMAND_CANNOT_BE_COMPLETED));
        // Move Relative without a player / with one that cannot seek
        let none = MediaInfo::default();
        assert_eq!(control_point_check(&none, &[OP_MOVE_RELATIVE, 0, 0, 0, 0]), Err(RESULT_MEDIA_PLAYER_INACTIVE));
        assert_eq!(control_point_check(&none, &[OP_PLAY]), Ok((OP_PLAY, 0)));
        let no_seek = MediaInfo { can_seek: false, ..player };
        assert_eq!(control_point_check(&no_seek, &[OP_MOVE_RELATIVE, 0, 0, 0, 0]), Err(RESULT_OPCODE_NOT_SUPPORTED));
    }

    #[test]
    fn seek_targets() {
        let player = playing(0, 1.0);
        assert_eq!(seek_target(&player, 300), Some(300));
        assert_eq!(seek_target(&player, -300), Some(700));
        assert_eq!(seek_target(&player, -3000), Some(0));
        assert_eq!(seek_target(&MediaInfo { duration: -1, ..player.clone() }, -3), Some(0));
        assert_eq!(seek_target(&MediaInfo { can_seek: false, ..player.clone() }, 3), None);
        assert_eq!(seek_target(&MediaInfo { state: STATE_INACTIVE, ..player }, 3), None);
        assert_eq!(seek_target(&MediaInfo::default(), 3), None);
    }

    #[test]
    fn simulated_player() {
        let mut p = SimulatedPlayer::new();
        assert!(p.dry_run());
        let last = Arc::new(AtomicU8::new(0));
        let l = last.clone();
        p.command(OP_PLAY, 0, Box::new(move |r| l.store(r, Ordering::SeqCst)));
        assert_eq!(last.load(Ordering::SeqCst), RESULT_MEDIA_PLAYER_INACTIVE);
        assert_eq!(p.info(), MediaInfo::default());

        let changes = Arc::new(AtomicU32::new(0));
        let c = changes.clone();
        p.start(Box::new(move || {
            c.fetch_add(1, Ordering::SeqCst);
        }));
        assert_eq!(changes.load(Ordering::SeqCst), 1);
        // What mcp-read expects
        let mi = p.info();
        assert!(mi.session && mi.can_seek);
        assert_eq!((mi.state, mi.player.as_str(), mi.duration, mi.position), (STATE_PAUSED, SIM_PLAYER, 18000, 0));
        assert!(mi.title.starts_with("Test track 1 \u{266A} \u{DC}n\u{EF}c\u{F6}d\u{E9} t\u{ED}tl\u{E9}"));
        // "Test track 1 ♪" (16 octets) + 5 x " Ünïcödé títlé" (20 octets)
        assert_eq!(mi.title.len(), 116);
        assert_eq!((mi.track, mi.playback_speed(), mi.opcodes_supported()), (1, 0, 0x1833));

        let run = |p: &mut SimulatedPlayer, op: u8, param: i32| {
            let r = Arc::new(AtomicU8::new(0));
            let r2 = r.clone();
            p.command(op, param, Box::new(move |x| r2.store(x, Ordering::SeqCst)));
            r.load(Ordering::SeqCst)
        };
        assert_eq!(run(&mut p, OP_PLAY, 0), RESULT_SUCCESS);
        assert_eq!(p.info().state, STATE_PLAYING);
        assert_eq!(run(&mut p, OP_PAUSE, 0), RESULT_SUCCESS);
        assert_eq!(p.info().state, STATE_PAUSED);
        assert_eq!(run(&mut p, OP_NEXT_TRACK, 0), RESULT_SUCCESS);
        assert!(p.info().title.starts_with("Test track 2 "));
        assert_eq!(p.info().track, 2);
        assert_eq!(run(&mut p, OP_MOVE_RELATIVE, 1500), RESULT_SUCCESS);
        assert_eq!(p.info().position, 1500);
        // Previous after 3 s: back to the start of the track
        assert_eq!(run(&mut p, OP_PREVIOUS_TRACK, 0), RESULT_SUCCESS);
        assert_eq!((p.info().track, p.info().position), (2, 0));
        // Previous at the start: the track before
        assert_eq!(run(&mut p, OP_PREVIOUS_TRACK, 0), RESULT_SUCCESS);
        assert_eq!((p.info().track, p.info().position), (1, 0));
        assert_eq!(run(&mut p, OP_PREVIOUS_TRACK, 0), RESULT_SUCCESS);
        assert_eq!(p.info().track, 1);
        assert_eq!(run(&mut p, OP_MOVE_RELATIVE, 99999), RESULT_SUCCESS);
        assert_eq!(p.info().position, 18000);
        assert_eq!(run(&mut p, OP_MOVE_RELATIVE, -99999), RESULT_SUCCESS);
        assert_eq!(p.info().position, 0);
        p.seek(5000);
        assert_eq!(p.info().position, 5000);
        assert_eq!(run(&mut p, OP_STOP, 0), RESULT_SUCCESS);
        assert_eq!((p.info().state, p.info().position), (STATE_PAUSED, 0));
        assert_eq!(run(&mut p, OP_FAST_FORWARD, 0), RESULT_OPCODE_NOT_SUPPORTED);
        assert!(changes.load(Ordering::SeqCst) >= 9);

        p.stop();
        assert_eq!(p.info(), MediaInfo::default());
        assert_eq!(run(&mut p, OP_PLAY, 0), RESULT_MEDIA_PLAYER_INACTIVE);
        // Started again: track 1
        p.start(Box::new(|| {}));
        assert_eq!(p.info().track, 1);
        assert!(format!("{p:?}").starts_with("SimulatedPlayer"));
    }
}
