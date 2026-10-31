//! Camera Mode's snapshots as the album reads them back (`lbsnap.c`'s reading side): what the
//! game asks of a slot's list (`lbSnap_8001D350`, `D394`, `D3E8`), a snapshot read into the
//! snapshot's buffer with its header (`lbSnap_8001E058`, `lb_8001BF04`), deleted
//! (`lbSnap_8001D5FC`, `lbCardNew_DeleteSnap`) or swapped with another by renaming both
//! (`lbSnap_8001D7B0`, `lb_8001C0F4`), and the buffer's image decoded (`lbSnap_8001DE8C`).
//!
//! The album steps each list itself (`lbCardNew_CompleteNextTask`) in the tick that starts
//! it: the list runs on the lent card at its start ([`super::save::start_list_run`]), its steps'
//! time passing at each call ([`Running::snap_task_step`]), and a read's bytes reach the
//! buffers at its end. A rename's stamps take the clock at their steps, so the swap runs on a
//! copy of the card first and on the card at its end, as the snapshot's save does.

use melee_save::tasks::{Run, Task, lists};

use super::save::{CardList, STATUS_SET, result};
use super::snap::SNAP_SIZE;
use super::snap_write::{HEADER, SnapHeader};
use crate::audio::clock::{CARD_ERASE, CardChain, CardWork};
use crate::game::code::unported_call;
use crate::game::state::Running;
use crate::hsd::heap;
use crate::hsd::jpeg_decode;

/// The album's images (`mnSnap_804A0B90`'s buffers): 640 by 480 RGB565.
pub const IMAGE_SIZE: u32 = 0x96000;

/// What a list does at its end besides its result.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub enum ListEnd {
    #[default]
    Nothing,
    /// A read: the file's data, comment, banner and icons as it writes them, into the
    /// snapshot's buffer, `lbSnap_80433380.filename` and `LbMcSnap`'s icon data, each part at
    /// the step whose read brings it (`puts`), `into` what the steps so far wrote.
    Read { into: ReadInto, puts: Vec<ReadPut> },
    /// A swap's renames, run on the card now with the times its stamps took.
    Swap {
        tasks: Vec<Vec<u8>>,
        seconds: u32,
        stamps: Vec<usize>,
        times: Vec<u32>,
    },
    /// A format, run on the card now with the time its step's `OSGetTime` read (`ticks`, on
    /// the code's time base), which the new ID's serial takes; `step` the step that starts
    /// it.
    Format { step: usize, ticks: Option<u64> },
}

/// What a read wrote into its buffers: each the bytes from its start.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct ReadInto {
    pub data: Vec<u8>,
    pub comment: Vec<u8>,
    pub banner: Vec<u8>,
    pub icons: Vec<u8>,
}

/// A copy a read's step makes into its buffers: the step, the buffer (0 the file's data,
/// 1 the comment, 2 the banner, 3 the icons), where in it, and the bytes.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct ReadPut {
    pub step: usize,
    pub buf: u8,
    pub off: u32,
    pub bytes: Vec<u8>,
}

/// The step of a list's steps (as `save_steps` makes them) each entry of the HSD layer's log
/// falls in: a transfer's own, else the last transfer's (0, the list's first, before any).
fn log_steps(log: &[u8]) -> Vec<usize> {
    let mut out = Vec::with_capacity(log.len());
    let mut idx = 1usize;
    let mut cur = 0usize;
    let mut tasks = 0;
    let mut transfers = false;
    for &c in log {
        match c {
            b't' => {
                if tasks > 0 && transfers {
                    cur = idx;
                    idx += 1;
                }
                tasks += 1;
                transfers = false;
            }
            b'r' | b'w' | b's' | b'c' => {
                transfers = true;
                cur = idx;
                idx += 1;
            }
            _ => {}
        }
        out.push(cur);
    }
    out
}

/// A list the album started and steps through on slot `chan`: its steps, the next, and its
/// end.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct AlbumList {
    pub chan: usize,
    pub list: CardList,
    pub next: usize,
    pub end: ListEnd,
}

/// One of the album's images as the decoder left it: its buffer's address and bytes (the
/// heap's image's before the first decode into it, unknown bytes as zero).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct AlbumImage {
    pub addr: u32,
    pub bytes: Vec<u8>,
    /// How many decodes wrote it, for the display's copy.
    pub version: u32,
}

/// `lbsnap.c`'s reading side's state: the list under way, the JPEG decoder's statics, the
/// icons a read wrote into `LbMcSnap`'s icon data, and the images decoded.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct SnapRead {
    pub list: Option<AlbumList>,
    pub decoder: jpeg_decode::Decoder,
    pub icons: Option<Vec<u8>>,
    pub images: Vec<AlbumImage>,
}

/// A step's work after its wait point: `chain`'s transfers.
fn chain_step(chain: CardChain) -> CardWork {
    CardWork {
        step: true,
        chain: Some(chain),
        ..CardWork::default()
    }
}

/// The step `save_steps` makes at each task's start after the first that follows transfers
/// (the call that sees the task before done and starts the next), by the task's place in the
/// log, as it walks the HSD layer's log.
fn task_steps(log: &[u8]) -> Vec<Option<usize>> {
    let mut out = Vec::new();
    let mut idx = 1usize;
    let mut tasks = 0;
    let mut transfers = false;
    for &c in log {
        match c {
            b't' => {
                if tasks > 0 {
                    if transfers {
                        out.push(Some(idx));
                        idx += 1;
                    } else {
                        out.push(None);
                    }
                }
                tasks += 1;
                transfers = false;
            }
            b'r' | b'w' | b's' | b'c' => {
                transfers = true;
                idx += 1;
            }
            _ => {}
        }
    }
    out
}

/// Inserts a step at `at`, moving the results of later steps along.
fn insert_step(list: &mut CardList, at: usize, work: CardWork) {
    list.steps.insert(at, work);
    for r in &mut list.results {
        if r.0 >= at {
            r.0 += 1;
        }
    }
}

impl Running<'_> {
    /// The slot's last result, 8 (a card came or went since its listing) in place of a
    /// listing's 0 once the pad alarm's probes saw a change (each starting function's
    /// preamble, and `lbSnap_8001D350` whole).
    fn snap_fresh(&mut self, chan: usize) -> i32 {
        self.card_alarm_probes();
        let s = &mut self.game.snap;
        if s.slots[chan].card_result == 0 && s.state_changed[chan] != 0 {
            s.slots[chan].card_result = 8;
        }
        s.slots[chan].card_result
    }

    /// `lbSnap_8001D350`: slot `chan`'s last result, 8 once a card came or went since its
    /// listing.
    #[doc(alias = "lbSnap_8001D350")]
    pub fn snap_card_result(&mut self, chan: usize) -> i32 {
        self.snap_fresh(chan)
    }

    /// `lbSnap_8001D394`: the snapshots slot `chan`'s last listing found.
    #[doc(alias = "lbSnap_8001D394")]
    pub fn snap_count(&self, chan: usize) -> i32 {
        self.game.snap.slots[chan]
            .num
            .expect("a slot's count before its listing")
    }

    /// `lbSnap_8001D3E8`: the blocks of slot `chan`'s snapshot `index`.
    #[doc(alias = "lbSnap_8001D3E8")]
    pub fn snap_entry_blocks(&self, chan: usize, index: i32) -> i32 {
        let e = self.game.snap.slots[chan].entries[index as usize];
        i32::from(e.blocks.expect("a listed snapshot's blocks"))
    }

    /// `lbSnap_FormatTime`: snapshot `index`'s name, its time's digits.
    fn snap_entry_name(&self, chan: usize, index: i32) -> Vec<u8> {
        let slot = &self.game.snap.slots[chan];
        let num = slot.num.expect("a listed slot");
        assert!(
            index < num,
            "the original's assertion failed: index < _p(slot)[chan].num"
        );
        let time = slot.entries[index as usize]
            .time
            .expect("a listed snapshot's time");
        time.to_string().into_bytes()
    }

    /// `lbSnap_8001D5FC`: snapshot `index` of slot `chan` deleted (`lbCardNew_DeleteSnap`),
    /// the slot's result 8 until it's listed again; the list's start's result (busy), or the
    /// slot's when its last listing failed or a card came or went.
    #[doc(alias = "lbSnap_8001D5FC")]
    #[doc(alias = "lbCardNew_DeleteSnap")]
    pub fn snap_delete(&mut self, chan: usize, index: i32) -> i32 {
        let r = self.snap_fresh(chan);
        if r != 0 {
            return r;
        }
        let name = self.snap_entry_name(chan, index);
        self.game.snap.slots[chan].card_result = 8;
        self.snap_list_start(chan, lists::delete(&name), ListKind::Delete)
    }

    /// `lbSnap_8001D7B0`: snapshots `index` and `jndex` of slot `chan` swap names through a
    /// new one (`lb_8001C0F4`), the slot's result 8 until it's listed again; the list's
    /// start's result (busy), or the slot's when its last listing failed or a card came or
    /// went.
    #[doc(alias = "lbSnap_8001D7B0")]
    #[doc(alias = "lb_8001C0F4")]
    pub fn snap_swap(&mut self, chan: usize, index: i32, jndex: i32) -> i32 {
        let r = self.snap_fresh(chan);
        if r != 0 {
            return r;
        }
        let a = self.snap_entry_name(chan, index);
        let b = self.snap_entry_name(chan, jndex);
        let c = self.snap_name(chan).into_bytes();
        self.game.snap.slots[chan].card_result = 8;
        self.snap_list_start(chan, lists::swap(&a, &b, &c), ListKind::Swap)
    }

    /// `lbSnap_8001E058`: snapshot `index` of slot `chan` read into the snapshot's buffer,
    /// its comment into `lbSnap_80433380.filename` and its banner and icon into `LbMcSnap`'s
    /// icon data (`lb_8001BF04`); the list's start's result (busy), or the slot's when its
    /// last listing failed or a card came or went.
    #[doc(alias = "lbSnap_8001E058")]
    #[doc(alias = "lb_8001BF04")]
    pub fn snap_read(&mut self, chan: usize, index: i32) -> i32 {
        let r = self.snap_fresh(chan);
        if r != 0 {
            return r;
        }
        let name = self.snap_entry_name(chan, index);
        self.snap_list_start(chan, lists::read_with_header(&name), ListKind::Read)
    }

    /// `lb_8001CC4C`: the save's file deleted from slot A's card (`lb_8001BA44` with the save's
    /// name and the card game's status), the game spinning on the list's steps
    /// (`lbCardNew_CompleteNextTask`) until it ends. Its result.
    #[doc(alias = "lb_8001CC4C")]
    #[doc(alias = "lb_8001BA44")]
    pub fn card_delete_save(&mut self) -> i32 {
        let r = self.snap_list_start(
            0,
            lists::delete(melee_save::melee::FILENAME),
            ListKind::DeleteSave,
        );
        self.card_spin(r)
    }

    /// `lb_8001B8C8`: slot `chan`'s card formatted (its mount, check and `CARDFormatAsync`),
    /// the game spinning on the list's steps until it ends. Its result.
    #[doc(alias = "lb_8001B8C8")]
    pub fn card_format(&mut self, chan: usize) -> i32 {
        let r = self.snap_list_start(chan, lists::format(0), ListKind::Format);
        self.card_spin(r)
    }

    /// The game spinning on a list started with `r` until it ends (`lbCardNew_CompleteNextTask`
    /// while busy, the Yoshiki Test's `waitCardTasks`). Its result.
    pub fn card_spin(&mut self, r: i32) -> i32 {
        let mut r = r;
        while r == result::BUSY {
            r = self.card_next_task();
        }
        r
    }

    /// Starts one of the album's lists on slot `chan`: the mount at once, the steps left for
    /// [`Running::snap_task_step`]. Busy, or the mount's failure.
    fn snap_list_start(&mut self, chan: usize, tasks: Vec<(Task, u32)>, kind: ListKind) -> i32 {
        if self.game.snap_read.list.is_some() {
            unported_call("an album list started while another runs");
        }
        self.card_alarm_probes();
        // `resetState`: no file has a result yet.
        self.game.card_tasks.file_errors = [result::START; 9];
        let now = self.game.audio.clock.as_ref().map_or(0, |c| c.now);
        let seconds = self.session_seconds();
        let run = Run::new(
            melee_save::melee::snapshot_manifest(0),
            melee_save::exec::Bufs::default(),
        );
        let (mut list, run) = match kind {
            ListKind::Swap | ListKind::Format => {
                super::save::start_list_dry_run(chan, run, &tasks, now, seconds)
            }
            _ => super::save::start_list_run(chan, run, &tasks, now, seconds),
        };
        for &w in &list.start {
            self.game.audio.clock_card_work(self.data, w);
        }
        if !list.card_in {
            self.game.card_tasks.saved_error = list.result;
            return list.result;
        }
        // The mount's probe found the card: a list that takes the card game's status clears
        // it (`*_p(status) = 0`).
        if kind == ListKind::DeleteSave {
            self.game.card_game.unk_status = false;
        }
        let mut format_step = None;
        // `OSGetFontEncode` reads the video interface's register for a format when no mount
        // read it before (a broken card's doesn't get that far).
        let format_font = if kind == ListKind::Format {
            super::save::font_encode_read()
        } else {
            0
        };
        // The CARD library's own tasks (a delete, the renames) transfer from the call that
        // starts them: the one that sees the open before them done, after which another
        // call starts the next task (`executeNextTask`), or ends the list.
        let sdk = |t: &Task| {
            matches!(
                t,
                Task::Delete(_) | Task::Rename { .. } | Task::Format { .. }
            )
        };
        let ran: Vec<&Task> = run
            .results
            .iter()
            .map(|(t, _)| t)
            .filter(|t| sdk(t))
            .collect();
        let mut stamps = Vec::new();
        let starts = task_steps(&run.steps);
        let mut added = 0;
        for (k, t) in ran.iter().enumerate() {
            let chain = match t {
                Task::Delete(_) => CardChain::Delete {
                    first: CARD_ERASE,
                    slot_b: false,
                }
                .on(chan),
                Task::Format { .. } => CardChain::Format {
                    first: format_font + FORMAT_FIRST,
                    slot_b: false,
                }
                .on(chan),
                _ => STATUS_SET.on(chan),
            };
            if matches!(t, Task::Format { .. }) {
                // A format follows the check with no open between: the call that sees the
                // mount done runs the check (from what the mount read) and goes on to start
                // it.
                list.steps[0].chain = Some(chain);
                format_step = Some(0);
                continue;
            }
            // The open after this task (a swap's): the step that starts it, else the
            // list's end.
            let at = match starts.get(k).copied().flatten() {
                Some(i) => i + added,
                None => list.steps.len() - 1,
            };
            let last = at == list.steps.len() - 1;
            if last {
                // The list's end: the task's call, then the unmount's.
                let unmount = list.steps[at];
                list.steps[at] = chain_step(chain);
                list.steps.push(unmount);
            } else {
                list.steps[at].chain = Some(chain);
                insert_step(
                    &mut list,
                    at + 1,
                    CardWork {
                        step: true,
                        ..CardWork::default()
                    },
                );
                added += 1;
            }
            if matches!(t, Task::Rename { .. }) {
                stamps.push(at);
            }
        }
        let end = match kind {
            ListKind::Read => {
                let at = log_steps(&run.steps);
                let puts = run
                    .puts
                    .iter()
                    .filter_map(|(n, p, bytes)| {
                        let buf = match p.buf {
                            melee_save::hsd::Buf::File(0) => 0,
                            melee_save::hsd::Buf::Comment => 1,
                            melee_save::hsd::Buf::Banner => 2,
                            melee_save::hsd::Buf::Icons => 3,
                            _ => return None,
                        };
                        Some(ReadPut {
                            step: n.checked_sub(1).map_or(0, |i| at[i]),
                            buf,
                            off: p.off,
                            bytes: bytes.clone(),
                        })
                    })
                    .collect();
                ListEnd::Read {
                    into: ReadInto::default(),
                    puts,
                }
            }
            ListKind::Swap => ListEnd::Swap {
                tasks: tasks
                    .iter()
                    .filter_map(|(t, _)| match t {
                        Task::Open { name: Some(n), .. } => Some(n.clone()),
                        _ => None,
                    })
                    .collect(),
                seconds,
                stamps,
                times: Vec::new(),
            },
            ListKind::Delete | ListKind::DeleteSave => ListEnd::Nothing,
            ListKind::Format => match format_step {
                Some(step) => ListEnd::Format { step, ticks: None },
                None => ListEnd::Nothing,
            },
        };
        self.game.snap_read.list = Some(AlbumList {
            chan,
            list,
            next: 0,
            end,
        });
        result::BUSY
    }

    /// `lbCardNew_CompleteNextTask` from the album and Camera Mode: a step of the list under
    /// way (the album's read, delete or swap, or a snapshot's save), busy until its last; with
    /// none, its wait point and the last list's result (`saved_error`).
    pub fn card_next_task(&mut self) -> i32 {
        if self.game.snap_read.list.is_some() {
            return self.snap_task_step();
        }
        if self.game.snap_write.save.is_some() {
            return self.snap_save_step();
        }
        self.card_complete_all_tasks();
        self.game.card_tasks.saved_error
    }

    /// `lbCardNew_CompleteNextTask` for the album's list: its wait point and the next step's
    /// transfers; busy until its last step, which returns the list's result and, for a read,
    /// hands its bytes to the buffers.
    fn snap_task_step(&mut self) -> i32 {
        let Some(l) = self.game.snap_read.list.as_mut() else {
            unported_call("lbCardNew_CompleteNextTask with no album list")
        };
        let work = l.list.steps[l.next];
        let k = l.next;
        l.next += 1;
        let last = l.next == l.list.steps.len();
        let results: Vec<(usize, i32)> = l
            .list
            .results
            .iter()
            .filter(|(at, _, _)| *at == k)
            .map(|&(_, f, e)| (f, e))
            .collect();
        for (f, e) in results {
            self.game.card_tasks.file_errors[f] = e;
        }
        let chan = self.game.snap_read.list.as_ref().map_or(0, |l| l.chan);
        self.game.audio.clock_card_work(
            self.data,
            crate::audio::clock::CardWork {
                step: work.step,
                ..Default::default()
            },
        );
        if super::save::lost(chan) {
            return self.snap_task_lost(work);
        }
        let time_at = match work.chain {
            Some(CardChain::Format { first, .. }) => first - FORMAT_FIRST + FORMAT_TIME_AT,
            _ => 0,
        };
        if let Some(AlbumList {
            end: ListEnd::Format { step, ticks },
            ..
        }) = self.game.snap_read.list.as_mut()
            && *step == k
        {
            // `__CARDFormatRegionAsync`'s `OSGetTime`, after the video interface's register.
            let c = self.game.audio.clock.as_ref().expect("the timeline");
            *ticks = Some(c.code_time() + time_at);
        }
        self.game.audio.clock_card_work(
            self.data,
            crate::audio::clock::CardWork {
                step: false,
                ..work
            },
        );
        // A read's bytes reach their buffers in the step whose read brings them.
        let into = match self.game.snap_read.list.as_mut() {
            Some(AlbumList {
                end: ListEnd::Read { into, puts },
                ..
            }) if puts.iter().any(|p| p.step == k) => {
                for p in puts.iter().filter(|p| p.step == k) {
                    let b = match p.buf {
                        0 => &mut into.data,
                        1 => &mut into.comment,
                        2 => &mut into.banner,
                        _ => &mut into.icons,
                    };
                    let (lo, hi) = (p.off as usize, p.off as usize + p.bytes.len());
                    if b.len() < hi {
                        b.resize(hi, 0);
                    }
                    b[lo..hi].copy_from_slice(&p.bytes);
                }
                Some(into.clone())
            }
            _ => None,
        };
        if let Some(into) = into {
            self.snap_read_into(into);
        }
        let stamped = matches!(&self.game.snap_read.list, Some(AlbumList { end: ListEnd::Swap { stamps, .. }, .. }) if stamps.contains(&k));
        if stamped {
            let now = self.session_seconds();
            if let Some(AlbumList {
                end: ListEnd::Swap { times, .. },
                ..
            }) = self.game.snap_read.list.as_mut()
            {
                times.push(now);
            }
        }
        if !last {
            return result::BUSY;
        }
        let l = self.game.snap_read.list.take().expect("the album's list");
        let r = l.list.result;
        self.game.card_tasks.saved_error = r;
        match l.end {
            ListEnd::Nothing | ListEnd::Read { .. } => {}
            ListEnd::Format { ticks, .. } => {
                let run = Run::new(
                    melee_save::melee::snapshot_manifest(0),
                    melee_save::exec::Bufs::default(),
                );
                let ticks = ticks.expect("the format's time");
                let written = super::save::finish_list(l.chan, run, &lists::format(ticks), 0, &[]);
                assert_eq!(written, r, "the format's list ended otherwise on the card");
            }
            ListEnd::Swap {
                tasks,
                seconds,
                times,
                ..
            } => {
                let [a, b, c] = <[Vec<u8>; 3]>::try_from(tasks).unwrap_or_else(|t| {
                    unported_call(&format!("a swap whose list opened {} files", t.len()))
                });
                let run = Run::new(
                    melee_save::melee::snapshot_manifest(0),
                    melee_save::exec::Bufs::default(),
                );
                let written = super::save::finish_list(
                    l.chan,
                    run,
                    &lists::swap(&a, &b, &c),
                    seconds,
                    &times,
                );
                assert_eq!(written, r, "the swap's list ended otherwise on the card");
            }
        }
        r
    }

    /// The album's list's step after its card was pulled out (`work` the step's work but its
    /// wait point): a read's step, whose `CARDFastOpen` finds no card, its callback
    /// (`fn_8001A0B0`) giving file 0 (the header's and the data's requests are file 0's) and
    /// the list the result (15); the list's end task's `CARDUnmount` finds no card either, so
    /// nothing more takes time, and the steps' reads after bring nothing. A pull before
    /// another step (the open's, a delete's or a rename's) isn't ported, as no session
    /// reaches one.
    fn snap_task_lost(&mut self, work: CardWork) -> i32 {
        let l = self.game.snap_read.list.take().expect("the album's list");
        let k = l.next - 1;
        if !matches!(
            (&l.end, work.chain),
            (ListEnd::Read { .. }, Some(CardChain::Read { .. }))
        ) {
            unported_call(&format!(
                "the album's list with its card pulled out before its step {k}, not a read's"
            ));
        }
        let r = melee_save::tasks::from_sdk(melee_save::card::CardError::NoCard.code());
        self.game.card_tasks.file_errors[0] = r;
        super::save::detach_slot(l.chan);
        self.game.card_tasks.saved_error = r;
        r
    }

    /// A read's bytes where its list wrote them: the file's into the snapshot's buffer (its
    /// header and image, as `lbSnap_8001DC0C` leaves them), the comment into
    /// `lbSnap_80433380.filename`, the banner and the icon into `LbMcSnap`'s icon data.
    fn snap_read_into(&mut self, read: ReadInto) {
        let snap = self.game.snap.snap.expect("the snapshot's buffer");
        if read.data.len() > SNAP_SIZE as usize {
            unported_call("a snapshot read past its buffer");
        }
        heap::with(|h| h.image.put(snap, &read.data));
        let d = &read.data;
        if d.len() >= HEADER as usize {
            let be32 = |at: usize| i32::from_be_bytes(d[at..at + 4].try_into().unwrap());
            let be16 = |at: usize| u16::from_be_bytes([d[at], d[at + 1]]);
            let header = SnapHeader {
                x0: be32(0),
                width: be16(4),
                height: be16(6),
                x8: d[8],
                xc: be32(0xC),
                stkind: be16(0x10),
                items: std::array::from_fn(|i| be32(0x14 + 4 * i)),
                fighters: be32(0x34),
            };
            let end = (HEADER as usize + header.xc.max(0) as usize).min(d.len());
            let w = &mut self.game.snap_write;
            w.header = Some(header);
            w.image = d[HEADER as usize..end].to_vec();
        }
        let w = &mut self.game.snap_write;
        if !read.comment.is_empty() {
            let mut f = w.filename.unwrap_or([0; 0x40]);
            let n = read.comment.len().min(0x40);
            f[..n].copy_from_slice(&read.comment[..n]);
            w.filename = Some(f);
        }
        let icon = self.game.snap.icon_data.expect("LbMcSnap's icon data");
        let at = |i: u32| self.data.ptr(icon.at(4 * i)).expect("an icon data pointer");
        let (banner_at, icons_at) = (at(0), at(1));
        if !read.banner.is_empty() {
            let mut b = self.game.snap_write.banner.clone().unwrap_or_else(|| {
                (0..read.banner.len() as u32)
                    .map(|i| self.data.u8(banner_at.at(i)))
                    .collect()
            });
            let n = read.banner.len().min(b.len());
            b[..n].copy_from_slice(&read.banner[..n]);
            crate::hsd::export::with_image(|img| {
                if let Some(a) = img.data_addr(banner_at) {
                    img.put(a, &b);
                }
            });
            self.game.snap_write.banner = Some(b);
        }
        if !read.icons.is_empty() {
            crate::hsd::export::with_image(|img| {
                if let Some(a) = img.data_addr(icons_at) {
                    img.put(a, &read.icons);
                }
            });
            self.game.snap_read.icons = Some(read.icons);
        }
    }

    /// `lbSnap_8001DE8C`: the snapshot's buffer's image decoded into the album's image at
    /// `dst` (`hsd_803B6BE4`, then `DCFlushRange`), when its header says it's an image (kind
    /// 4); whether it decoded.
    #[doc(alias = "lbSnap_8001DE8C")]
    pub fn snap_decode(&mut self, dst: u32) -> i32 {
        let snap = self.game.snap.snap.expect("the snapshot's buffer");
        let data = self.data;
        let (bytes, known) =
            heap::with(|h| h.image.bytes(data, snap, SNAP_SIZE)).expect("the heap's image");
        let byte = |i: usize| -> u8 {
            if !known[i] {
                unported_call(&format!(
                    "the snapshot's buffer's byte {i:#x}, which the heap's image doesn't know"
                ));
            }
            bytes[i]
        };
        let be32 =
            |at: usize| u32::from_be_bytes([byte(at), byte(at + 1), byte(at + 2), byte(at + 3)]);
        if be32(0) != 4 {
            return 0;
        }
        let size = be32(0xC) as i32;
        let width = u16::from_be_bytes([byte(4), byte(5)]);
        let height = u16::from_be_bytes([byte(6), byte(7)]);
        let i = match self
            .game
            .snap_read
            .images
            .iter()
            .position(|m| m.addr == dst)
        {
            Some(i) => i,
            None => {
                let (b, _) =
                    heap::with(|h| h.image.bytes(data, dst, IMAGE_SIZE)).expect("the heap's image");
                self.game.snap_read.images.push(AlbumImage {
                    addr: dst,
                    bytes: b,
                    version: 0,
                });
                self.game.snap_read.images.len() - 1
            }
        };
        let mut image = std::mem::take(&mut self.game.snap_read.images[i].bytes);
        self.game.snap_read.decoder.start = snap + HEADER;
        let done = jpeg_decode::decode_counted(
            &mut self.game.snap_read.decoder,
            |k| byte(HEADER as usize + k as usize),
            size,
            &mut image,
        );
        let r = done.result;
        // The macroblocks it stored, into the heap's image.
        for (lo, hi) in jpeg_decode::stored_spans(done.width, done.stored) {
            heap::with(|h| h.image.put(dst + lo, &image[lo as usize..hi as usize]));
        }
        let img = &mut self.game.snap_read.images[i];
        img.bytes = image;
        img.version = img.version.wrapping_add(1);
        // `DCFlushRange` over the header's size.
        let _ = (width, height);
        i32::from(r != 0)
    }
}

/// The album's lists, by what their end does, and the Yoshiki Test's that the game waits on
/// in a spin (`lb_8001BA44`'s delete of the save, which takes the card game's status, and
/// `lb_8001B8C8`'s format).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ListKind {
    Read,
    Delete,
    Swap,
    DeleteSave,
    Format,
}

/// A format's part from its step's call to its first erase's end, with interrupts off, past
/// `OSGetFontEncode`'s read when it makes one: `__CARDFormatRegionAsync`'s read of the video
/// interface's register (8), its `OSGetTime` (16), then the erase.
const FORMAT_FIRST: u64 = super::save::EXI_READ + super::save::TIME_READ + CARD_ERASE;
/// Where in the step its `OSGetTime` reads the time base: after the register.
const FORMAT_TIME_AT: u64 = super::save::EXI_READ;
