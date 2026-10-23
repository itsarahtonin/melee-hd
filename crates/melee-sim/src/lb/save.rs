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

use crate::audio::clock::TB_HZ;

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
    /// The real-time clock at power-on, in seconds since 2000-01-01, which the library stamps
    /// the card's directory with as the time base runs on (`OSGetTime`).
    pub clock: u64,
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
    /// The time base's offset from the OS's time (`OSGetTime`): the clock at power-on.
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
    /// The channel's status register as ssbm-rs's console stores it holds the card's EXT bit
    /// (a card in). Its reads add EXT while the card is in, and otherwise give what the
    /// register holds: the interrupt mask's update (`SetInterruptMask`) writes back what it
    /// read, EXT included, so after a mount's or an ID read's detach the bit stays set until a
    /// probe's clearing of the insertion interrupt writes the register without it. So the
    /// first probe after a pull still finds a card (it reads the time and doesn't answer).
    pub ext_held: bool,
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
    /// (no EXT bit in the first read, [`CardDevice::ext_held`]) forgets them too; a card
    /// reads the time (`OSGetTime`), and answers once seen for three tenths of a second. A
    /// card that answers with its ID forgotten has it read (`EXIGetID`).
    pub fn exi_probe_timed(&mut self, now: u64) -> (bool, u64) {
        let mut time = EXI_READ;
        let present = self.present(now);
        if self.attached {
            return (present, time);
        }
        let ext = present || self.ext_held;
        if self.extint(now) {
            time += EXI_READ;
            self.probe_start = 0;
            self.id_time = 0;
            self.ext_held = false;
        }
        self.extint_seen = now;
        if !ext {
            self.probe_start = 0;
            self.id_time = 0;
            return (false, time);
        }
        time += TIME_READ;
        let tenths = ((now + self.time_offset) / (TB_HZ / 1000) / 100) as u32 + 1;
        if self.probe_start == 0 {
            self.probe_start = tenths;
        }
        let answers = tenths.wrapping_sub(self.probe_start) >= 3;
        if answers && self.id_time != self.probe_start {
            time += ID_READ;
            self.id_time = self.probe_start;
            self.ext_held = true;
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

/// How a task list the game waits on spends time on ssbm-rs's card: the first task's work
/// (the mount's transfers, run at once from the list's start), then each call of
/// `lbCardNew_CompleteNextTask` until the list ends: a wait point (`hsd_803AAA48`) and that
/// step's transfers. The transfers are the CARD library's EXI commands, which on ssbm-rs's
/// card finish at once but read the interface's registers ([`MOUNT_WORK`],
/// [`BLOCK_WORK`]). The steps were counted from ssbm-rs's runs of the recorded sessions
/// (`CARD_STEPS`, `CALLS=EXIDma,EXIImm,EXISync`: 80 segments for the mount, 16 for a
/// block); the reads were fitted to the sessions' traces (`docs/saves.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardTime {
    /// `lb_8001BD34` loading the save. With the file opened (intact or damaged): the
    /// mount, then 23 steps, twenty of which read a block each (steps 2 to 13 and 15 to 22:
    /// the file's eleven blocks, a block a step, twice over for the data kept twice and the
    /// spare). Without (no file, a full card, a broken one): as [`CardTime::Mount`].
    Load,
    /// A list that only mounts the card and looks at its directory (`lb_8001C2D8` looking
    /// for another game's file, `lb_8001C87C` probing for the save): the mount, then one
    /// step (the check, the free blocks and the open run from what the mount read).
    Mount,
}

/// Register reads of the mount's transfers (80 segments: the ID, the directories and the
/// allocation tables, with the status commands around them). The search's preload issue
/// (62,195,550 on the traces) is linear in it and the search's step, which fixes it.
pub const MOUNT_WORK: u64 = 1643;
/// Register reads of the step after the mount: the check's ([`CHECK_WORK`]), and in the
/// load's list, when the card isn't broken, one more ([`LOAD_WORK`]). Fitted to the no-file,
/// full and broken cards' sessions, whose loads and probes end on that step.
pub const CHECK_WORK: u64 = 2;
pub const LOAD_WORK: u64 = 1;
/// Register reads of a block's read (sixteen 512-byte segments). Only the last block's
/// reads reach the load's end (the wait points before it end at events), so the traces fix
/// the last step's work but not its split; 323 keeps the mount's rate of about 20 a
/// segment.
pub const BLOCK_WORK: u64 = 323;

impl CardTime {
    /// The first task's work, and each step's work after its wait point, for a list that
    /// ended on `result` and found a card in the slot or not (no card: the probe fails, and
    /// the list with it, before any wait).
    fn steps(self, result: i32, card_in: bool) -> (u64, Vec<u64>) {
        if !card_in {
            return (0, Vec::new());
        }
        match (self, result) {
            (CardTime::Load, result::READY | result::DAMAGED) => (
                MOUNT_WORK,
                (1..=23)
                    .map(|s| match s {
                        1 => CHECK_WORK + LOAD_WORK,
                        14 | 23 => 0,
                        _ => BLOCK_WORK,
                    })
                    .collect(),
            ),
            (CardTime::Load, result::NO_FILE) => (MOUNT_WORK, vec![CHECK_WORK + LOAD_WORK]),
            (CardTime::Load, result::MALFORMED) | (CardTime::Mount, _) => {
                (MOUNT_WORK, vec![CHECK_WORK])
            }
            (CardTime::Load, r) => unported_call(&format!(
                "the card's time over a load that ends on lbCardResult {r}"
            )),
        }
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

/// The card data as memory holds it.
fn card_data_bytes(d: &GmCardData) -> Vec<u8> {
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
        let seconds =
            self.game.audio.clock.as_ref().map_or(0, |c| c.now) / crate::audio::clock::TB_HZ;
        let thing = &self.game.game_data.thing;
        let memory = card_data_bytes(thing);
        let mut bufs = melee_save::exec::Bufs::default();
        for (i, f) in bufs.files.iter_mut().enumerate().skip(1) {
            *f = memory[file_span(i).0..].to_vec();
        }
        let mut run = Run::new(melee_save::melee::manifest(), bufs);
        self.card_alarm_probes();
        let now = self.clock_now();
        let (card_in, probe) = with_device(|dev| dev.exi_probe_timed(now));
        // The mount's probe found the card: the lists that take the card game's status clear
        // it (`*_p(status) = 0`); the others pass none.
        if card_in && status {
            self.game.card_game.unk_status = false;
        }
        let r = with_device(|dev| {
            let mut clock = Clock::at((dev.clock + seconds) as u32);
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
        let (first, steps) = time.steps(r, card_in);
        // A list without a card that answers fails at the mount's probe (`CARDProbeEx`, with
        // interrupts off), which takes no time where ssbm-rs stands in for the CARD library.
        if !card_in && with_device(|dev| dev.waits) {
            self.game.audio.clock_read(self.data, probe);
        }
        // The mount attaches slot A's channel, and the list's end unmounts it.
        let mounted = !steps.is_empty();
        with_device(|dev| dev.attached = mounted);
        self.game.audio.clock_reg_reads(self.data, first);
        for work in steps {
            self.game.audio.clock_wait(self.data);
            self.game.audio.clock_reg_reads(self.data, work);
        }
        // The unmount's detach writes the status register back with the card's EXT bit.
        let now = self.clock_now();
        with_device(|dev| {
            dev.attached = false;
            dev.ext_held |= mounted && dev.present(now);
        });
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
            ext_held: true,
            pull: Some((100 * TENTH, Some(200 * TENTH))),
            ..CardDevice::default()
        };
        // Settled: the status register and the time.
        assert_eq!(dev.exi_probe_timed(10 * TENTH), (true, 24));
        // Attached, the register alone.
        dev.attached = true;
        assert_eq!(dev.exi_probe_timed(11 * TENTH), (true, 8));
        dev.attached = false;
        // Pulled: the insertion interrupt read and cleared, the held EXT bit read as a card
        // (the time), then no card.
        assert_eq!(dev.exi_probe_timed(101 * TENTH), (false, 32));
        assert_eq!(dev.exi_probe_timed(102 * TENTH), (false, 8));
        // Put back: the interrupt and the time, settling, then its ID read once it answers.
        assert_eq!(dev.exi_probe_timed(201 * TENTH), (false, 32));
        assert_eq!(dev.exi_probe_timed(202 * TENTH), (false, 24));
        assert_eq!(dev.exi_probe_timed(205 * TENTH), (true, 24 + ID_READ));
        assert_eq!(dev.exi_probe_timed(206 * TENTH), (true, 24));
    }
}
