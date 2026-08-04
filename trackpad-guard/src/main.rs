//! Disable sway touchpad while typing — evdev intercept (v4).
//!
//! Architecture: trackpad-guard sits between the real AMIRA touchpad
//! and libinput. We do NOT use `swaymsg ... events disabled` anymore,
//! because that flow killed in-progress touches in libinput's view and
//! forced the user to lift+retouch to recover.
//!
//! Instead:
//! 1. A udev rule tags the AMIRA touchpad evdev nodes with
//!    LIBINPUT_IGNORE_DEVICE=1 so libinput refuses to open them.
//! 2. We open + EVIOCGRAB the real touchpad nodes ourselves.
//! 3. We create a virtual uinput device that mirrors the real touchpad's
//!    capabilities (vid:pid, keys, relative axes, properties).
//! 4. Each event batch from the real touchpad is forwarded byte-for-byte
//!    to the virtual device — UNLESS a keyboard event has been seen in
//!    the last KEY_TYPING_WINDOW (185 ms), in which case the batch is
//!    silently dropped.
//!
//! From sway/libinput's perspective there's only the virtual device,
//! and it never changes state — no send_events toggles, no
//! "touch cancelled" events. So mid-typing brushes on the trackpad
//! simply do nothing; the moment typing stops, the next motion event
//! flows through naturally.
//!
//! SIGUSR1 still triggers a manual USB unbind/rebind on the AMIRA
//! touchpad interface (vid:pid 6080:8061), bound to Mod+Shift+T.

use evdev::{
    uinput::VirtualDevice, AbsoluteAxisCode, Device, EventSummary, EventType, InputEvent, KeyCode,
    UinputAbsSetup,
};
use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM, SIGUSR1, SIGUSR2};
use signal_hook::iterator::Signals;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const VENDOR_ID: u16 = 0x6080;
const KEYBOARD_NAME: &str = "AMIRA-KEYBOAR USB KEYBOARD";
/// The AMIRA's mouse-class evdev node has one of these names depending on
/// which kernel HID driver is bound to the interface (hid-generic vs
/// hid-multitouch). We open whichever appears.
const TOUCHPAD_NAMES: &[&str] = &[
    "AMIRA-KEYBOAR USB KEYBOARD Touchpad",
    "AMIRA-KEYBOAR USB KEYBOARD Mouse",
];
/// Name we give the virtual replacement device. Deliberately distinct
/// from the real names so the LIBINPUT_IGNORE_DEVICE udev rule (which
/// matches the real names) doesn't apply to our virtual.
const VIRTUAL_NAME: &str = "trackpad-guard AMIRA touchpad";

/// USB ids used by the SIGUSR1-driven manual rebind path.
const TOUCHPAD_USB_PRODUCT: &str = "8061";
const TOUCHPAD_USB_VENDOR_STR: &str = "6080";

/// Touchpad events arriving within this window of the most recent
/// keyboard event are dropped (typing → palm protection).
const KEY_TYPING_WINDOW: Duration = Duration::from_millis(185);

const DISCOVER_INTERVAL: Duration = Duration::from_secs(2);

/// Phantom-contact / stuck-stream filter. Catches failure modes where
/// the AMIRA touchpad delivers a steady event stream that translates to
/// no cursor motion — observed across multiple stuck reports on
/// 2026-05-15:
///   - Classic phantom: BTN_TOUCH=1 held for minutes, position frozen.
///   - Jittery phantom: BTN_TOUCH=1 held, position wandering by 5-7 units
///     around a center point (not enough to move cursor visibly).
///   - Flicker stuck: BTN_TOUCH rapidly toggling but position never
///     escaping a small region.
///
/// Earlier versions of this filter gated on BTN_TOUCH=1 — that missed
/// the flicker case because each touch_down reset the stationary timer.
/// The current logic is BTN_TOUCH-independent: it tracks how long the
/// reported (ABS_X, ABS_Y) has stayed within PHANTOM_NOISE of a fixed
/// reference, requires a high recent batch rate (so we only fire when
/// the touchpad is actively reporting, not when the user is idle), and
/// triggers recovery when the position has been stuck for PHANTOM_HOLD.
///
/// Recovery is two-stage:
///   (a) emit a synthetic MT slot-0 release into the virtual device so
///       libinput's state machine sees the finger lift, and
///   (b) issue a USB rebind on the touchpad interface (subject to
///       PHANTOM_REBIND_COOLDOWN). Without the rebind the firmware
///       keeps emitting on the stuck tracking ID and libinput refuses
///       those events as a new contact; the rebind clears the firmware
///       state.
/// Minimum time the reported position must stay within PHANTOM_NOISE
/// before we declare the contact phantom. Tradeoff axis: lower values
/// catch shorter stuck events but false-positive on legitimate user
/// pauses (e.g. resting a finger mid-cursor-motion, peaks of slow
/// circles). 2s was demonstrably too low — fired repeatedly during
/// normal touchpad use, causing the "stuck for ~1s then cursor catches
/// up with a jitter" symptom (the suppress window persisting until
/// motion exceeds PHANTOM_BREAK). The original multi-minute phantom
/// events would still be caught at 10s, just with a longer initial
/// wait. Sub-10s stuck events the user has historically observed
/// self-resolved without intervention.
const PHANTOM_HOLD: Duration = Duration::from_secs(10);
/// Position drift below this threshold counts as "still." 4 units
/// (~0.3mm at 14 units/mm) is below typical finger jitter for
/// intentional motion but above pure sensor noise. Earlier value of 8u
/// missed jittery phantoms whose position wandered 5-7u while the cursor
/// was clearly frozen.
const PHANTOM_NOISE: i32 = 4;
/// Motion this large exits phantom-suppress mode — the user clearly
/// moved a real finger with intent.
const PHANTOM_BREAK: i32 = 40;
/// Minimum batches in the last RATE_WINDOW to count as "actively
/// reporting." Below this we treat the touchpad as idle and won't fire
/// regardless of how long the position has been stationary — protects
/// against false positives when a single stale position read sits in
/// the buffer while the user does nothing.
const PHANTOM_MIN_RATE_BATCHES: usize = 30;
const PHANTOM_RATE_WINDOW: Duration = Duration::from_secs(2);
/// Cooldown between phantom-triggered USB rebinds. Shorter than the
/// wedge-watchdog cooldown (30s) because phantom faults can come in
/// bursts as firmware recovers and re-enters the bad state — but long
/// enough that we don't burn the bus with rebinds on every flutter.
const PHANTOM_REBIND_COOLDOWN: Duration = Duration::from_secs(15);

#[derive(Debug)]
enum PhantomAction {
    /// Forward this batch normally.
    Forward,
    /// Suppress this batch — we are already in phantom mode.
    Drop,
    /// First batch we've classified as phantom. Caller should emit a
    /// synthetic release into the virtual device + trigger a USB rebind,
    /// then drop this batch.
    EmitReleaseAndDrop,
}

struct PhantomGuard {
    /// Reference position for the current "stationary window." Updated
    /// whenever position drifts beyond PHANTOM_NOISE.
    ref_x: i32,
    ref_y: i32,
    ref_known: bool,
    /// When the position last moved beyond PHANTOM_NOISE. None until the
    /// first position read.
    stationary_since: Option<Instant>,
    /// Timestamps of recent batches, used to compute the "actively
    /// reporting" rate gate. Trimmed to PHANTOM_RATE_WINDOW on each
    /// observe() call.
    recent_batches: VecDeque<Instant>,
    /// True once we've declared phantom for this stuck event. Cleared
    /// when a real BTN_TOUCH=0 arrives or motion exceeds PHANTOM_BREAK.
    active: bool,
}

impl PhantomGuard {
    fn new() -> Self {
        Self {
            ref_x: 0,
            ref_y: 0,
            ref_known: false,
            stationary_since: None,
            recent_batches: VecDeque::new(),
            active: false,
        }
    }

    /// Drop all internal state. Called after a rebind so the next
    /// device's events start with a clean slate instead of comparing
    /// against pre-rebind positions.
    fn reset(&mut self) {
        self.ref_known = false;
        self.stationary_since = None;
        self.recent_batches.clear();
        self.active = false;
    }

    fn observe(&mut self, batch: &[InputEvent], now: Instant) -> PhantomAction {
        // Trim and record the batch-rate window.
        while let Some(&front) = self.recent_batches.front() {
            if now.duration_since(front) > PHANTOM_RATE_WINDOW {
                self.recent_batches.pop_front();
            } else {
                break;
            }
        }
        self.recent_batches.push_back(now);

        let mut touch_up = false;
        let mut new_x: Option<i32> = None;
        let mut new_y: Option<i32> = None;
        for ev in batch {
            match ev.destructure() {
                EventSummary::Key(_, code, value) => {
                    if code == KeyCode::BTN_TOUCH && value == 0 {
                        touch_up = true;
                    }
                }
                EventSummary::AbsoluteAxis(_, code, value) => {
                    if code == AbsoluteAxisCode::ABS_X {
                        new_x = Some(value);
                    } else if code == AbsoluteAxisCode::ABS_Y {
                        new_y = Some(value);
                    }
                }
                _ => {}
            }
        }

        if touch_up {
            // Hardware reported a real release — clean slate. Forward so
            // libinput sees the lift.
            self.ref_known = false;
            self.stationary_since = None;
            self.active = false;
            return PhantomAction::Forward;
        }

        // Update reference position / stationary timer from any position
        // events in this batch.
        if let (Some(x), Some(y)) = (
            new_x.or_else(|| self.ref_known.then_some(self.ref_x)),
            new_y.or_else(|| self.ref_known.then_some(self.ref_y)),
        ) {
            if !self.ref_known {
                self.ref_x = x;
                self.ref_y = y;
                self.ref_known = true;
                self.stationary_since = Some(now);
            } else if new_x.is_some() || new_y.is_some() {
                let dx = (x - self.ref_x).abs();
                let dy = (y - self.ref_y).abs();
                if dx > PHANTOM_NOISE || dy > PHANTOM_NOISE {
                    self.ref_x = x;
                    self.ref_y = y;
                    self.stationary_since = Some(now);
                    if self.active && (dx > PHANTOM_BREAK || dy > PHANTOM_BREAK) {
                        // Intentional motion → exit phantom mode and let
                        // libinput pick up the new position.
                        self.active = false;
                        return PhantomAction::Forward;
                    }
                }
            }
        }

        if self.active {
            return PhantomAction::Drop;
        }

        // Detection requires (a) the position has been stuck long enough
        // and (b) the touchpad is actively reporting at a high rate.
        // The rate gate is the key protection against false positives on
        // idle: if the user isn't touching, batches don't arrive, and we
        // don't fire regardless of how stale the ref position is.
        let stuck_long_enough = self
            .stationary_since
            .is_some_and(|t| now.duration_since(t) >= PHANTOM_HOLD);
        let actively_reporting = self.recent_batches.len() >= PHANTOM_MIN_RATE_BATCHES;
        if stuck_long_enough && actively_reporting {
            self.active = true;
            return PhantomAction::EmitReleaseAndDrop;
        }

        PhantomAction::Forward
    }
}

/// Emit a synthetic slot-0 release into the virtual device. This is what
/// the AMIRA firmware would send if it had noticed the finger lift —
/// ABS_MT_TRACKING_ID=-1 retires the slot, then BTN_TOUCH=0 +
/// BTN_TOOL_FINGER=0 tell libinput no fingers remain. SYN_REPORT is added
/// automatically by VirtualDevice::emit. Only slot 0 is released: every
/// phantom event we've observed so far is single-touch, and aggressively
/// releasing higher slots could disrupt legitimate multi-touch gestures.
fn emit_phantom_release(virt: &mut VirtualDevice) -> std::io::Result<()> {
    let events = [
        InputEvent::new(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_MT_SLOT.0, 0),
        InputEvent::new(
            EventType::ABSOLUTE.0,
            AbsoluteAxisCode::ABS_MT_TRACKING_ID.0,
            -1,
        ),
        InputEvent::new(EventType::KEY.0, KeyCode::BTN_TOUCH.0, 0),
        InputEvent::new(EventType::KEY.0, KeyCode::BTN_TOOL_FINGER.0, 0),
    ];
    virt.emit(&events)
}

/// Maximum MT slots we model. The AMIRA reports far fewer (typically up
/// to 5); we size generously and ignore any slot index at or above this.
const MAX_SLOTS: usize = 16;

/// A finger slot is treated as a "ghost" — and force-released — once it
/// has held an open tracking id this long with no position update, *while
/// at least one other slot is still actively moving*. The "other slot is
/// moving" requirement is what makes this safe: it fires only in the exact
/// situation that freezes the cursor — the system believes 2+ fingers are
/// down (so it locks into scroll/gesture mode and stops moving the
/// pointer) when really only one finger is present, because the AMIRA
/// dropped a finger-up report (commonly when a Mod+digit workspace-switch
/// chord collides with touch reporting). Real two-finger scrolls and
/// pinches keep both slots updating, so neither goes stale. Tradeoff: a
/// finger deliberately held perfectly still for >STALE_SLOT_TIMEOUT while
/// the other moves a lot would be released early; that gesture is rare and
/// a stuck cursor is never acceptable, so we err toward releasing.
const STALE_SLOT_TIMEOUT: Duration = Duration::from_millis(1500);

/// A slot left open while our forwarded view says nothing is touching
/// (BTN_TOUCH=0 and no BTN_TOOL_* set) for longer than this is a "lifted
/// ghost": the finger came up but its ABS_MT_TRACKING_ID=-1 release was
/// lost, so libinput still holds the contact and freezes the pointer with
/// no further events arriving. Unlike STALE_SLOT_TIMEOUT this needs no
/// second moving slot — a LONE ghost after a lift is the exact case (#13)
/// that PhantomGuard can't see (it requires an active contact). Checked on
/// the watchdog tick so an idle ghost still clears. Short, because once the
/// pad reports no contact any surviving slot is unambiguously wrong; the
/// small grace only avoids racing a lift whose release lands a beat later.
const LIFTED_GHOST_TIMEOUT: Duration = Duration::from_millis(500);

/// Tracks the MT slot state of the stream we forward to the virtual
/// device, so we can spot a slot whose finger-up was lost and synthesize
/// the missing release. Fed exclusively from forwarded batches, so it
/// mirrors what libinput actually believes — including a lift we may have
/// dropped inside the typing gate.
struct SlotGuard {
    /// Sticky "current slot" pointer, mirroring evdev MT protocol type B:
    /// ABS_MT_SLOT only re-appears when it changes, so this persists across
    /// batches.
    current_slot: usize,
    /// Per slot: Some(last_position_update) while the slot holds an open
    /// tracking id; None when released or never touched.
    active: [Option<Instant>; MAX_SLOTS],
    // --- contact-count state as libinput sees it (from forwarded events) ---
    // Originally diagnostic-only; btn_touch + the tool_* flags now ALSO drive
    // lifted_ghost_slots(): when they say "nothing touching" yet a slot is
    // still open, that slot is a ghost to release. Still logged on every
    // transition so a stuck-but-forwarding window (cursor frozen while fwd>0)
    // stays debuggable. See notes/trackpad-guard-engineering-log.md.
    /// Last BTN_TOUCH value forwarded (Some(1)=contact, Some(0)=lifted).
    btn_touch: Option<i32>,
    /// BTN_TOOL_* finger-count flags as libinput currently sees them.
    tool_finger: bool,
    tool_doubletap: bool,
    tool_tripletap: bool,
    tool_quadtap: bool,
    tool_quinttap: bool,
}

impl SlotGuard {
    fn new() -> Self {
        Self {
            current_slot: 0,
            active: [None; MAX_SLOTS],
            btn_touch: None,
            tool_finger: false,
            tool_doubletap: false,
            tool_tripletap: false,
            tool_quadtap: false,
            tool_quinttap: false,
        }
    }

    /// Drop all state — used after a USB rebind, since the post-rebind
    /// device starts with a clean slot state and our model must not carry
    /// pre-rebind ghosts forward.
    fn reset(&mut self) {
        self.current_slot = 0;
        self.active = [None; MAX_SLOTS];
        self.btn_touch = None;
        self.tool_finger = false;
        self.tool_doubletap = false;
        self.tool_tripletap = false;
        self.tool_quadtap = false;
        self.tool_quinttap = false;
    }

    /// One-line snapshot of what libinput currently believes: active MT
    /// slots (with how long since each was updated), the BTN_TOOL finger
    /// count, and last BTN_TOUCH. Appended to the heartbeat so a frozen
    /// window shows the finger-count state at the time.
    fn diag_snapshot(&self, now: Instant) -> String {
        let mut slots = String::new();
        for (i, s) in self.active.iter().enumerate() {
            if let Some(t) = s {
                if !slots.is_empty() {
                    slots.push(',');
                }
                slots.push_str(&format!("{}:{}ms", i, now.duration_since(*t).as_millis()));
            }
        }
        if slots.is_empty() {
            slots.push_str("none");
        }
        let mut tool = String::new();
        for (on, ch) in [
            (self.tool_finger, 'F'),
            (self.tool_doubletap, 'D'),
            (self.tool_tripletap, 'T'),
            (self.tool_quadtap, 'Q'),
            (self.tool_quinttap, '5'),
        ] {
            if on {
                tool.push(ch);
            }
        }
        if tool.is_empty() {
            tool.push_str("none");
        }
        format!("slots=[{slots}] tool={tool} touch={:?}", self.btn_touch)
    }

    /// Record that we ourselves retired `slot` (synthetic release), so the
    /// model stays consistent with what libinput now sees.
    fn note_released(&mut self, slot: usize) {
        if slot < MAX_SLOTS {
            self.active[slot] = None;
        }
    }

    /// Slots we currently believe libinput holds open.
    fn open_slots(&self) -> Vec<usize> {
        self.active
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.map(|_| i))
            .collect()
    }

    /// Record that we forced the contact count to zero (SIGUSR2 resync frame 1).
    fn note_all_lifted(&mut self) {
        self.btn_touch = Some(0);
        self.tool_finger = false;
        self.tool_doubletap = false;
        self.tool_tripletap = false;
        self.tool_quadtap = false;
        self.tool_quinttap = false;
    }

    /// Record that we synthesized a single fresh contact on `slot` (SIGUSR2
    /// resync frame 2), so the model matches what libinput was just told.
    fn note_landed(&mut self, slot: usize, now: Instant) {
        if slot < MAX_SLOTS {
            self.active[slot] = Some(now);
        }
        self.btn_touch = Some(1);
        self.tool_finger = true;
    }

    /// Update the slot model from a forwarded batch and return any slots
    /// that have gone stale and should be force-released. `now` is the
    /// batch's kernel emit time.
    fn observe(&mut self, batch: &[InputEvent], now: Instant) -> Vec<usize> {
        for ev in batch {
            if let EventSummary::AbsoluteAxis(_, code, value) = ev.destructure() {
                if code == AbsoluteAxisCode::ABS_MT_SLOT {
                    if value >= 0 && (value as usize) < MAX_SLOTS {
                        self.current_slot = value as usize;
                    }
                } else if code == AbsoluteAxisCode::ABS_MT_TRACKING_ID {
                    if self.current_slot < MAX_SLOTS {
                        self.active[self.current_slot] = if value >= 0 { Some(now) } else { None };
                    }
                } else if (code == AbsoluteAxisCode::ABS_MT_POSITION_X
                    || code == AbsoluteAxisCode::ABS_MT_POSITION_Y)
                    && self.current_slot < MAX_SLOTS
                    && self.active[self.current_slot].is_some()
                {
                    self.active[self.current_slot] = Some(now);
                }
            }
            // Diagnostic-only: track BTN_TOUCH + BTN_TOOL_* as libinput sees
            // them and log every finger-count transition, so a dropped
            // finger-up (the suspected uncaught freeze cause) is visible.
            if let EventSummary::Key(_, code, value) = ev.destructure() {
                let on = value != 0;
                if code == KeyCode::BTN_TOUCH {
                    self.btn_touch = Some(value);
                } else if code == KeyCode::BTN_TOOL_FINGER {
                    note_tool("BTN_TOOL_FINGER", &mut self.tool_finger, on);
                } else if code == KeyCode::BTN_TOOL_DOUBLETAP {
                    note_tool("BTN_TOOL_DOUBLETAP", &mut self.tool_doubletap, on);
                } else if code == KeyCode::BTN_TOOL_TRIPLETAP {
                    note_tool("BTN_TOOL_TRIPLETAP", &mut self.tool_tripletap, on);
                } else if code == KeyCode::BTN_TOOL_QUADTAP {
                    note_tool("BTN_TOOL_QUADTAP", &mut self.tool_quadtap, on);
                } else if code == KeyCode::BTN_TOOL_QUINTTAP {
                    note_tool("BTN_TOOL_QUINTTAP", &mut self.tool_quinttap, on);
                }
            }
        }

        // Only act when at least one slot is still actively moving — that's
        // the slot the user is trying to move the cursor with. Without a
        // live slot there's no cursor being held hostage (a single held
        // finger is PhantomGuard's job, not ours), so we never fire.
        let any_fresh = self
            .active
            .iter()
            .any(|s| s.is_some_and(|t| now.duration_since(t) <= STALE_SLOT_TIMEOUT));
        if !any_fresh {
            return Vec::new();
        }
        let mut stale = Vec::new();
        for (i, s) in self.active.iter().enumerate() {
            if s.is_some_and(|t| now.duration_since(t) > STALE_SLOT_TIMEOUT) {
                stale.push(i);
            }
        }
        stale
    }

    /// Slots libinput still holds open while our forwarded view says nothing
    /// is touching (BTN_TOUCH=0 and no BTN_TOOL_* set) and the slot has been
    /// silent longer than `grace`. These are "lifted ghosts": the finger-up's
    /// ABS_MT_TRACKING_ID=-1 was lost, so libinput freezes the pointer while
    /// zero further events arrive. Distinct from observe()'s stale path,
    /// which needs a second moving slot; this fires on a LONE ghost. Meant to
    /// be polled from the watchdog. A synthetic release (emit_slot_releases)
    /// resyncs libinput; no USB rebind is needed because the firmware already
    /// believes the finger is up (it sent BTN_TOUCH=0) and isn't streaming.
    fn lifted_ghost_slots(&self, now: Instant, grace: Duration) -> Vec<usize> {
        // Only when the forwarded contact count is unambiguously zero.
        if self.btn_touch != Some(0) {
            return Vec::new();
        }
        if self.tool_finger
            || self.tool_doubletap
            || self.tool_tripletap
            || self.tool_quadtap
            || self.tool_quinttap
        {
            return Vec::new();
        }
        self.active
            .iter()
            .enumerate()
            .filter_map(|(i, s)| match s {
                Some(t) if now.duration_since(*t) > grace => Some(i),
                _ => None,
            })
            .collect()
    }
}

/// Diagnostic helper: log a BTN_TOOL_* finger-count transition and update
/// the tracked flag. Only fires on an actual change, so it stays quiet
/// during steady state.
fn note_tool(name: &str, flag: &mut bool, on: bool) {
    if *flag != on {
        eprintln!(
            "trackpad-guard: [diag] {name} {} -> {}",
            *flag as u8, on as u8
        );
        *flag = on;
    }
}

/// True if this batch CHANGES the contact count as libinput tracks it — in
/// EITHER direction: a finger landing (BTN_TOUCH=1, BTN_TOOL_*=1, or a new MT
/// slot via ABS_MT_TRACKING_ID>=0) or a finger lifting (BTN_TOUCH=0,
/// BTN_TOOL_*=0, ABS_MT_TRACKING_ID=-1).
///
/// Such a batch MUST be forwarded even while the typing gate is dropping.
/// The gate's job is palm protection — suppressing touch *motion* during
/// typing — but a finger going down or up is not motion. Dropping a lift
/// leaves libinput believing a finger is still down (count stuck one HIGH:
/// one real finger reads as two — observed 2026-06-10). Dropping a landing
/// leaves libinput never seeing the finger arrive (count stuck one LOW: two
/// fingers read as one, one finger does nothing — observed 2026-06-17). Both
/// are the same contact-count desync, opposite directions; forwarding any
/// count-changing batch keeps the count honest while still dropping
/// motion-only batches, so palm protection is unchanged. See
/// notes/trackpad-guard-engineering-log.md.
///
/// Note: BTN_TOUCH / BTN_TOOL_* / ABS_MT_TRACKING_ID are emitted by the
/// kernel only on a transition, so matching them (regardless of value)
/// selects exactly the count-changing batches; pure-motion batches carry
/// only ABS_MT_SLOT / ABS_MT_POSITION_* and stay droppable.
fn batch_changes_contact(batch: &[InputEvent]) -> bool {
    batch.iter().any(|ev| match ev.destructure() {
        EventSummary::Key(_, code, _value) => {
            code == KeyCode::BTN_TOUCH
                || code == KeyCode::BTN_TOOL_FINGER
                || code == KeyCode::BTN_TOOL_DOUBLETAP
                || code == KeyCode::BTN_TOOL_TRIPLETAP
                || code == KeyCode::BTN_TOOL_QUADTAP
                || code == KeyCode::BTN_TOOL_QUINTTAP
        }
        EventSummary::AbsoluteAxis(_, code, _value) => code == AbsoluteAxisCode::ABS_MT_TRACKING_ID,
        _ => false,
    })
}

/// Force-release the given ghost MT slots in the virtual device, then
/// restore the slot pointer to `restore_slot` (where the real stream left
/// it) so the next forwarded batch — which only re-sends ABS_MT_SLOT when
/// it changes — keeps routing to the live finger. BTN_TOUCH is left
/// untouched: a real finger is still down, so the contact count drops from
/// N to N-1, not to zero. All emitted in one SYN frame.
fn emit_slot_releases(
    virt: &mut VirtualDevice,
    slots: &[usize],
    restore_slot: usize,
) -> std::io::Result<()> {
    let mut events = Vec::with_capacity(slots.len() * 2 + 1);
    for &s in slots {
        events.push(InputEvent::new(
            EventType::ABSOLUTE.0,
            AbsoluteAxisCode::ABS_MT_SLOT.0,
            s as i32,
        ));
        events.push(InputEvent::new(
            EventType::ABSOLUTE.0,
            AbsoluteAxisCode::ABS_MT_TRACKING_ID.0,
            -1,
        ));
    }
    events.push(InputEvent::new(
        EventType::ABSOLUTE.0,
        AbsoluteAxisCode::ABS_MT_SLOT.0,
        restore_slot as i32,
    ));
    virt.emit(&events)
}

/// Tracking id used for a synthetic re-landing. Real ids come from the AMIRA
/// firmware and are small sequential integers, so a high fixed value cannot
/// collide with a live one. libinput only cares that the id is *different*
/// from the one it just saw released.
const RESYNC_TRACKING_ID: i32 = 0x4000;

/// SIGUSR2 rescue: force libinput's contact state back in sync with reality
/// WITHOUT touching USB (no unbind/rebind, so nothing can hang and the pointer
/// can never disappear — the failure mode SIGUSR1 risks).
///
/// Two frames:
///   1. Release every slot we believe open — plus slots 0 and 1 unconditionally,
///      because the whole premise of this rescue is that our model and libinput
///      DISAGREE, so "slots we believe open" may be exactly the wrong list. The
///      AMIRA reports DOUBLETAP, so 0 and 1 are always in range; duplicate
///      releases on an already-released slot are dropped by the input core.
///      BTN_TOUCH and every BTN_TOOL_* go to 0, clearing any stuck finger count.
///   2. If we believe a finger is still physically down, re-land exactly ONE
///      contact with a FRESH tracking id. This second frame is the point of the
///      whole thing: libinput ignores motion on a slot it considers released, so
///      frame 1 alone would leave the held finger dead until the user lifted and
///      re-landed — which is the workaround we're trying to replace. Re-landing
///      one finger also directly corrects the suspected fault (libinput believing
///      more contacts than exist). No position is sent: the kernel retains the
///      slot's last position, and the next real motion batch updates it.
///
/// Deliberately re-lands ONE contact only. If two fingers really are down, the
/// second one's motion is ignored until it lifts — acceptable for a manual
/// rescue, and far safer than inventing contacts that aren't there.
fn emit_contact_resync(
    virt: &mut VirtualDevice,
    slots: &mut SlotGuard,
    now: Instant,
) -> std::io::Result<Option<usize>> {
    let mut release: Vec<usize> = (0..MAX_SLOTS)
        .filter(|i| slots.active[*i].is_some())
        .collect();
    for s in [0usize, 1] {
        if !release.contains(&s) {
            release.push(s);
        }
    }

    let mut frame = Vec::with_capacity(release.len() * 2 + 6);
    for &s in &release {
        frame.push(InputEvent::new(
            EventType::ABSOLUTE.0,
            AbsoluteAxisCode::ABS_MT_SLOT.0,
            s as i32,
        ));
        frame.push(InputEvent::new(
            EventType::ABSOLUTE.0,
            AbsoluteAxisCode::ABS_MT_TRACKING_ID.0,
            -1,
        ));
    }
    frame.push(InputEvent::new(EventType::KEY.0, KeyCode::BTN_TOUCH.0, 0));
    for code in [
        KeyCode::BTN_TOOL_FINGER,
        KeyCode::BTN_TOOL_DOUBLETAP,
        KeyCode::BTN_TOOL_TRIPLETAP,
        KeyCode::BTN_TOOL_QUADTAP,
        KeyCode::BTN_TOOL_QUINTTAP,
    ] {
        frame.push(InputEvent::new(EventType::KEY.0, code.0, 0));
    }
    virt.emit(&frame)?;

    // Did a finger appear to be down before we cleared everything? Use the
    // slot the real stream is currently addressing so subsequent motion-only
    // batches (which re-send ABS_MT_SLOT only when it changes) land on it.
    let was_down = slots.btn_touch == Some(1) || !slots.open_slots().is_empty();
    for s in &release {
        slots.note_released(*s);
    }
    slots.note_all_lifted();

    if !was_down {
        return Ok(None);
    }
    let slot = slots.current_slot.min(MAX_SLOTS - 1);
    virt.emit(&[
        InputEvent::new(
            EventType::ABSOLUTE.0,
            AbsoluteAxisCode::ABS_MT_SLOT.0,
            slot as i32,
        ),
        InputEvent::new(
            EventType::ABSOLUTE.0,
            AbsoluteAxisCode::ABS_MT_TRACKING_ID.0,
            RESYNC_TRACKING_ID,
        ),
        InputEvent::new(EventType::KEY.0, KeyCode::BTN_TOUCH.0, 1),
        InputEvent::new(EventType::KEY.0, KeyCode::BTN_TOOL_FINGER.0, 1),
    ])?;
    slots.note_landed(slot, now);
    Ok(Some(slot))
}

enum Msg {
    /// A keyboard event batch arrived. The timestamp is captured in the
    /// reader thread the moment fetch_events() returns, so it reflects
    /// when the kernel emitted the event — NOT when the main loop gets
    /// around to processing the message. Using processing time would
    /// drift under load: a backlog can make us think a stale keystroke
    /// happened "now" and incorrectly gate touchpad events that are
    /// actually well past the typing window.
    KeyActivity {
        at: Instant,
    },
    TouchpadEvents {
        events: Vec<InputEvent>,
        at: Instant,
    },
    KeyboardReaderDied,
    /// A touchpad reader thread exited (typically ENODEV from fetch_events
    /// when the underlying USB device drops or its evdev node is
    /// invalidated). Carries the device path so the main loop can drop
    /// just that entry from the active set and schedule a rescan to
    /// re-grab whatever node reappears.
    TouchpadReaderDied {
        path: PathBuf,
    },
    ManualRebind,
    /// SIGUSR2: synthesize an "all contacts up" frame to the virtual device.
    /// The light counterpart to ManualRebind — see emit_contact_resync().
    ManualResync,
    Shutdown,
}

fn matches_keyboard(device: &Device) -> bool {
    device.input_id().vendor() == VENDOR_ID && device.name() == Some(KEYBOARD_NAME)
}

fn matches_touchpad(device: &Device) -> bool {
    if device.input_id().vendor() != VENDOR_ID {
        return false;
    }
    let Some(name) = device.name() else {
        return false;
    };
    TOUCHPAD_NAMES.contains(&name)
}

fn find_keyboards() -> Vec<(PathBuf, Device)> {
    evdev::enumerate()
        .filter(|(_, d)| matches_keyboard(d))
        .collect()
}

fn find_touchpads() -> Vec<(PathBuf, Device)> {
    evdev::enumerate()
        .filter(|(_, d)| matches_touchpad(d))
        .collect()
}

fn spawn_keyboard_reader(mut device: Device, tx: mpsc::Sender<Msg>) {
    thread::spawn(move || loop {
        match device.fetch_events() {
            Ok(events) => {
                let at = Instant::now();
                let mut had_transition = false;
                for ev in events {
                    if let EventSummary::Key(_, _, value) = ev.destructure() {
                        // Only real presses (value=1) and releases
                        // (value=0) count as "typing." Autorepeat
                        // (value=2) is *deliberately* ignored: the
                        // AMIRA drops release events often enough that
                        // counting autorepeats as activity would let a
                        // single missed release lock the touchpad off
                        // forever — the kernel keeps autorepeating
                        // until we get a fresh release. This matches
                        // the original Rust port's design.
                        if value == 0 || value == 1 {
                            had_transition = true;
                        }
                    }
                }
                if had_transition && tx.send(Msg::KeyActivity { at }).is_err() {
                    return;
                }
            }
            Err(_) => {
                let _ = tx.send(Msg::KeyboardReaderDied);
                return;
            }
        }
    });
}

fn spawn_touchpad_reader(path: PathBuf, mut device: Device, tx: mpsc::Sender<Msg>) {
    thread::spawn(move || loop {
        match device.fetch_events() {
            Ok(events) => {
                let at = Instant::now();
                let batch: Vec<InputEvent> = events.collect();
                if !batch.is_empty() && tx.send(Msg::TouchpadEvents { events: batch, at }).is_err()
                {
                    return;
                }
            }
            Err(_) => {
                let _ = tx.send(Msg::TouchpadReaderDied { path });
                return;
            }
        }
    });
}

/// Dedupe candidate touchpads the same way initial discovery does: for
/// each USB product id, prefer the "Touchpad"-named device over the
/// "Mouse"-named one. Used by both first-time setup and the rescan path.
fn dedupe_touchpads(touchpads: Vec<(PathBuf, Device)>) -> Vec<(PathBuf, Device)> {
    let mut chosen: HashMap<u16, (PathBuf, Device)> = HashMap::new();
    for (path, dev) in touchpads {
        let product = dev.input_id().product();
        let is_touchpad = dev.name() == Some("AMIRA-KEYBOAR USB KEYBOARD Touchpad");
        let prefer_new = match chosen.get(&product) {
            None => true,
            Some(existing) => {
                is_touchpad && existing.1.name() != Some("AMIRA-KEYBOAR USB KEYBOARD Touchpad")
            }
        };
        if prefer_new {
            chosen.insert(product, (path, dev));
        }
    }
    chosen.into_values().collect()
}

/// Re-enumerate /dev/input, grab any AMIRA touchpad we don't already
/// hold, and spawn a reader thread for it. Returns the number of newly
/// acquired devices. Safe to call repeatedly — devices already in
/// `active` are skipped, so this is the same primitive used both at
/// startup and on recovery.
fn try_acquire_missing(active: &mut HashSet<PathBuf>, tx: &mpsc::Sender<Msg>) -> usize {
    let mut acquired = 0;
    for (path, mut dev) in dedupe_touchpads(find_touchpads()) {
        if active.contains(&path) {
            continue;
        }
        match dev.grab() {
            Ok(()) => {
                eprintln!("trackpad-guard: (re)grabbed {}", path.display());
                spawn_touchpad_reader(path.clone(), dev, tx.clone());
                active.insert(path);
                acquired += 1;
            }
            Err(e) => {
                eprintln!(
                    "trackpad-guard: grab {} failed during rescan: {e}",
                    path.display()
                );
            }
        }
    }
    acquired
}

/// Build a uinput virtual device that mirrors the capabilities of the
/// given real device. Sway/libinput will discover this and treat it as
/// the actual touchpad (the real one is hidden via LIBINPUT_IGNORE_DEVICE).
fn create_virtual_touchpad(real: &Device) -> std::io::Result<VirtualDevice> {
    let mut builder = VirtualDevice::builder()?
        .name(VIRTUAL_NAME)
        .input_id(real.input_id());

    if let Some(keys) = real.supported_keys() {
        builder = builder.with_keys(keys)?;
    }
    if let Some(rel) = real.supported_relative_axes() {
        builder = builder.with_relative_axes(rel)?;
    }
    // Mirror absolute axes one-by-one — libinput needs the ABS_X/ABS_Y +
    // resolution info to classify the device as a touchpad rather than a
    // plain mouse, and to compute pointer acceleration correctly.
    if let Ok(absinfo_iter) = real.get_absinfo() {
        for (code, info) in absinfo_iter {
            builder = builder.with_absolute_axis(&UinputAbsSetup::new(code, info))?;
        }
    }
    if let Some(msc) = real.misc_properties() {
        builder = builder.with_msc(msc)?;
    }
    let props = real.properties();
    if props.iter().next().is_some() {
        builder = builder.with_properties(props)?;
    }
    builder.build()
}

/// True if the AMIRA touchpad's USB device is currently bound to the
/// `usb` driver — i.e., the kernel sees it on the bus and the port
/// directory has an `idProduct` of 0x8061 with our vendor. Used as the
/// authoritative "device is really gone" signal for the wedge watchdog,
/// since plain event silence can just mean the user isn't touching the
/// trackpad. Reads a few small sysfs files, no syscalls into hotpath.
fn touchpad_is_bound_to_bus() -> bool {
    let drivers = match std::fs::read_dir("/sys/bus/usb/drivers/usb/") {
        Ok(d) => d,
        Err(_) => return true, // be conservative — don't false-positive a rebind
    };
    for entry in drivers.flatten() {
        let path = entry.path();
        let vendor = std::fs::read_to_string(path.join("idVendor"))
            .ok()
            .map(|s| s.trim().to_string());
        let product = std::fs::read_to_string(path.join("idProduct"))
            .ok()
            .map(|s| s.trim().to_string());
        if vendor.as_deref() == Some(TOUCHPAD_USB_VENDOR_STR)
            && product.as_deref() == Some(TOUCHPAD_USB_PRODUCT)
        {
            return true;
        }
    }
    false
}

fn find_touchpad_usb_id() -> Option<String> {
    for entry in std::fs::read_dir("/sys/bus/usb/devices/").ok()?.flatten() {
        let path = entry.path();
        let vendor = std::fs::read_to_string(path.join("idVendor"))
            .ok()
            .map(|s| s.trim().to_string());
        let product = std::fs::read_to_string(path.join("idProduct"))
            .ok()
            .map(|s| s.trim().to_string());
        if vendor.as_deref() == Some(TOUCHPAD_USB_VENDOR_STR)
            && product.as_deref() == Some(TOUCHPAD_USB_PRODUCT)
        {
            return Some(entry.file_name().to_string_lossy().into_owned());
        }
    }
    None
}

fn write_sysfs_with_fallback(path: &str, value: &str) -> std::io::Result<()> {
    // Fast path: try a direct write. Succeeds when the daemon runs as
    // root (the system-service deployment path). If the open fails with
    // PermissionDenied, fall back to `sudo -n tee` so the binary still
    // works when launched by hand as a regular user with the appropriate
    // sudoers entry.
    match std::fs::OpenOptions::new().write(true).open(path) {
        Ok(mut f) => {
            f.write_all(value.as_bytes())?;
            f.write_all(b"\n")?;
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            let mut child = Command::new("sudo")
                .arg("-n")
                .arg("tee")
                .arg(path)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(value.as_bytes())?;
                stdin.write_all(b"\n")?;
            }
            let status = child.wait()?;
            if !status.success() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("sudo tee {path} exited with {status}"),
                ));
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn rebind_touchpad_usb() {
    // We try to find the device's USB path under /sys/bus/usb/devices/. If
    // the touchpad has already self-disconnected from the bus, that lookup
    // returns None — but we can still recover by binding any AMIRA-shaped
    // device ID back to the usb driver, since the kernel still has the
    // port number even after the device dropped. Fall back to the known
    // hub path "1-1.6" which is where the AMIRA touchpad interface lives.
    let id_for_bind = find_touchpad_usb_id().unwrap_or_else(|| "1-1.6".to_string());
    let id_for_unbind = find_touchpad_usb_id();

    eprintln!(
        "trackpad-guard: USB rebind on {} (unbind={} + bind)",
        id_for_bind,
        id_for_unbind.as_deref().unwrap_or("<absent>"),
    );

    // Step 1: attempt unbind only if the device is currently bound.
    // The recurring wedge mode we see in practice is that the AMIRA
    // touchpad drops itself off the bus before our watchdog notices —
    // unbind then fails with ENODEV and we used to abort before getting
    // to the bind step, leaving the device stuck off the bus indefinitely.
    if let Some(unbind_id) = id_for_unbind {
        match write_sysfs_with_fallback("/sys/bus/usb/drivers/usb/unbind", &unbind_id) {
            Ok(()) => {
                thread::sleep(Duration::from_millis(500));
            }
            Err(e) if e.raw_os_error() == Some(19) => {
                // ENODEV — device already gone from the bus. That's the
                // state we want anyway; proceed to bind.
                eprintln!(
                    "trackpad-guard: USB rebind: unbind reported ENODEV; \
                     device already off the bus, proceeding to bind"
                );
            }
            Err(e) => {
                eprintln!("trackpad-guard: USB rebind: unbind failed: {e} (continuing to bind)");
            }
        }
    } else {
        eprintln!("trackpad-guard: USB rebind: device not currently bound, skipping unbind");
    }

    // Step 2: bind. This is the step that actually brings the device
    // back. If it fails, log loudly — the rescan won't have anything to
    // grab.
    if let Err(e) = write_sysfs_with_fallback("/sys/bus/usb/drivers/usb/bind", &id_for_bind) {
        eprintln!("trackpad-guard: USB rebind: bind failed: {e}");
        return;
    }
    eprintln!("trackpad-guard: USB rebind complete");
}

fn main() {
    let (tx, rx) = mpsc::channel::<Msg>();

    {
        let tx = tx.clone();
        let mut signals = Signals::new([SIGUSR1, SIGUSR2, SIGTERM, SIGINT, SIGHUP])
            .expect("install signal handler");
        thread::spawn(move || {
            for sig in signals.forever() {
                match sig {
                    SIGUSR1 => {
                        let _ = tx.send(Msg::ManualRebind);
                    }
                    SIGUSR2 => {
                        let _ = tx.send(Msg::ManualResync);
                    }
                    _ => {
                        let _ = tx.send(Msg::Shutdown);
                        return;
                    }
                }
            }
        });
    }

    // Initial discovery.
    let keyboards = loop {
        let found = find_keyboards();
        if !found.is_empty() {
            break found;
        }
        eprintln!("trackpad-guard: no matching keyboards found, retrying in 2s");
        thread::sleep(DISCOVER_INTERVAL);
    };

    let mut touchpads = loop {
        let found = find_touchpads();
        if !found.is_empty() {
            break found;
        }
        eprintln!("trackpad-guard: no matching touchpads found, retrying in 2s");
        thread::sleep(DISCOVER_INTERVAL);
    };

    // On the AMIRA, both hid-generic and hid-multitouch can bind to the
    // same physical USB interface and each creates its own input device
    // ("Mouse" and "Touchpad" respectively) carrying the same underlying
    // HID reports. Forwarding from both would double every event. Dedupe
    // by USB product id, preferring the Touchpad-named variant.
    touchpads = dedupe_touchpads(touchpads);

    // Build the virtual replacement device. Prefer mirroring from a
    // "Touchpad"-named real device if present — hid-multitouch exposes
    // it with INPUT_PROP_POINTER + INPUT_PROP_BUTTONPAD set, which is
    // what makes libinput classify the result as a touchpad (not a
    // plain mouse). Fall back to the first found otherwise.
    let mirror_idx = touchpads
        .iter()
        .position(|(_, d)| d.name() == Some("AMIRA-KEYBOAR USB KEYBOARD Touchpad"))
        .unwrap_or(0);
    let virt = match create_virtual_touchpad(&touchpads[mirror_idx].1) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "trackpad-guard: failed to create virtual touchpad: {e}. \
                 Check that /dev/uinput is writable by the input group \
                 (see udev/60-trackpad-guard-amira.rules) and that the \
                 uinput kernel module is loaded."
            );
            std::process::exit(1);
        }
    };

    // EVIOCGRAB each real touchpad so nothing else reads them.
    let mut active_touchpads: HashSet<PathBuf> = HashSet::new();
    for (path, dev) in touchpads.iter_mut() {
        if let Err(e) = dev.grab() {
            eprintln!(
                "trackpad-guard: failed to EVIOCGRAB {}: {e}. \
                 If libinput is still holding it, you may need to restart \
                 sway or unbind/rebind the USB device once for the \
                 LIBINPUT_IGNORE_DEVICE tag to take effect.",
                path.display()
            );
            std::process::exit(1);
        }
        active_touchpads.insert(path.clone());
    }

    eprintln!(
        "trackpad-guard: watching {} keyboard(s), {} real touchpad(s), \
         virtual device created (mirroring AMIRA capabilities)",
        keyboards.len(),
        touchpads.len()
    );

    let mut keyboards_alive = keyboards.len();
    // Remember how many touchpads we had at the cleanest moment we ever
    // observed. The opportunistic rescan path uses this as the target
    // and stops enumerating /dev/input every tick once we're back to it
    // — enumeration is non-trivial work on the main thread and starves
    // touchpad event forwarding when run too often.
    let expected_touchpads = touchpads.len();
    for (_, device) in keyboards {
        spawn_keyboard_reader(device, tx.clone());
    }
    for (path, device) in touchpads {
        spawn_touchpad_reader(path, device, tx.clone());
    }

    let mut last_key_ts: Option<Instant> = None;
    let mut last_rebind = Instant::now() - Duration::from_secs(60);
    let debug = std::env::var_os("TRACKPAD_GUARD_DEBUG").is_some();
    let mut virt = virt;
    let mut dropped_batches: u64 = 0;
    let mut forwarded_batches: u64 = 0;
    let mut phantom_batches: u64 = 0;
    let mut slot_releases: u64 = 0;
    let mut phantom_guard = PhantomGuard::new();
    let mut slot_guard = SlotGuard::new();
    // Initial state is "forwarding" — first batch with no recent key
    // stays in that state silently; first batch during typing logs the
    // transition.
    let mut was_forwarding = true;
    // Suppress unused-when-debug-off warning.
    let _ = debug;

    // Heartbeat counters: rolling 5-second window of activity so that
    // when the user reports a stuck moment we can see exactly what the
    // daemon was seeing in the surrounding period.
    let mut hb_last = Instant::now();
    let mut hb_tp: u64 = 0;
    let mut hb_tp_fwd: u64 = 0;
    let mut hb_tp_drop: u64 = 0;
    let mut hb_tp_phantom: u64 = 0;
    let mut hb_slot_rel: u64 = 0;
    let mut hb_key: u64 = 0;
    const HEARTBEAT: Duration = Duration::from_secs(5);

    // Recovery state: when a touchpad reader dies (ENODEV from the USB
    // device dropping or its evdev node being invalidated), we schedule a
    // rescan instead of exiting. The main loop wakes on the deadline,
    // tries to (re)grab any missing touchpads, and reschedules with
    // exponential backoff if the node hasn't reappeared yet.
    let mut next_rescan: Option<Instant> = None;
    let mut rescan_backoff = Duration::from_millis(185);
    const RESCAN_INITIAL: Duration = Duration::from_millis(185);
    const RESCAN_BACKOFF_MAX: Duration = Duration::from_secs(2);
    // recv_timeout when no rescan is pending — long enough to effectively
    // block until a real message arrives.
    const IDLE_TIMEOUT: Duration = Duration::from_secs(3600);

    // Wedge-watchdog state: catches the "events were flowing, then stopped
    // without the evdev node going away" failure mode. The rescan path
    // above only fires when fetch_events() returns Err; in the wedge case
    // the reader stays blocked on a healthy node that has simply gone
    // silent, so we need an active check. We force a USB rebind if:
    //   (1) the touchpad produced enough events recently to count as
    //       "active" since the last rebind (WEDGE_BURST_MIN), and
    //   (2) it's been silent for at least WEDGE_SILENCE for the last
    //       observed event, and
    //   (3) we haven't already rebound in the last WEDGE_COOLDOWN.
    //
    // Note: an earlier version of this added "user present at the
    // keyboard within WEDGE_KEY_PRESENCE" as a fourth condition, to
    // avoid spurious rebinds when the machine was idle. That turned out
    // to lock the watchdog out when the user was actively trying to use
    // a stuck trackpad without typing — exactly the case it most needed
    // to catch. The burst-since-rebind counter already proves we had
    // real activity, and a stray rebind on an idle machine costs nothing
    // visible, so the presence check is gone.
    let mut last_tp_event: Option<Instant> = None;
    // Mid-motion stall instrumentation (see STALL_LOG_SILENCE below).
    let mut motion_run: u32 = 0;
    let mut stall_logged = false;
    let mut tp_burst_since_rebind: u32 = 0;
    // Last observed BTN_TOUCH value from the real touchpad. None until
    // we see a key transition. Used by the wedge watchdog to distinguish
    // "user lifted, now idle" (last_btn_touch = Some(0), silence is
    // expected) from "finger was down, firmware went silent on us"
    // (last_btn_touch = Some(1), silence is a stuck contact and we
    // should rebind even if the USB device is still bound — the bus
    // presence check alone misses this class).
    let mut last_btn_touch: Option<i32> = None;
    let mut next_watchdog = Instant::now() + Duration::from_secs(1);
    const WATCHDOG_TICK: Duration = Duration::from_secs(1);
    // Silence threshold is intentionally long. A short threshold (we
    // started with 3s) is technically faster to react to a wedge, but
    // false-positives constantly during normal "user pauses typing /
    // pauses touchpad" patterns — every pause looked like a wedge, the
    // daemon rebound, and the user perceived the resulting blackouts as
    // jitter. The USB-bus presence check below is the primary signal;
    // this threshold just ensures we don't fire instantly on a
    // sub-second drop that the bus might recover from on its own.
    const WEDGE_SILENCE: Duration = Duration::from_secs(5);
    const WEDGE_COOLDOWN: Duration = Duration::from_secs(30);
    const WEDGE_BURST_MIN: u32 = 30;

    'main: loop {
        // Run periodic timers (watchdog + rescan) BEFORE reading the next
        // message, every iteration. Doing this only on recv_timeout's
        // Timeout branch was wrong: when keyboard events stream in
        // continuously the channel never goes quiet, so we'd never check
        // these deadlines and the wedge would sit unnoticed indefinitely.

        let now = Instant::now();

        // Rescan deadline?
        if next_rescan.is_some_and(|t| t <= now) {
            let new = try_acquire_missing(&mut active_touchpads, &tx);
            if new > 0 {
                eprintln!(
                    "trackpad-guard: rescan recovered {new} touchpad(s) \
                     (now holding {})",
                    active_touchpads.len()
                );
                next_rescan = None;
                rescan_backoff = RESCAN_INITIAL;
            } else {
                rescan_backoff = (rescan_backoff * 2).min(RESCAN_BACKOFF_MAX);
                next_rescan = Some(now + rescan_backoff);
            }
        }

        // Watchdog deadline?
        if next_watchdog <= now {
            next_watchdog = now + WATCHDOG_TICK;

            // Opportunistic rescan: pick up an AMIRA touchpad that wasn't
            // there at startup (or was dropped by a rebind) but is on the bus
            // now. ONLY when we're short of the expected count — enumerating
            // /dev/input opens/probes devices on the main thread, which stalls
            // event forwarding. Running it every tick while a finger is moving
            // shows up as a ~1s cursor hitch (observed 2026-07-01 while short a
            // reader for ~3 min after a phantom rebind). So skip the sweep
            // while the pointer is in active use and defer it to a brief pause;
            // the reader-death path (next_rescan, with backoff) still drives
            // primary recovery, and a redundant second node can wait.
            if active_touchpads.len() < expected_touchpads {
                let pointer_idle =
                    last_tp_event.is_none_or(|t| t.elapsed() > Duration::from_millis(400));
                if pointer_idle {
                    let _ = try_acquire_missing(&mut active_touchpads, &tx);
                }
            }

            // Mid-motion stall detector — LOG ONLY, no recovery action. The
            // 2026-08-03 freeze was: pad reports a FALSE lift (BTN_TOUCH=0)
            // mid-motion, then goes totally mute for 4.27s while the finger is
            // still down, then resumes with a fresh contact. Every existing
            // guard misses it by design — the wedge watchdog treats silence
            // after BTN_TOUCH=0 as "user went idle", the bus stays bound, and
            // no batches arrive for PhantomGuard to judge. Do NOT wire a rebind
            // to this: an "extended silence" rebind trigger existed and was
            // removed 2026-05-27 because it fired on every Mod+digit chord and
            // left the device permanently mute afterwards. This line exists to
            // MEASURE the class (how often, how long, what preceded it) so the
            // next fix is chosen on evidence rather than guessed at.
            const STALL_LOG_SILENCE: Duration = Duration::from_millis(800);
            const STALL_LOG_MIN_RUN: u32 = 20;
            if !stall_logged
                && motion_run >= STALL_LOG_MIN_RUN
                && last_tp_event.is_some_and(|t| t.elapsed() > STALL_LOG_SILENCE)
            {
                eprintln!(
                    "trackpad-guard: real stream went silent mid-motion after {} batches \
                     — last BTN_TOUCH={:?} | {}",
                    motion_run,
                    last_btn_touch,
                    slot_guard.diag_snapshot(now)
                );
                stall_logged = true;
            }

            let silent_for = last_tp_event.map(|t| t.elapsed());
            let cooldown_ok = last_rebind.elapsed() > WEDGE_COOLDOWN;
            let active_recently = tp_burst_since_rebind >= WEDGE_BURST_MIN;
            let silent = silent_for.is_some_and(|d| d > WEDGE_SILENCE);
            // Two distinct stuck classes the watchdog handles:
            //
            // A. USB-bus loss: the AMIRA dropped itself off the bus. We
            //    can see this directly in sysfs; rebind brings it back.
            //
            // B. Firmware silence with finger still down: BTN_TOUCH was
            //    last reported as 1 but events stopped flowing. The bus
            //    is still bound (so check A misses it) and there's no
            //    held-position phantom to detect (so PhantomGuard misses
            //    it — no batches arrive at all). Distinguishing this
            //    from "user lifted and went idle" requires the
            //    last_btn_touch state: silence after BTN_TOUCH=0 is
            //    normal (user idle), silence after BTN_TOUCH=1 is
            //    stuck.
            let bus_lost = !touchpad_is_bound_to_bus();
            let finger_was_down = last_btn_touch == Some(1);
            // Previously a third trigger ("extended silence after recent
            // activity", silent_for > 8s) fired here as a catch-all when
            // neither bus-loss nor BTN_TOUCH=1 was true. Removed
            // 2026-05-27 after live captures showed it firing on every
            // Mod+digit workspace-switch chord and producing only
            // ENODEV/ENODEV sysfs cycles, then leaving the device in a
            // permanently silent post-rebind state. Workspace-switch
            // chords cause the AMIRA touchpad endpoint to go silent
            // briefly; the kernel handles the natural reconnect via
            // autobind + our TouchpadReaderDied rescan path. Forcing a
            // sysfs unbind/bind on top of that races with the kernel
            // and makes recovery worse, not better. Bus-loss and
            // BTN_TOUCH=1 remain as triggers — those are distinct,
            // legitimately-rebindable stuck classes.
            let trigger_reason = if active_recently && silent && cooldown_ok && bus_lost {
                Some("USB device off the bus")
            } else if active_recently && silent && cooldown_ok && finger_was_down {
                Some("BTN_TOUCH=1 last seen, no events since")
            } else {
                None
            };
            if let Some(reason) = trigger_reason {
                eprintln!(
                    "trackpad-guard: wedge watchdog — touchpad silent for \
                     {:?} after {} batches ({}), forcing rebind",
                    silent_for, tp_burst_since_rebind, reason,
                );
                rebind_touchpad_usb();
                last_rebind = Instant::now();
                tp_burst_since_rebind = 0;
                phantom_guard.reset();
                slot_guard.reset();
                last_btn_touch = None;
            }

            // Lifted-ghost recovery: libinput still holds an MT slot open
            // while our forwarded view says nothing is touching (BTN_TOUCH=0,
            // no BTN_TOOL_*). The finger-up's ABS_MT_TRACKING_ID=-1 was lost,
            // so the pointer stays frozen with no further events — the wedge
            // triggers above (bus-loss / finger-down) and PhantomGuard all
            // miss it. Synthesize the missing release; no rebind, the firmware
            // already thinks the finger is up. Timer-driven here so an idle
            // ghost (observed holding for minutes) still clears. (#13)
            let ghosts = slot_guard.lifted_ghost_slots(now, LIFTED_GHOST_TIMEOUT);
            if !ghosts.is_empty() {
                eprintln!(
                    "trackpad-guard: lifted ghost MT slot(s) {:?} open while \
                     BTN_TOUCH=0/tool=none — synthesizing release",
                    ghosts,
                );
                let restore = slot_guard.current_slot;
                if let Err(e) = emit_slot_releases(&mut virt, &ghosts, restore) {
                    eprintln!("trackpad-guard: lifted-ghost release emit failed: {e}");
                } else {
                    for s in &ghosts {
                        slot_guard.note_released(*s);
                    }
                    slot_releases += ghosts.len() as u64;
                    hb_slot_rel += ghosts.len() as u64;
                }
            }
        }

        let now = Instant::now();
        let until_rescan = next_rescan
            .map(|t| t.saturating_duration_since(now))
            .unwrap_or(IDLE_TIMEOUT);
        let until_watchdog = next_watchdog.saturating_duration_since(now);
        let wait = until_rescan.min(until_watchdog);
        let msg = match rx.recv_timeout(wait) {
            Ok(m) => m,
            Err(mpsc::RecvTimeoutError::Timeout) => continue 'main,
            Err(mpsc::RecvTimeoutError::Disconnected) => break 'main,
        };
        match msg {
            Msg::KeyActivity { at } => {
                last_key_ts = Some(at);
                hb_key += 1;
            }
            Msg::TouchpadEvents { events, at } => {
                hb_tp += 1;
                // A batch arriving while we had flagged a mid-motion stall means
                // the pad resumed on its own. Log the recovered duration: paired
                // with the stall line this is the only proof of how long the
                // device was mute, and it distinguishes "pad went quiet" from
                // "our reader stalled" — the real node is EVIOCGRABbed so an
                // external capture can never see that difference (2026-08-03).
                if stall_logged {
                    if let Some(prev) = last_tp_event {
                        eprintln!(
                            "trackpad-guard: real stream RESUMED after {:?} of silence \
                             (daemon was alive throughout — the pad was mute, not us)",
                            prev.elapsed()
                        );
                    }
                    stall_logged = false;
                }
                // Consecutive-batch run length, used to tell a stall that
                // interrupts active motion from the user simply stopping.
                match last_tp_event {
                    Some(prev) if at.duration_since(prev) < Duration::from_millis(300) => {
                        motion_run = motion_run.saturating_add(1)
                    }
                    _ => motion_run = 1,
                }
                last_tp_event = Some(at);
                tp_burst_since_rebind = tp_burst_since_rebind.saturating_add(1);
                // Track the latest BTN_TOUCH value so the wedge watchdog
                // can tell "finger was down when events stopped" apart
                // from "user lifted and went idle." Scanning every batch
                // adds negligible cost — we already iterate events in
                // phantom_guard.observe().
                for ev in &events {
                    if let EventSummary::Key(_, code, value) = ev.destructure() {
                        if code == KeyCode::BTN_TOUCH && (value == 0 || value == 1) {
                            last_btn_touch = Some(value);
                        }
                    }
                }
                // Compare the touchpad event's arrival time against the
                // last keyboard event's arrival time, NOT against now.
                // This stays correct even if the main loop is processing
                // a backlog — what matters is whether the kernel emitted
                // the touchpad event close in time to a real keystroke.
                let gap = last_key_ts.map(|t| at.saturating_duration_since(t));
                let typing = gap.map(|g| g < KEY_TYPING_WINDOW).unwrap_or(false);

                // Log every gate-state transition with the gap in ms so
                // we can correlate user-reported "stuck" periods to what
                // the daemon was actually seeing.
                let became_dropping = typing && was_forwarding;
                let became_forwarding = !typing && !was_forwarding;
                if became_dropping {
                    eprintln!(
                        "trackpad-guard: gate ON (dropping) — gap-since-last-key={:?}",
                        gap
                    );
                    was_forwarding = false;
                } else if became_forwarding {
                    eprintln!(
                        "trackpad-guard: gate OFF (forwarding) — gap-since-last-key={:?}",
                        gap
                    );
                    was_forwarding = true;
                }

                // Always run the phantom detector so its stationary
                // window stays accurate, but only ACT on its verdict when
                // the typing gate is open — a typing-gated batch is
                // already being dropped, no need to also report it as
                // phantom-suppressed.
                let phantom_action = phantom_guard.observe(&events, at);

                if typing {
                    // Palm protection drops touch *motion* while typing — but
                    // never a finger landing or lifting. Forwarding any batch
                    // that changes the contact count keeps libinput's count in
                    // sync, so neither a lost lift (ghost finger, count too
                    // high) nor a lost landing (missing finger, count too low)
                    // can desync the gesture state (see batch_changes_contact).
                    if batch_changes_contact(&events) {
                        if let Err(e) = virt.emit(&events) {
                            eprintln!(
                                "trackpad-guard: virtual emit (gated contact-change) failed: {e}"
                            );
                        } else {
                            forwarded_batches += 1;
                            hb_tp_fwd += 1;
                            // Keep the slot model consistent with what libinput
                            // now sees. observe() records the landing or retires
                            // the slot on a lift; any other stale slots it
                            // reports are left for the next forwarded batch to
                            // handle (we keep the gated path minimal).
                            let _ = slot_guard.observe(&events, at);
                        }
                    } else {
                        dropped_batches += 1;
                        hb_tp_drop += 1;
                    }
                } else {
                    match phantom_action {
                        PhantomAction::Forward => {
                            if let Err(e) = virt.emit(&events) {
                                eprintln!("trackpad-guard: virtual emit failed: {e}");
                            } else {
                                forwarded_batches += 1;
                                hb_tp_fwd += 1;
                                // Track MT slots in what we just forwarded.
                                // If a slot's finger-up was lost — leaving
                                // libinput stuck in multi-finger mode while
                                // one finger keeps moving — synthesize the
                                // missing release so the cursor unfreezes
                                // immediately, no rebind, no blackout.
                                let stale = slot_guard.observe(&events, at);
                                if !stale.is_empty() {
                                    eprintln!(
                                        "trackpad-guard: ghost MT slot(s) {:?} held >{:?} \
                                         while another finger moves — releasing stale contact(s)",
                                        stale, STALE_SLOT_TIMEOUT,
                                    );
                                    let restore = slot_guard.current_slot;
                                    if let Err(e) = emit_slot_releases(&mut virt, &stale, restore) {
                                        eprintln!(
                                            "trackpad-guard: ghost-slot release emit failed: {e}"
                                        );
                                    }
                                    for s in &stale {
                                        slot_guard.note_released(*s);
                                    }
                                    slot_releases += stale.len() as u64;
                                    hb_slot_rel += stale.len() as u64;
                                }
                            }
                        }
                        PhantomAction::Drop => {
                            phantom_batches += 1;
                            hb_tp_phantom += 1;
                        }
                        PhantomAction::EmitReleaseAndDrop => {
                            eprintln!(
                                "trackpad-guard: phantom contact detected — held ≥{:?} within {} units; \
                                 synthesizing slot-0 release",
                                PHANTOM_HOLD, PHANTOM_NOISE,
                            );
                            if let Err(e) = emit_phantom_release(&mut virt) {
                                eprintln!("trackpad-guard: synthetic release emit failed: {e}");
                            }
                            // Keep the slot model in step: the phantom
                            // release retires slot 0.
                            slot_guard.note_released(0);
                            // The synthetic release fixes libinput's view
                            // (finger up) but the AMIRA firmware doesn't
                            // know we did that and keeps streaming events
                            // on the stuck tracking ID. libinput won't
                            // accept those as a new contact without a
                            // fresh tracking ID from hardware. The only
                            // way to clear the firmware-side stuck state
                            // is a USB rebind — same primitive the wedge
                            // watchdog uses, but triggered by phantom
                            // detection rather than bus loss.
                            if last_rebind.elapsed() > PHANTOM_REBIND_COOLDOWN {
                                eprintln!(
                                    "trackpad-guard: phantom contact — issuing USB rebind \
                                     to clear firmware state"
                                );
                                rebind_touchpad_usb();
                                last_rebind = Instant::now();
                                tp_burst_since_rebind = 0;
                                phantom_guard.reset();
                                slot_guard.reset();
                            } else {
                                eprintln!(
                                    "trackpad-guard: phantom contact — rebind cooldown \
                                     active ({:?} since last); only synthetic release applied",
                                    last_rebind.elapsed(),
                                );
                            }
                            phantom_batches += 1;
                            hb_tp_phantom += 1;
                        }
                    }
                }
            }
            Msg::ManualRebind => {
                if last_rebind.elapsed() < Duration::from_secs(5) {
                    eprintln!("trackpad-guard: SIGUSR1 ignored (rate-limited)");
                } else {
                    eprintln!("trackpad-guard: SIGUSR1 — manual USB rebind requested");
                    rebind_touchpad_usb();
                    last_rebind = Instant::now();
                    tp_burst_since_rebind = 0;
                    phantom_guard.reset();
                    slot_guard.reset();
                }
            }
            Msg::ManualResync => {
                // Log the state we're rescuing FROM before we clobber it: this
                // line is the whole experiment. If the cursor comes back the
                // instant this fires, the freeze was libinput-side contact state
                // (and we can then automate the trigger); if it does not, the
                // fault is downstream of libinput and no amount of contact
                // bookkeeping here will fix it.
                let now = Instant::now();
                eprintln!(
                    "trackpad-guard: SIGUSR2 — contact resync requested | {}",
                    slot_guard.diag_snapshot(now)
                );
                match emit_contact_resync(&mut virt, &mut slot_guard, now) {
                    Ok(Some(slot)) => eprintln!(
                        "trackpad-guard: contact resync emitted — all contacts released, \
                         re-landed slot {slot} with fresh tracking id"
                    ),
                    Ok(None) => eprintln!(
                        "trackpad-guard: contact resync emitted — all contacts released \
                         (nothing appeared to be down, no re-land)"
                    ),
                    Err(e) => eprintln!("trackpad-guard: contact resync emit failed: {e}"),
                }
                // The re-landed contact is brand new; don't let the phantom
                // detector judge it on the old stationary window.
                phantom_guard.reset();
            }
            Msg::KeyboardReaderDied => {
                keyboards_alive = keyboards_alive.saturating_sub(1);
                if keyboards_alive == 0 {
                    eprintln!("trackpad-guard: all keyboards disconnected, exiting");
                    break 'main;
                }
            }
            Msg::TouchpadReaderDied { path } => {
                // The reader thread saw fetch_events() return Err — almost
                // always ENODEV after the USB device dropped or its evdev
                // node got invalidated by a renumeration. Drop just this
                // path and schedule a rescan; the new node (often with the
                // same path) will be picked up within a few hundred ms.
                let removed = active_touchpads.remove(&path);
                eprintln!(
                    "trackpad-guard: reader for {} died (was tracked: {}), \
                     scheduling rescan",
                    path.display(),
                    removed
                );
                if next_rescan.is_none() {
                    rescan_backoff = RESCAN_INITIAL;
                    next_rescan = Some(Instant::now() + RESCAN_INITIAL);
                }
            }
            Msg::Shutdown => break 'main,
        }

        // Heartbeat: once HEARTBEAT has elapsed AND there's been any
        // activity, emit a one-liner summarizing the window. We only log
        // when something happened so an idle daemon doesn't spam logs.
        let elapsed = hb_last.elapsed();
        if elapsed >= HEARTBEAT && (hb_tp != 0 || hb_key != 0) {
            let last_key_ago = last_key_ts.map(|t| t.elapsed());
            eprintln!(
                "trackpad-guard: hb {}s — tp_batches={} (fwd={} drop={} phantom={} slotrel={}) key_batches={} last_key_ago={:?} | {}",
                elapsed.as_secs(),
                hb_tp,
                hb_tp_fwd,
                hb_tp_drop,
                hb_tp_phantom,
                hb_slot_rel,
                hb_key,
                last_key_ago,
                slot_guard.diag_snapshot(Instant::now()),
            );
            hb_last = Instant::now();
            hb_tp = 0;
            hb_tp_fwd = 0;
            hb_tp_drop = 0;
            hb_tp_phantom = 0;
            hb_slot_rel = 0;
            hb_key = 0;
        }
    }

    eprintln!(
        "trackpad-guard: exiting (forwarded {} batches, dropped {}, phantom-suppressed {}, ghost-slot releases {})",
        forwarded_batches, dropped_batches, phantom_batches, slot_releases
    );
}
