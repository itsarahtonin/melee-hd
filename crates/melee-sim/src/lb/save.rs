//! The native save: the memory card in slot A as the game sees it, answering `lbcardnew`'s
//! task lists (`lb_8001BD34` and its kin) through `melee-save`, which runs the HSD card layer
//! and the SDK's CARD library on the card's image as the console does. The card itself is the
//! console's device, not game state: the frontend owns it and lends it to each step
//! ([`CardLent`]), as the allocator model is lent ([`crate::hsd::heap::Lent`]). What the game
//! keeps of a task list (its results, the work areas it allocated) is state ([`CardTasks`]).
//!
//! A task list's card and HSD calls complete at once, as on ssbm-rs's emulated card, whose
//! erases and programs finish immediately; the original runs them a step per
//! `lbCardNew_CompleteNextTask`, each step a wait point (`hsd_803AAA48`) where time passes and
//! interrupts are taken. So a list the game waits on takes time on the audio timeline: as
//! long as ssbm-rs's card takes, measured per list and outcome ([`CardTime`]).

use std::cell::RefCell;

use crate::audio::clock::{CardWork, TB_HZ};

use melee_save::tasks::{Run, Task, lists};
use melee_save::{Card, Clock, Console};

use super::super::game::code::unported_call;
use super::super::game::mirror::{Export, Mem, MemMut, Mirror};
use super::super::game::state::Running;
use super::super::game::structs::GmCardData;

/// `lbCardResult`s the game tests.
pub mod result {
    pub use melee_save::tasks::lb::*;
    /// `LbCardResult_Busy`: a task list still running.
    pub const BUSY: i32 = 11;
}

/// The memory card in slot A, if one is in, and the console the card library runs on
/// (its flash ID, which a card's serial must match): the console's device, lent to a step.
#[derive(Clone, Debug, Default)]
pub struct CardDevice {
    pub card: Option<Card>,
    pub console: Console,
    /// Whether the card layer's step is a wait point: on ssbm-rs's console, when a card was
    /// in at power-on (its runner hooks `hsd_803AAA48` then). Only then does ssbm-rs run the
    /// CARD library and its probes of the slots; without, it stands in for them, and they take
    /// no time.
    pub waits: bool,
    /// When the card is out of the slot, on the time base: from the first time to the second
    /// (or for good), an input of the session's ([`crate::game::session::Setup::card_pull`]).
    pub pull: Option<(u64, Option<u64>)>,
    /// `__EXIProbeStartTime[0]`: the probe's tenth of a second when it first saw the card
    /// (0 none); the probe answers only from three tenths later, the card settled.
    pub probe_start: u32,
    /// What the code's time base reads (`OSGetTime`) add to the timeline's: the console's
    /// clock at power-on, in time base ticks from 2000-01-01
    /// ([`crate::game::session::Setup::power_on`]). The probe's tenths and the library's
    /// stamps on the card's directory read it.
    pub time_offset: u64,
    /// Slot A's channel is attached (`Ecb[0].state`'s `EXI_STATE_ATTACHED`): from a task
    /// list's mount to its unmount.
    pub attached: bool,
    /// `Ecb[0].idTime`: the probe's tenth of a second when the card's ID was read
    /// (`EXIGetID`), which a probe that finds it settled reads again when this isn't its
    /// start (0 once the card changed).
    pub id_time: u32,
    /// When a probe of the channel unattached last looked at its insertion interrupt
    /// (EXTINT), which pulling the card out or putting it back raises, on the time base.
    pub extint_seen: u64,
    /// `OSGetFontEncode` has read the video interface's register, and keeps the answer: the
    /// first mount whose ID passes its checksums reads it (`VerifyID`).
    pub font_encode_known: bool,
}

/// Time base ticks a hardware register's read takes on ssbm-rs's console, and a read of the
/// time base (`OSGetTime`).
pub const EXI_READ: u64 = 8;
pub const TIME_READ: u64 = 16;
/// Time base ticks of `EXIGetID` reading a card's ID when it isn't known, counted from
/// ssbm-rs's ports of it and its callees: its two `__EXIProbe`s (48), the interrupts' clear
/// and the insertion interrupt's unmask (16), the select (16), a command written and one read
/// back, each waited for (`EXISync`: 16 and 24, with `__OSGetDIConfig`'s register), the
/// deselect with its probe (24), and the detach's mask (8).
pub const ID_READ: u64 = 152;

impl CardDevice {
    /// Whether the card is in the slot at time base `now`.
    pub fn present(&self, now: u64) -> bool {
        self.card.is_some()
            && !self
                .pull
                .is_some_and(|(out, back)| now >= out && back.is_none_or(|b| now < b))
    }

    /// `EXIProbe(0)` at `now`: whether a card answers, and the time the probe takes on
    /// ssbm-rs's console. `__EXIProbe` reads the channel's status register; attached, that is
    /// all, and a card in answers. Unattached, an insertion interrupt a pull or an insertion
    /// raised is read again and cleared, forgetting the probe's start and the ID; no card
    /// (no EXT bit in the first read: the device line, which reads add only while a card is
    /// in) forgets them too; a card reads the time (`OSGetTime`), and answers once seen for
    /// three tenths of a second. A card that answers with its ID forgotten has it read
    /// (`EXIGetID`).
    pub fn exi_probe_timed(&mut self, now: u64) -> (bool, u64) {
        let mut time = EXI_READ;
        let present = self.present(now);
        if self.attached {
            return (present, time);
        }
        if self.extint(now) {
            time += EXI_READ;
            self.probe_start = 0;
            self.id_time = 0;
        }
        self.extint_seen = now;
        if !present {
            self.probe_start = 0;
            self.id_time = 0;
            return (false, time);
        }
        time += TIME_READ;
        let tenths = probe_tenths(now, self.time_offset);
        if self.probe_start == 0 {
            self.probe_start = tenths;
        }
        let answers = tenths.wrapping_sub(self.probe_start) as i32 >= 3;
        if answers && self.id_time != self.probe_start {
            time += ID_READ;
            self.id_time = self.probe_start;
        }
        (answers, time)
    }

    /// Whether the card was pulled out or put back since the channel's insertion interrupt
    /// was last looked at.
    fn extint(&self, now: u64) -> bool {
        let Some((out, back)) = self.pull else {
            return false;
        };
        self.card.is_some()
            && [Some(out), back]
                .into_iter()
                .flatten()
                .any(|b| self.extint_seen < b && b <= now)
    }
}

/// `__EXIProbe`'s count of tenths of a second at timeline time `now`, on the code's time base
/// (`time_offset` the clock at power-on): `OSGetTime` in milliseconds over 100, plus one, in
/// the word it keeps (`__EXIProbeStartTime`).
pub fn probe_tenths(now: u64, time_offset: u64) -> u32 {
    (((now + time_offset) / (TB_HZ / 1000) / 100) as u32).wrapping_add(1)
}

/// The pad alarm's handler's card probes at `now` (`fn_800195FC`'s `lb_8001C600` and
/// `lbSnap_8001D2BC`: `CARDProbe` of slot A twice, then of slot B, which is empty): their
/// time, and whether slot A's card answered. On ssbm-rs's console without a card at power-on
/// they take none, as without a session's card device (a replay's).
pub fn alarm_card_probes(now: u64) -> (u64, bool) {
    LENT.with(|l| {
        let mut l = l.borrow_mut();
        let Some(dev) = l.as_mut().filter(|d| d.waits) else {
            return (0, false);
        };
        let (card, first) = dev.exi_probe_timed(now);
        let (again, second) = dev.exi_probe_timed(now + first);
        debug_assert_eq!(card, again, "slot A's two probes in one alarm differ");
        (first + second + EXI_READ, card)
    })
}

thread_local! {
    static LENT: RefCell<Option<CardDevice>> = const { RefCell::new(None) };
}

/// A step's loan of the card device to the thread.
#[must_use]
pub struct CardLent(bool);

impl CardLent {
    pub fn take(dev: &mut Option<CardDevice>) -> CardLent {
        let lent = LENT.with(|l| l.borrow().is_none()) && dev.is_some();
        if lent {
            LENT.with(|l| *l.borrow_mut() = dev.take());
        }
        CardLent(lent)
    }

    pub fn give_back(self, dev: &mut Option<CardDevice>) {
        if self.0 {
            *dev = LENT.with(|l| l.borrow_mut().take());
        }
        std::mem::forget(self);
    }
}

impl Drop for CardLent {
    fn drop(&mut self) {
        if self.0 {
            LENT.with(|l| l.borrow_mut().take());
        }
    }
}

/// The lent device, if a step has it: what `f` makes of it.
fn with_device<R>(f: impl FnOnce(&mut CardDevice) -> R) -> R {
    LENT.with(|l| {
        let mut l = l.borrow_mut();
        let dev = l
            .as_mut()
            .expect("a card task without the console's card device (a session lends it)");
        f(dev)
    })
}

/// What `lbcardnew`'s state keeps that the game reads: the work areas' addresses
/// (`lbCardNew_AllocWorkArea`) and the last list's per-file results (`unk_38`'s
/// `lb_error`, which `lb_8001B6E0` reads). The rest of its state (the task array, the HSD
/// layer's `CardState`, the CARD library's control blocks) is the card layers', which the
/// native save replaces: no game code reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct CardTasks {
    pub work_area: u32,
    pub lib_area: u32,
    /// Each logical file's last result as `lbCardResult` (`unk_38[i].lb_error`), 16 for
    /// none (`resetState`).
    pub file_errors: [i32; 9],
}

/// How a task list the game waits on spends time on ssbm-rs's card: the mount's probe and
/// transfers, run at once from the list's start, then each call of
/// `lbCardNew_CompleteNextTask` until the list ends (a wait point, `hsd_803AAA48`, and that
/// step's transfers), then the unmount. The transfers are the CARD library's EXI commands,
/// which on ssbm-rs's card finish at once: a read is a chain of 512-byte segments, each issued
/// by the last one's transfer-complete interrupt's handler with interrupts off
/// ([`crate::audio::clock::Clock::card_chain`]). ssbm-rs's probe of the boot's load (every
/// interrupt change, EXI transfer, event and interrupt delivery, `local/research/card-chain`)
/// gives each part's time; the steps were counted from its runs of the recorded sessions
/// (`CARD_STEPS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardTime {
    /// `lb_8001BD34` loading the save. With the file opened (intact or damaged): the
    /// mount, then 23 steps: the check and the open (no transfer: they run from what the
    /// mount read), twenty block reads (steps 2 to 13 and 15 to 22: the file's eleven blocks,
    /// a block a step, twice over for the data kept twice and the spare), steps 14 and 23
    /// none. Without (no file, a full card, a broken one): the mount and the check.
    Load,
    /// A list that only mounts the card and looks at its directory (`lb_8001C2D8` looking
    /// for another game's file, `lb_8001C87C` probing for the save): the mount, then one
    /// step (the check, the free blocks and the open run from what the mount read).
    Mount,
}

/// A step's transfers after its wait point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    /// None: the step runs from what the card layers already read.
    Idle,
    /// A block's read (`CARDReadAsync` of 0x2000 bytes: sixteen segments).
    Block,
}

/// The mount (`taskMount`, `CARDMountAsync`) with interrupts off from its attach (two probes
/// of the slot) through `DoMount`'s status commands (the status cleared and read, the card's
/// interrupt enabled) to its first segment's DMA, after `CARDProbeEx`'s probes: 384 ticks.
pub const MOUNT_FIRST: u64 = 384;
/// The mount's reads: the ID, both directories and both allocation tables, 0x2000 bytes each.
pub const MOUNT_SEGMENTS: u32 = 80;
/// The last segment's handler, its deselect and the mount's callbacks (`__CARDMountCallback`'s
/// `__CARDVerify`), and with them `OSGetFontEncode`'s register read ([`EXI_READ`]) the first
/// time an ID gets that far ([`CardDevice::font_encode_known`]): ssbm-rs's probe has 72 for
/// the boot's load, 64 for the memory card scene's probe after it and for a broken card's.
pub const MOUNT_TAIL: u64 = 64;
/// A block's read from its step (`hsd_803AAA48` after `CARDFastOpen`): the select (24), three
/// commands 16 apart and the DMA 32 after them, with interrupts off.
pub const BLOCK_FIRST: u64 = 96;
pub const BLOCK_SEGMENTS: u32 = 16;
/// The block's last segment's handler: the deselect and `BlockReadCallback`'s callbacks into
/// the HSD layer.
pub const BLOCK_TAIL: u64 = 64;
/// `CARDUnmount` at a list's end (`DoUnmount`: the channel's callback cleared and the channel
/// detached), with interrupts off: 24 in every list ssbm-rs's probe logs (the intact, no-file
/// and broken cards' loads, and the scene's probe).
pub const UNMOUNT: u64 = 24;

impl CardTime {
    /// The steps after the mount's for a list that ended on `result`; `None` when no card
    /// answered the mount's probe (the list fails there, before any wait).
    fn steps(self, result: i32, card_in: bool) -> Option<Vec<Step>> {
        if !card_in {
            return None;
        }
        Some(match (self, result) {
            (CardTime::Load, result::READY | result::DAMAGED) => (1..=23)
                .map(|s| match s {
                    1 | 14 | 23 => Step::Idle,
                    _ => Step::Block,
                })
                .collect(),
            (CardTime::Load, result::NO_FILE | result::MALFORMED) | (CardTime::Mount, _) => {
                vec![Step::Idle]
            }
            (CardTime::Load, r) => unported_call(&format!(
                "the card's time over a load that ends on lbCardResult {r}"
            )),
        })
    }
}

/// Register reads of a sector the save's list writes (a sector's erase and its 64 page
/// programs, each page two interrupts), after its step's wait point: fitted to the menu
/// sessions' traces (`docs/saves.md`), to be replaced by a probe of a save's writes. With the
/// write's waits ([`SAVE_WRITE_WAITS`]) the recorded sessions that save compare clean through
/// the main menu with none; 323 (a block's read before reads were chains) makes
/// `2026-10-06_17-44-10`'s tick 274 end 2,239 ticks early.
pub const SAVE_WRITE_WORK: u64 = 0;
/// The waits a sector the save's list writes takes, its step's the first: fitted to the menu
/// sessions' traces (`docs/saves.md`), to be replaced by a probe of a save's writes. With two,
/// `2026-10-06_17-44-10`'s tick 274 ends 2,239 ticks early and `2026-10-06_12-42-06`'s tick
/// 163 has the writer done (`x10`) before the trace's; with four, 12-42-06's tick 163 ends
/// 2,239 ticks early. ssbm-rs's step log (`CARD_STEPS`) shows no step for the waits between,
/// where the HSD layer is still busy (the erase's interrupt, then the programs').
pub const SAVE_WRITE_WAITS: usize = 3;
/// Register reads of a file's status set or a file created in the save's list (a directory's
/// write): fitted to the menu sessions' traces (`docs/saves.md`), to be replaced by a probe of
/// a save's writes. The save's list sets no status and creates nothing while its file is
/// intact.
pub const SAVE_OTHER_WORK: u64 = 0;

/// A step of the save's list at one of the game's waits: its wait point, then `work`.
fn step(work: CardWork) -> CardWork {
    CardWork { step: true, ..work }
}

/// The save list's steps after the mount, from what the HSD layer asked of the card
/// (`run.steps`), and when each file's result changes: at its callback (`fn_8001A0B0`, in
/// the call that sees its request's last transfer done) and at the write task's start
/// (`taskWrite` queueing each file's write). One step a wait, each a
/// `lbCardNew_CompleteNextTask`: the check's and the open's (no transfer, as a load's); each
/// sector transfer after its wait point (a request starts in the call that issues its first
/// transfer, and a later request of the same task in the call its previous one ends in; a
/// read is a chain of segments, as a load's blocks, a write [`SAVE_WRITE_WORK`] over
/// [`SAVE_WRITE_WAITS`]); between two tasks, the call that sees the first's last transfer
/// done, runs its callbacks and starts the next (`executeNextTask`, whose requests the HSD
/// layer takes up at its next call); and the list's end, the call that sees its last transfer
/// done, runs the callbacks and unmounts the card ([`UNMOUNT`]).
fn save_steps(run: &Run) -> (Vec<CardWork>, Vec<(usize, usize, i32)>) {
    let idle = step(CardWork::default());
    let mut steps = vec![idle];
    let mut results = Vec::new();
    let mut tasks = 0;
    let mut callbacks = run.callbacks.iter();
    for (i, &c) in run.steps.iter().enumerate() {
        match c {
            b't' => {
                if tasks > 0 {
                    steps.push(idle);
                }
                tasks += 1;
                if run.write_queued == Some(i) {
                    let at = steps.len() - 1;
                    for &(file, h) in &run.queued {
                        results.push((at, file, melee_save::tasks::from_hsd(h)));
                    }
                }
            }
            b'k' => {
                let &(arg, h) = callbacks.next().expect("a callback for each k");
                if (0..9).contains(&arg) {
                    results.push((steps.len(), arg as usize, melee_save::tasks::from_hsd(h)));
                }
            }
            b'q' => {}
            b'r' => steps.push(step(CardWork {
                chain: Some((BLOCK_FIRST, BLOCK_SEGMENTS, BLOCK_TAIL)),
                ..CardWork::default()
            })),
            b'w' => {
                steps.push(step(CardWork {
                    reads: SAVE_WRITE_WORK,
                    ..CardWork::default()
                }));
                steps.extend(std::iter::repeat_n(idle, SAVE_WRITE_WAITS - 1));
            }
            _ => steps.push(step(CardWork {
                reads: SAVE_OTHER_WORK,
                ..CardWork::default()
            })),
        }
    }
    steps.push(step(CardWork {
        ticks: UNMOUNT,
        ..CardWork::default()
    }));
    (steps, results)
}

/// The save's card data and header as the writer's list takes them (`lb_8001BE30`'s
/// manifest, banner and icons).
#[derive(Clone, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct SaveRequest {
    pub data: Vec<u8>,
    pub banner: Vec<u8>,
    pub icons: Vec<u8>,
}

/// The save's writer as it runs over the game's waits (`lb_8001CC84` from `lb_800195D0`):
/// the list the writer's first step starts, the register reads of each step left after it,
/// and what the game takes up of a list started or ended at its next look
/// ([`Running::card_writer_take`]). It lives on the audio side, where the waits are.
#[derive(Clone, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Writer {
    /// A list to start at the next wait.
    pub start: Option<SaveRequest>,
    /// Each later step's card work, after its wait point; `next` the next one's.
    pub steps: Vec<CardWork>,
    pub next: usize,
    /// The list's result once its last step ran, and whether its mount found the card.
    pub done: Option<(i32, bool)>,
    /// The list's per-file results (`unk_38`'s `lb_error`) as of the last step run.
    pub file_errors: [i32; 9],
    /// The comment the list's start made (`lb_8001C658`).
    pub comment: Option<[u8; 0x40]>,
    /// Each change of a file's result: the step it comes in, the file and the result.
    pub results: Vec<(usize, usize, i32)>,
    /// Whether the results changed since the game last took them.
    pub errors_due: bool,
}

impl Writer {
    /// Whether a list is started or running.
    pub fn busy(&self) -> bool {
        self.start.is_some() || self.next < self.steps.len()
    }

    /// The writer's step at one of the game's waits at `now` ([`crate::audio::clock::Frames::game_wait`]):
    /// a list started (its probe, the comment's clock read and the mount, with no wait point
    /// first), or its next step (after its wait point); none when no list runs.
    pub fn wait(&mut self, now: u64) -> Option<CardWork> {
        if let Some(req) = self.start.take() {
            return Some(self.begin(req, now));
        }
        if self.next < self.steps.len() {
            let w = self.steps[self.next];
            for &(at, file, e) in &self.results {
                if at == self.next {
                    self.file_errors[file] = e;
                    self.errors_due = true;
                }
            }
            self.next += 1;
            // The list's last step unmounts the card (its end task), detaching the channel.
            if self.next == self.steps.len() {
                detach();
            }
            return Some(w);
        }
        None
    }

    /// Starts the list on the lent card at `now`: it writes what it writes at once, and the
    /// steps' time is left for the waits after. The start's work: the mount's probe
    /// (`CARDProbeEx`: the slot's and its ID's) and transfers, as a load's.
    fn begin(&mut self, req: SaveRequest, now: u64) -> CardWork {
        let (card_in, probe, offset) = with_device(|dev| {
            if !dev.waits {
                // No card at power-on: ssbm-rs stands in for the CARD library.
                return (false, 0, dev.time_offset);
            }
            let (card, probe) = dev.exi_probe_timed(now + TIME_READ);
            let id = if card {
                dev.exi_probe_timed(now + TIME_READ + probe).1
            } else {
                0
            };
            (card, probe + id, dev.time_offset)
        });
        // `lb_8001C658`: the comment's date, from `OSGetTime` as the list starts.
        let seconds = ((now + offset) / TB_HZ) as u32;
        let comment = melee_save::melee::comment(seconds, false);
        self.comment = Some(comment);
        let mut bufs = melee_save::exec::Bufs::default();
        for (i, f) in bufs.files.iter_mut().enumerate().skip(1) {
            *f = req.data[file_span(i).0..].to_vec();
        }
        bufs.banner = req.banner;
        bufs.icons = req.icons;
        let mut run = Run::new(melee_save::melee::manifest(), bufs);
        run.comment = comment;
        let r = with_device(|dev| {
            let mut clock = Clock::at(seconds);
            let card = if card_in { dev.card.as_mut() } else { None };
            run.execute(
                card,
                &dev.console.clone(),
                &mut clock,
                &lists::save(melee_save::melee::FILENAME),
            )
        });
        // `resetState`: no file has a result yet.
        self.file_errors = [result::START; 9];
        self.errors_due = true;
        self.next = 0;
        self.steps.clear();
        // The comment's `OSGetTime` (`lb_8001C658`), before the list starts.
        let mut work = CardWork {
            ticks: TIME_READ + probe,
            ..CardWork::default()
        };
        if !card_in {
            // The mount's probe fails, and the list with it, before any step.
            self.done = Some((r, false));
            return work;
        }
        // The mount attaches slot A's channel; its check reads the font encoding the first
        // time the card's ID gets that far.
        let font = with_device(|dev| {
            dev.attached = true;
            let read = !dev.font_encode_known && dev.card.as_ref().is_some_and(id_valid);
            dev.font_encode_known |= read;
            if read { EXI_READ } else { 0 }
        });
        work.chain = Some((MOUNT_FIRST, MOUNT_SEGMENTS, MOUNT_TAIL + font));
        (self.steps, self.results) = save_steps(&run);
        self.done = Some((r, true));
        work
    }

    /// The list's end, once its last step ran: its result and whether its mount found the
    /// card.
    pub fn take_done(&mut self) -> Option<(i32, bool)> {
        if self.busy() {
            return None;
        }
        self.done.take()
    }
}

impl CardTasks {
    /// `lbCardNew_ForgetMemory`.
    #[doc(alias = "lbCardNew_ForgetMemory")]
    pub fn forget_memory(&mut self) {
        self.work_area = 0;
        self.lib_area = 0;
    }

    /// `lbCardNew_Init` at boot: no list, every file's result none (`resetState`). The HSD
    /// layer's own setup (`hsd_803B2374`) is the replaced layer's.
    #[doc(alias = "lbCardNew_Init")]
    pub fn init(&mut self) {
        *self = CardTasks {
            file_errors: [result::START; 9],
            ..CardTasks::default()
        };
    }

    /// `lb_8001B6E0`.
    #[doc(alias = "lb_8001B6E0")]
    pub fn file_error(&self, file: usize) -> i32 {
        self.file_errors[file]
    }
}

/// Whether the card's ID passes `VerifyID`'s first checks (no device ID, a 4 Mbit card, its
/// checksums), which reach the font encoding's.
fn id_valid(card: &Card) -> bool {
    let id = &card.image()[..0x200];
    let be16 = |at: usize| u16::from_be_bytes([id[at], id[at + 1]]);
    be16(0x20) == 0
        && be16(0x22) == 4
        && melee_save::card::checksum(&id[..0x1FC]) == (be16(0x1FC), be16(0x1FE))
}

/// The card data's logical files: where each lies in `GmCardData` (file 1 the save data,
/// 2 to 8 the name tag banks).
fn file_span(file: usize) -> (usize, usize) {
    use melee_save::melee::{NAME_TAG_BANK, SAVE_DATA};
    match file {
        1 => (0, SAVE_DATA),
        2..=8 => (SAVE_DATA + (file - 2) * NAME_TAG_BANK, NAME_TAG_BANK),
        _ => unreachable!("Melee's save has files 1 to 8"),
    }
}

/// The list's end unmounts the card, detaching slot A's channel.
pub fn detach() {
    with_device(|dev| dev.attached = false);
}

/// The card data as memory holds it.
pub fn card_data_bytes(d: &GmCardData) -> Vec<u8> {
    let mut bytes = vec![0u8; GmCardData::SIZE as usize];
    d.export(
        &mut MemMut {
            bytes: &mut bytes,
            base: 0,
        },
        0,
    );
    bytes
}

/// A mirror's `Cx` for structs with no pointers.
struct Plain;

impl super::super::game::mirror::Cx for Plain {
    fn import_handle(&mut self, kind: &'static str, _: u32) -> Option<u32> {
        unreachable!("a pointer ({kind}) in the card's data")
    }
    fn import_data(&mut self, _: u32) -> Option<melee_data::archive::DataRef> {
        unreachable!("a pointer in the card's data")
    }
    fn compare_handle(&mut self, _: &str, _: &'static str, _: Option<u32>, _: u32) {}
    fn compare_data(&mut self, _: &str, _: Option<melee_data::archive::DataRef>, _: u32) {}
    fn mismatch(&mut self, _: &str, _: String, _: String) {}
}

impl Running<'_> {
    /// Runs a task list on the lent card, with the card data's memory as the files' buffers
    /// (a read writes into it), and keeps what the game reads of it; the time it takes passes
    /// on the timeline. The list's result.
    fn run_card_list(&mut self, tasks: &[(Task, u32)], time: CardTime, status: bool) -> (i32, Run) {
        let start = self.clock_now();
        let thing = &self.game.game_data.thing;
        let memory = card_data_bytes(thing);
        let mut bufs = melee_save::exec::Bufs::default();
        for (i, f) in bufs.files.iter_mut().enumerate().skip(1) {
            *f = memory[file_span(i).0..].to_vec();
        }
        let mut run = Run::new(melee_save::melee::manifest(), bufs);
        self.card_alarm_probes();
        let now = self.clock_now();
        // The mount's `CARDProbeEx`: the slot's probe and, for a card that answers, its ID's
        // (`EXIGetID`, a second probe, reading the ID when it isn't known), interrupts off.
        let (card_in, probe) = with_device(|dev| {
            let (card, probe) = dev.exi_probe_timed(now);
            let id = if card {
                dev.exi_probe_timed(now + probe).1
            } else {
                0
            };
            (card, probe + id)
        });
        // The mount's probe found the card: the lists that take the card game's status clear
        // it (`*_p(status) = 0`); the others pass none.
        if card_in && status {
            self.game.card_game.unk_status = false;
        }
        let r = with_device(|dev| {
            // The library's `OSTicksToSeconds(OSGetTime())`, as the list started.
            let mut clock = Clock::at(((start + dev.time_offset) / TB_HZ) as u32);
            let card = if card_in { dev.card.as_mut() } else { None };
            run.execute(card, &dev.console.clone(), &mut clock, tasks)
        });
        // What the reads wrote, file by file, the bytes they changed.
        let mut after = memory.clone();
        for (i, f) in run.bufs.files.iter().enumerate().skip(1) {
            let (at, len) = file_span(i);
            assert!(
                f.len() <= memory.len() - at,
                "a card read past the card data (file {i})"
            );
            for (k, &b) in f.iter().enumerate() {
                if b != memory[at + k] {
                    after[at + k] = b;
                }
            }
            let _ = len;
        }
        if after != memory {
            self.game.game_data.thing = GmCardData::import(
                &Mem {
                    bytes: &after,
                    base: 0,
                },
                0,
                &mut Plain,
            );
        }
        let t = &mut self.game.card_tasks;
        for i in 0..9 {
            t.file_errors[i] = match run.file_results[i] {
                -1 => result::START,
                h => melee_save::tasks::from_hsd(h),
            };
        }
        let steps = time.steps(r, card_in);
        // The mount's probe, with interrupts off; a list without a card that answers fails
        // there. Where ssbm-rs stands in for the CARD library it takes no time.
        if with_device(|dev| dev.waits) {
            self.game.audio.clock_read(self.data, probe);
        }
        if let Some(steps) = steps {
            // The mount attaches slot A's channel, and the list's end unmounts it. Its check
            // reads the font encoding the first time the card's ID gets that far.
            let font = with_device(|dev| {
                dev.attached = true;
                let read = !dev.font_encode_known && dev.card.as_ref().is_some_and(id_valid);
                dev.font_encode_known |= read;
                if read { EXI_READ } else { 0 }
            });
            let a = &mut self.game.audio;
            a.clock_card_chain(self.data, MOUNT_FIRST, MOUNT_SEGMENTS, MOUNT_TAIL + font);
            for step in steps {
                a.clock_wait(self.data);
                if step == Step::Block {
                    a.clock_card_chain(self.data, BLOCK_FIRST, BLOCK_SEGMENTS, BLOCK_TAIL);
                }
            }
            a.clock_read(self.data, UNMOUNT);
        }
        // The list's end unmounts the card, detaching the channel.
        with_device(|dev| dev.attached = false);
        self.card_alarm_probes();
        (r, run)
    }

    /// `lb_8001BD34` with the save's manifest (`lb_8001CBBC`'s): the save loaded into the
    /// card data, the game waiting for the list. Its result.
    #[doc(alias = "lb_8001BD34")]
    pub fn card_load(&mut self) -> i32 {
        let (r, _) = self.run_card_list(
            &lists::load(melee_save::melee::FILENAME),
            CardTime::Load,
            true,
        );
        r
    }

    /// `lb_8001B7E0` with the save's manifest (`lb_8001C87C`'s): what the card holds of the
    /// save, the game waiting for the list: 1 or 2 when it's there (opened, damaged), else
    /// why not (`lbCardResult`).
    #[doc(alias = "lb_8001B7E0")]
    #[doc(alias = "lb_8001C87C")]
    pub fn card_probe_save(&mut self) -> i32 {
        let (r, _) = self.run_card_list(
            &lists::probe(melee_save::melee::FILENAME),
            CardTime::Mount,
            true,
        );
        r
    }

    /// `lb_8001C2D8`: whether a file of another game is on the card (`lbCardResult` 0 when
    /// it is).
    #[doc(alias = "lb_8001C2D8")]
    pub fn card_find_file(&mut self, company: [u8; 2], game: [u8; 4], name: &[u8]) -> i32 {
        let (r, _) = self.run_card_list(
            &lists::find_file(company, game, name),
            CardTime::Mount,
            false,
        );
        r
    }

    /// `lbCardNew_ProbeEx`: whether a memory card is in the slot (`lbCardResult` 0), else
    /// the device error. `CARDProbeEx` probes the slot (`EXIProbeEx`) and asks a card that
    /// answers for its ID (`EXIGetID`, which probes again and finds it known), with
    /// interrupts off; on ssbm-rs's console without a card at power-on it's stood in for and
    /// takes no time.
    #[doc(alias = "lbCardNew_ProbeEx")]
    pub fn card_probe(&mut self) -> i32 {
        self.card_alarm_probes();
        let now = self.clock_now();
        let (ready, time) = with_device(|dev| {
            if !dev.waits {
                return (false, 0);
            }
            let (ready, probe) = dev.exi_probe_timed(now);
            let id = if ready {
                dev.exi_probe_timed(now + probe).1
            } else {
                0
            };
            (ready, probe + id)
        });
        self.game.audio.clock_read(self.data, time);
        if ready {
            result::READY
        } else {
            result::DEVICE_ERROR
        }
    }

    fn clock_now(&self) -> u64 {
        self.game.audio.clock.as_ref().map_or(0, |c| c.now)
    }

    /// The pad alarm's card probes since the last look, at the times each alarm fired:
    /// `lb_8001C600`'s (`CARDProbe(0)`, the swap noted when it changes) and
    /// `lbSnap_8001D2BC`'s (`CARDProbe` of slots A and B, none in B).
    #[doc(alias = "lb_8001C600")]
    #[doc(alias = "lbSnap_8001D2BC")]
    pub fn card_alarm_probes(&mut self) {
        let Some(times) = self
            .game
            .audio
            .clock
            .as_mut()
            .and_then(|c| c.alarm_times.as_mut())
            .map(std::mem::take)
        else {
            return;
        };
        for p in times {
            let g = &mut self.game.card_game;
            if p.card != g.probe_status {
                g.unk_status = true;
            }
            g.probe_status = p.card;
            for (chan, card) in [p.card, false].into_iter().enumerate() {
                let s = &mut self.game.snap;
                if i32::from(card) != s.card_state[chan] {
                    s.state_changed[chan] = 1;
                }
                s.card_state[chan] = i32::from(card);
            }
        }
    }

    /// `lbCardNew_CompleteAllTasks(11)` after a scene: no list is ever left running (the
    /// game waits on each but the save's writer, which `lb_8001CDB4` finishes first), so one
    /// call of `lbCardNew_CompleteNextTask` ends it: a wait point on ssbm-rs's console with a
    /// card in (`hsd_803AAA48`).
    #[doc(alias = "lbCardNew_CompleteAllTasks")]
    #[doc(alias = "lbCardNew_CompleteNextTask")]
    pub fn card_complete_all_tasks(&mut self) {
        if with_device(|dev| dev.waits) {
            self.game.audio.clock_wait(self.data);
        }
    }

    /// `lbCardNew_AllocWorkArea`: the CARD library's work area and the HSD layer's, from the
    /// scene's heap, once a scene.
    #[doc(alias = "lbCardNew_AllocWorkArea")]
    pub fn card_alloc_work_area(&mut self) {
        let t = &mut self.game.card_tasks;
        if t.work_area == 0 {
            t.work_area = crate::hsd::heap::mem_alloc(0xA000);
            t.lib_area = crate::hsd::heap::mem_alloc(0x2000);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tenth of a second on the time base.
    const TENTH: u64 = TB_HZ / 10;

    #[test]
    fn a_probe_takes_what_the_exi_library_reads() {
        // In since power-on, its ID read at boot, pulled at 10 s and put back at 20 s.
        let mut dev = CardDevice {
            card: Some(Card::blank()),
            waits: true,
            probe_start: 1,
            id_time: 1,
            pull: Some((100 * TENTH, Some(200 * TENTH))),
            ..CardDevice::default()
        };
        // Settled: the status register and the time.
        assert_eq!(dev.exi_probe_timed(10 * TENTH), (true, 24));
        // Attached, the register alone.
        dev.attached = true;
        assert_eq!(dev.exi_probe_timed(11 * TENTH), (true, 8));
        dev.attached = false;
        // Pulled: the insertion interrupt read and cleared, and no card (no EXT bit, so no
        // time read), then no card.
        assert_eq!(dev.exi_probe_timed(101 * TENTH), (false, 16));
        assert_eq!(dev.probe_start, 0);
        assert_eq!(dev.exi_probe_timed(102 * TENTH), (false, 8));
        // Put back: the interrupt and the time, settling, then its ID read once it answers.
        assert_eq!(dev.exi_probe_timed(201 * TENTH), (false, 32));
        assert_eq!(dev.exi_probe_timed(202 * TENTH), (false, 24));
        assert_eq!(dev.exi_probe_timed(205 * TENTH), (true, 24 + ID_READ));
        assert_eq!(dev.exi_probe_timed(206 * TENTH), (true, 24));
    }
}
