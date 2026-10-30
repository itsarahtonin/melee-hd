//! The game's card operations as `lbcardnew` runs them: a list of tasks (mount, check, open,
//! read, write and the rest), each starting only when the one before ended with a result its
//! mask accepts, and the result the game sees. Each task's card and HSD calls complete at once,
//! so a list runs to its end in one call; `lbCardNew_CompleteNextTask`'s step counts are the
//! caller's to model.

use crate::card::{Card, CardError, Clock, Console, Mounted};
use crate::exec::{Bufs, Callback, Layer, Request};
use crate::state::IconInfo;

/// `lbCardResult`.
pub mod lb {
    pub const READY: i32 = 0;
    /// After an open that found the file: `taskUnk3` turns 0 and 2 into 1.
    pub const OPENED: i32 = 1;
    /// A file damaged: a block missing or out of step, or failing its checksum.
    pub const DAMAGED: i32 = 2;
    /// The file isn't one the layer can read: its block count, header or icon address.
    pub const BAD_FILE: i32 = 3;
    pub const NO_FILE: i32 = 4;
    /// No file, and too few free blocks for one.
    pub const NO_SPACE: i32 = 5;
    /// No file, and no free directory entry.
    pub const NO_ENTRY: i32 = 6;
    pub const NULL_FILENAME: i32 = 7;
    /// The card is broken or of another encoding.
    pub const MALFORMED: i32 = 9;
    pub const BUSY_CARD: i32 = 10;
    pub const BAD_SECTOR_SIZE: i32 = 12;
    pub const INVALID: i32 = 13;
    pub const FATAL_ERROR: i32 = 14;
    /// No card, or not a memory card.
    pub const DEVICE_ERROR: i32 = 15;
    /// Before any task.
    pub const START: i32 = 16;
}

/// `convertSdkResult`.
pub fn from_sdk(code: i32) -> i32 {
    match code {
        -3..=-1 => lb::DEVICE_ERROR,
        -4 => lb::NO_FILE,
        0 => lb::READY,
        -5 | -128 => lb::FATAL_ERROR,
        -6 | -13 => lb::MALFORMED,
        _ => lb::INVALID,
    }
}

/// `convertHsdResult`.
pub fn from_hsd(code: i32) -> i32 {
    match code {
        0 | 1 => lb::READY,
        -261..=-258 => lb::DAMAGED,
        -263 | -262 | -257 => lb::BAD_FILE,
        -267..=-264 => lb::BUSY_CARD,
        _ => from_sdk(code),
    }
}

/// One logical file of a card file's manifest (`LbCardEntry`): its size (0 for none) and how
/// it's kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub size: i32,
    pub flags: i32,
}

/// What a game's card file holds: its banner and icons' formats and its logical files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub icon: IconInfo,
    pub files: Vec<Entry>,
}

/// A task (`CardTask`) and the results of the task before that let it run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Task {
    Mount,
    Check,
    /// Opens `name` if it's there; with a manifest, says whether one could be made.
    Open {
        name: Option<Vec<u8>>,
        with_manifest: bool,
    },
    Unk3,
    Format {
        ticks: u64,
    },
    Delete(Vec<u8>),
    /// `taskRename`: `old` named `new`.
    Rename {
        old: Vec<u8>,
        new: Vec<u8>,
    },
    Create(Vec<u8>),
    Read,
    Write,
    SetStatus,
    ReadHeader,
    ListSnapshots,
    FindFile {
        company: [u8; 2],
        game: [u8; 4],
        name: Vec<u8>,
    },
}

/// A snapshot as `taskListSnapshots` lists it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub time: u32,
    pub file_no: i32,
    pub blocks: u16,
}

/// What a task list runs with and leaves.
#[derive(Clone, Debug)]
pub struct Run {
    pub manifest: Manifest,
    pub bufs: Bufs,
    /// The comment a create or a status update writes (`hsd_803B286C` and `hsd_803B2928`
    /// copy it into the state).
    pub comment: [u8; 0x40],
    /// Each task's result, in order, for the tasks that ran.
    pub results: Vec<(Task, i32)>,
    /// Each logical file's last HSD result (`unk_38`'s), -1 for none.
    pub file_results: [i32; 9],
    /// The mount's and check's own results (`CARD_RESULT_*`), when they ran.
    pub mount: Option<i32>,
    pub check: Option<i32>,
    /// Every HSD callback, in order.
    pub callbacks: Vec<Callback>,
    pub snapshots: Vec<Snapshot>,
    /// What the HSD layer asked of the card over the list, as [`Layer::log`] records it,
    /// with `t` where a task that makes requests starts and a `k` for each callback (in
    /// [`Run::callbacks`]' order) after its request's transfers.
    pub steps: Vec<u8>,
    /// The files the read or write task queued, each with what queueing it returned (0, or
    /// the refusal), in order: the results `taskRead` and `taskWrite` set before any runs.
    pub queued: Vec<(usize, i32)>,
    /// Where in them the write task started, queueing its files' writes (each file's result
    /// set then): its `t`.
    pub write_queued: Option<usize>,
    pub free_bytes: i32,
    pub free_files: i32,
    pub layer: Layer,
    /// Each copy the HSD layer made into the buffers (`Layer::puts`), with how many of
    /// `steps` came before it.
    pub puts: Vec<(usize, crate::hsd::Ptr, Vec<u8>)>,
    /// The HSD layer's transfers the card answers before it's pulled out of the slot
    /// ([`Layer::budget`]), none for a card left in.
    pub pull_after: Option<usize>,
}

/// Whether a log entry of the HSD layer's ([`Run::steps`]) is a transfer: a sector read or
/// written, a status set or a file created.
pub fn is_transfer(c: u8) -> bool {
    matches!(c, b'r' | b'w' | b's' | b'c')
}

impl Run {
    /// A run for a manifest, its files' data in `bufs` (a read fills them).
    pub fn new(manifest: Manifest, bufs: Bufs) -> Self {
        Self {
            manifest,
            bufs,
            comment: [0; 0x40],
            results: Vec::new(),
            file_results: [-1; 9],
            mount: None,
            check: None,
            callbacks: Vec::new(),
            snapshots: Vec::new(),
            steps: Vec::new(),
            queued: Vec::new(),
            write_queued: None,
            free_bytes: 0,
            free_files: 0,
            layer: Layer::new(0),
            puts: Vec::new(),
            pull_after: None,
        }
    }

    /// The list's result: its last task's.
    pub fn result(&self) -> i32 {
        self.results.last().map_or(lb::START, |r| r.1)
    }

    /// Runs `tasks` on the card in the slot (None for an empty slot), as `executeNextTask`
    /// does, and returns the list's result.
    pub fn execute(
        &mut self,
        card: Option<&mut Card>,
        console: &Console,
        clock: &mut Clock,
        tasks: &[(Task, u32)],
    ) -> i32 {
        let mut result = lb::START;
        let mut saved = lb::START;
        let mut mounted: Option<Mounted> = None;
        let mut card = card;
        for (task, mask) in tasks {
            if mask & (1u32 << result) == 0 {
                break;
            }
            if mounted.is_some()
                && matches!(
                    task,
                    Task::Open { .. }
                        | Task::Create(_)
                        | Task::Read
                        | Task::Write
                        | Task::SetStatus
                        | Task::ReadHeader
                )
            {
                if matches!(task, Task::Write) {
                    self.write_queued = Some(self.steps.len());
                }
                self.steps.push(b't');
            }
            result = match task {
                Task::Mount => match card.take() {
                    None => {
                        self.mount = Some(CardError::NoCard.code());
                        lb::DEVICE_ERROR
                    }
                    Some(c) => {
                        let (m, r) = c.mount(console, clock);
                        mounted = Some(m);
                        self.mount = Some(r);
                        from_sdk(r)
                    }
                },
                Task::Check => match mounted.as_mut() {
                    None => saved,
                    Some(m) => {
                        let r = crate::card::code(m.check());
                        self.check = Some(r);
                        from_sdk(r)
                    }
                },
                Task::Open {
                    name,
                    with_manifest,
                } => match mounted.as_mut() {
                    None => lb::DEVICE_ERROR,
                    Some(m) => self.open(m, name.as_deref(), *with_manifest),
                },
                Task::Unk3 => match saved {
                    0 | 2 => lb::OPENED,
                    r => r,
                },
                Task::Format { ticks } => match mounted.as_mut() {
                    None => saved,
                    Some(m) => {
                        m.format(*ticks);
                        lb::READY
                    }
                },
                Task::Delete(name) => match mounted.as_mut() {
                    None => lb::DEVICE_ERROR,
                    Some(m) => from_sdk(crate::card::code(m.delete(name))),
                },
                Task::Rename { old, new } => match mounted.as_mut() {
                    None => lb::DEVICE_ERROR,
                    Some(m) => from_sdk(crate::card::code(m.rename(old, new))),
                },
                Task::Create(name) => match mounted.as_mut() {
                    None => lb::DEVICE_ERROR,
                    Some(m) => {
                        self.bufs.filename = name.clone();
                        self.layer.state.set_comment(&self.comment);
                        self.request(m, Request::Create, lb::READY)
                    }
                },
                Task::Read | Task::Write => match mounted.as_mut() {
                    None => lb::DEVICE_ERROR,
                    Some(m) => {
                        let mut saved_error = lb::READY;
                        for i in 0..9 {
                            if self.layer.state.file_size(i) == 0 {
                                continue;
                            }
                            if matches!(task, Task::Write) && self.layer.state.file_size(i) < 0 {
                                // hsd_803B2A4C refuses before queueing.
                                self.file_results[i] = -257;
                                self.queued.push((i, -257));
                                saved_error = from_hsd(-257);
                                continue;
                            }
                            self.queued.push((i, 0));
                            let req = if matches!(task, Task::Read) {
                                Request::ReadFile(i as i32)
                            } else {
                                Request::WriteFile(i as i32)
                            };
                            saved_error = self.request(m, req, saved_error);
                        }
                        saved_error
                    }
                },
                Task::SetStatus => match mounted.as_mut() {
                    None => lb::DEVICE_ERROR,
                    Some(m) => {
                        self.layer.state.set_comment(&self.comment);
                        self.request(m, Request::SetStatus, lb::READY)
                    }
                },
                Task::ReadHeader => match mounted.as_mut() {
                    None => lb::DEVICE_ERROR,
                    Some(m) => self.request(m, Request::ReadHeader, lb::READY),
                },
                Task::ListSnapshots => match mounted.as_mut() {
                    None => lb::DEVICE_ERROR,
                    Some(m) => {
                        self.list_snapshots(m);
                        lb::READY
                    }
                },
                Task::FindFile {
                    company,
                    game,
                    name,
                } => match mounted.as_mut() {
                    None => lb::DEVICE_ERROR,
                    Some(m) => {
                        let found = (0..crate::card::MAX_FILES as i32).any(|no| {
                            m.get_status(no).is_ok_and(|s| {
                                s.company() == company
                                    && s.game_name() == game
                                    && s.file_name() == &name[..]
                            })
                        });
                        if found { lb::READY } else { lb::INVALID }
                    }
                },
            };
            saved = result;
            self.results.push((task.clone(), result));
        }
        result
    }

    /// Runs one HSD request; each callback's error becomes the task's result
    /// (`fn_8001A0B0`).
    fn request(&mut self, m: &mut Mounted, req: Request, mut saved: i32) -> i32 {
        let made = self.steps.iter().filter(|&&c| is_transfer(c)).count();
        self.layer.budget = self.pull_after.map(|n| n.saturating_sub(made));
        let calls = self.layer.request(m, &mut self.bufs, req);
        let base = self.steps.len();
        self.puts.extend(
            self.layer
                .puts
                .drain(..)
                .map(|(at, p, bytes)| (base + at, p, bytes)),
        );
        self.steps.append(&mut self.layer.log);
        for (arg, r) in calls {
            self.steps.push(b'k');
            self.callbacks.push((arg, r));
            if (0..9).contains(&arg) {
                self.file_results[arg as usize] = r;
            }
            let e = from_hsd(r);
            if e != 0 {
                saved = e;
            }
        }
        saved
    }

    /// `taskOpen`.
    fn open(&mut self, m: &mut Mounted, name: Option<&[u8]>, with_manifest: bool) -> i32 {
        let (bytes, files) = match m.free_blocks() {
            Ok(f) => f,
            Err(e) => return from_sdk(e.code()),
        };
        self.free_bytes = bytes;
        self.free_files = files;
        let Some(name) = name else {
            return lb::NULL_FILENAME;
        };
        let opened = m.open(name);
        self.layer = Layer::new(0);
        if let Ok(no) = opened {
            return self.request(m, Request::Open(no), lb::READY);
        }
        if !with_manifest {
            return lb::NO_FILE;
        }
        if files == 0 {
            return lb::NO_ENTRY;
        }
        self.setup_entries();
        if bytes < (self.layer.state.sectors_needed() << 13) {
            lb::NO_SPACE
        } else {
            lb::NO_FILE
        }
    }

    /// `setupCardEntries`: the manifest's icons and files, each with data, registered.
    fn setup_entries(&mut self) {
        let s = &mut self.layer.state;
        s.set_icon_info(&self.manifest.icon);
        for (i, e) in self.manifest.files.iter().enumerate() {
            if e.size != 0 {
                s.set_file(i, e.size, e.flags, DATA + i as u32);
            }
        }
    }

    /// `taskListSnapshots`: this game's files named by a number, newest first.
    fn list_snapshots(&mut self, m: &mut Mounted) {
        let console = m.console().clone();
        let mut list: Vec<Snapshot> = Vec::new();
        for no in 0..crate::card::MAX_FILES as i32 {
            let Ok(s) = m.get_status(no) else { continue };
            let name = s.file_name();
            if s.company() != console.company
                || s.game_name() != console.game_name
                || !name.first().is_some_and(u8::is_ascii_digit)
            {
                continue;
            }
            let digits = name.iter().take_while(|c| c.is_ascii_digit()).count();
            let time = std::str::from_utf8(&name[..digits])
                .ok()
                .and_then(|t| t.parse::<u64>().ok())
                .map_or(u32::MAX, |t| t.min(u64::from(u32::MAX)) as u32);
            let node = Snapshot {
                time,
                file_no: no,
                blocks: (s.length() >> 13) as u16,
            };
            let at = list
                .iter()
                .position(|n| n.time <= time)
                .unwrap_or(list.len());
            list.insert(at, node);
        }
        self.snapshots = list;
    }
}

/// The address a registered file's data stands at: only whether it's null matters to the layer
/// (and `file_data[0]` of a file without data, 0, reaches the spare blocks' file table).
const DATA: u32 = 0x8100_0000;

/// The lists the game runs (`lbcardnew`'s builders), by what they do.
pub mod lists {
    use super::{Task, lb};

    fn mount_check() -> Vec<(Task, u32)> {
        vec![
            (Task::Mount, 1 << lb::START),
            (Task::Check, 1 << lb::READY | 1 << lb::MALFORMED),
        ]
    }

    fn open(name: &[u8], with_manifest: bool) -> Vec<(Task, u32)> {
        let mut v = mount_check();
        v.push((
            Task::Open {
                name: Some(name.to_vec()),
                with_manifest,
            },
            1 << lb::READY,
        ));
        v.push((Task::Unk3, u32::MAX));
        v
    }

    /// `lb_8001B7E0`: whether the file is there (and if not, whether one fits).
    pub fn probe(name: &[u8]) -> Vec<(Task, u32)> {
        open(name, true)
    }

    /// `lb_8001BD34`: loads the file.
    pub fn load(name: &[u8]) -> Vec<(Task, u32)> {
        let mut v = open(name, false);
        v.push((Task::Read, 1 << lb::READY | 1 << lb::OPENED));
        v
    }

    /// `lb_8001BE30`: saves to the file: its status, then every logical file.
    pub fn save(name: &[u8]) -> Vec<(Task, u32)> {
        let mut v = open(name, false);
        v.push((Task::SetStatus, 1 << lb::OPENED));
        v.push((Task::Write, 1 << lb::READY | 1 << lb::OPENED));
        v
    }

    /// `lb_8001BC18` and `lb_8001BB48`: creates the file when it isn't there.
    pub fn create(name: &[u8]) -> Vec<(Task, u32)> {
        let mut v = open(name, true);
        v.push((Task::Create(name.to_vec()), 1 << lb::NO_FILE));
        v
    }

    /// `lb_8001BF04`: a snapshot's header, then its data.
    pub fn read_with_header(name: &[u8]) -> Vec<(Task, u32)> {
        let mut v = open(name, false);
        v.push((Task::ReadHeader, 1 << lb::OPENED));
        v.push((Task::Read, 1 << lb::READY | 1 << lb::OPENED));
        v
    }

    /// `lb_8001BA44`: deletes the file.
    pub fn delete(name: &[u8]) -> Vec<(Task, u32)> {
        let mut v = open(name, false);
        v.push((
            Task::Delete(name.to_vec()),
            1 << lb::OPENED | 1 << lb::DAMAGED | 1 << lb::BAD_FILE,
        ));
        v
    }

    /// `lb_8001C0F4`: files `a` and `b` swap names through `c`: `a` opened and named `c`,
    /// `b` opened and named `a`, then `c` opened and named `b`.
    pub fn swap(a: &[u8], b: &[u8], c: &[u8]) -> Vec<(Task, u32)> {
        let renamed = 1 << lb::OPENED | 1 << lb::DAMAGED | 1 << lb::BAD_FILE;
        let mut v = open(a, false);
        for (opened, old, new) in [(a, a, c), (b, b, a), (c, c, b)] {
            if opened != a {
                v.push((
                    Task::Open {
                        name: Some(opened.to_vec()),
                        with_manifest: false,
                    },
                    1 << lb::READY,
                ));
                v.push((Task::Unk3, u32::MAX));
            }
            v.push((
                Task::Rename {
                    old: old.to_vec(),
                    new: new.to_vec(),
                },
                renamed,
            ));
        }
        v
    }

    /// `lb_8001B8C8`: formats the card.
    pub fn format(ticks: u64) -> Vec<(Task, u32)> {
        let mut v = mount_check();
        v.push((Task::Format { ticks }, u32::MAX));
        v
    }

    fn no_name() -> Vec<(Task, u32)> {
        let mut v = mount_check();
        v.push((
            Task::Open {
                name: None,
                with_manifest: false,
            },
            1 << lb::READY,
        ));
        v.push((Task::Unk3, u32::MAX));
        v
    }

    /// `lb_8001BFD8`: the snapshots and the free space.
    pub fn list_snapshots() -> Vec<(Task, u32)> {
        let mut v = no_name();
        v.push((Task::ListSnapshots, 1 << lb::NULL_FILENAME));
        v
    }

    /// `lb_8001C2D8`: whether another game's file is there.
    pub fn find_file(company: [u8; 2], game: [u8; 4], name: &[u8]) -> Vec<(Task, u32)> {
        let mut v = no_name();
        v.push((
            Task::FindFile {
                company,
                game,
                name: name.to_vec(),
            },
            1 << lb::NULL_FILENAME,
        ));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A snapshot's create on a formatted card, the card pulled out after `pull` of the HSD
    /// layer's transfers: the list's result, file 0's, and the card it leaves.
    fn snapshot_pulled(pull: Option<usize>) -> (i32, i32, Run, Card) {
        let console = Console::ssbm_rs();
        let mut card = Card::blank();
        card.format(&console, 0);
        let size = 0x38 + 248_093;
        let mut bufs = Bufs::default();
        bufs.files[0] = (0..size as usize + 0x2000)
            .map(|i| (i * 7 + i / 300) as u8)
            .collect();
        bufs.banner = vec![0x11; 0x1800];
        bufs.icons = vec![0x22; 0x600];
        let mut run = Run::new(crate::melee::snapshot_manifest(size), bufs);
        run.pull_after = pull;
        let r = run.execute(
            Some(&mut card),
            &console,
            &mut Clock::at(1000),
            &lists::create(b"1000"),
        );
        let file = from_hsd(run.file_results[0]);
        (r, file, run, card)
    }

    /// A card pulled out between a save's transfers: the transfers before stay on the card, the
    /// next write's open finds no card, and file 0 and the list end on it (15), as ssbm-rs's
    /// probe of slot B pulled mid-save logs (the create, then 11 of the 32 sectors).
    #[test]
    fn a_card_pulled_out_mid_save_keeps_the_transfers_before() {
        let (r, file, whole, full) = snapshot_pulled(None);
        assert_eq!((r, file), (lb::READY, lb::READY));
        // The create, the header's sector written and read back, the 32 sectors and the
        // status.
        assert_eq!(whole.steps.iter().filter(|&&c| is_transfer(c)).count(), 36);
        let (r, file, run, card) = snapshot_pulled(Some(12));
        assert_eq!((r, file), (lb::DEVICE_ERROR, lb::DEVICE_ERROR));
        assert_eq!(run.callbacks.last(), Some(&(0, CardError::NoCard.code())));
        assert_eq!(run.steps.iter().filter(|&&c| is_transfer(c)).count(), 12);
        let blank = {
            let mut c = Card::blank();
            c.format(&Console::ssbm_rs(), 0);
            c
        };
        let block = |c: &Card, b: usize| c.image()[b * 0x2000..(b + 1) * 0x2000].to_vec();
        // The file's blocks from 5: the first nine (its ten sectors' writes reach them) as the
        // whole save's, the rest as the formatted card left them.
        assert_eq!(run.steps.iter().filter(|&&c| c == b'w').count(), 10);
        assert!((5..14).all(|b| block(&card, b) == block(&full, b)));
        assert!((14..37).all(|b| block(&card, b) == block(&blank, b)));
        assert!((5..37).all(|b| block(&full, b) != block(&blank, b)));
        // Pulled before the create: nothing written.
        let (r, _, _, card) = snapshot_pulled(Some(0));
        assert_eq!(r, lb::DEVICE_ERROR);
        assert_eq!(card.image(), blank.image());
    }
}
