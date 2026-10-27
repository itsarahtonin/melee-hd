//! HAL's object scheduler (`sysdolphin/baselib/gobj*.c`): GObjs on process links in priority
//! order, each with processes that `run_procs` invokes every tick, and render links that the
//! renderer walks.
//!
//! The original's intrusive lists are kept with slot ids for pointers, because order is
//! behavior: where a process lands relative to the one running decides whether it runs this
//! tick, and code that removes or reprioritizes the running object is deferred until its
//! process returns. Processes, render callbacks, user data and objects are the game's plain
//! `Copy` tokens (`Kinds`), so the scheduler is data a snapshot can clone.

use std::fmt::Debug;

use super::pool::Pool;
use crate::hsd::heap::Kind;
use crate::hsd::pool::kind as pool_kind;

/// Process links, `p_link_max + 1` (the default 63, which Melee keeps).
pub const P_LINKS: usize = 64;
/// Render links, `gx_link_max + 1`; one more past them is the "max" link (`GObj_SetupGXLinkMax`).
pub const GX_LINKS: usize = 64;
/// Process priorities (s_links), `gproc_pri_max + 1`; Melee sets the maximum to 0x18.
pub const PROC_PRIORITIES: usize = 25;

/// What a process finds in f27 when `run_procs` invokes it: the boot's, since nothing from
/// `main` down the scene loop (`gm_801A4510`, `runGameMode`, `gm_801A4014`, `gm_801A4D34`) to
/// `HSD_GObj_RunProcs` writes it, the SDK doesn't clear the FPRs, and no Slippi or playback
/// code writes it. Saved calls of the ReDead's damage callback made with every unit in the
/// interpreter and with every port have 0.0 there (`ps0`; `ps1` holds whatever a callee's
/// single-precision use left, which `lfd` doesn't restore), as ssbm-rs's apploader boot leaves
/// it. A process that reads f27 before writing it (`it_802EA804` under the ReDead's) reads this.
pub const PROC_F27: f64 = 0.0;
/// What a process finds in f20, as in f27 ([`PROC_F27`]): the boot's 0.0, since nothing from
/// `main` down the scene loop to `HSD_GObj_RunProcs`, and from there through the fighters'
/// computer player frame (`Fighter_procCpu`, `ftCo_800B3900`, `ftCo_800B2AFC`,
/// `ftCo_800B24B8`, `ftCo_800AE7AC`), writes it. A function that saves f20 in its frame
/// (`ftCo_800A6FC4`) saves this.
pub const PROC_F20: f64 = 0.0;

/// What `gx_link` holds when a GObj has no render link.
pub const GX_LINK_NONE: u8 = 0xFF;

/// A GObj: a slot in the scheduler's pool, compared as the original compared addresses.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, melee_derive::StateEq)]
pub struct GObjId(pub u32);

/// A process (`HSD_GObjProc`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, melee_derive::StateEq)]
pub struct ProcId(pub u32);

/// The game's types for what GObjs carry: plain tokens, never closures.
pub trait Kinds: Clone + Debug + 'static {
    /// A process's function (`HSD_GObjEvent`).
    type Proc: Copy + Debug + PartialEq;
    /// A render callback (`GObj_RenderFunc`).
    type Render: Copy + Debug + PartialEq;
    /// User data (`user_data`): a fighter, an item, ….
    type UserData: Copy + Debug + PartialEq;
    /// The HSD object (`hsd_obj`): a joint tree, a camera, ….
    type Obj: Copy + Debug + PartialEq;

    /// The function `GObj_InitUserData` names to free user data (its GObj's word after the
    /// user data), where melee-sim knows it: the heap's image lays it out.
    fn user_data_remover(_: &Self::UserData) -> Option<u32> {
        None
    }
}

/// `HSD_GObj`.
#[derive(melee_derive::CloneFrom, Debug, melee_derive::StateEq)]
pub struct GObj<K: Kinds> {
    pub classifier: u16,
    pub p_link: u8,
    pub gx_link: u8,
    pub p_priority: u8,
    pub render_priority: u8,
    pub obj_kind: u8,
    pub user_data_kind: u8,
    pub next: Option<GObjId>,
    pub prev: Option<GObjId>,
    pub next_gx: Option<GObjId>,
    pub prev_gx: Option<GObjId>,
    /// The most recently queued process; older ones follow through `Proc::child`.
    pub proc: Option<ProcId>,
    pub render: Option<K::Render>,
    pub gxlink_prios: u64,
    pub obj: Option<K::Obj>,
    pub user_data: Option<K::UserData>,
}

/// `HSD_GObjProc`.
#[derive(melee_derive::CloneFrom, Debug, melee_derive::StateEq)]
pub struct Proc<K: Kinds> {
    /// The owner's next older process.
    pub child: Option<ProcId>,
    /// Neighbours in the run order of this priority.
    pub next: Option<ProcId>,
    pub prev: Option<ProcId>,
    pub s_link: u8,
    /// Paused (`HSD_GObj_80390C5C`).
    pub flags_1: bool,
    pub flags_2: bool,
    /// The tick phase this process last ran in, 0 to 2, or 3 before its first run.
    pub flags_3: u8,
    pub gobj: GObjId,
    pub on_invoke: K::Proc,
}

/// `HSD_GObj_DelayedProcInfo`: changes requested for the running process's GObj, applied when
/// the process returns.
#[derive(Clone, Copy, Debug, Default, melee_derive::StateEq)]
pub struct Delayed {
    pub in_delayed_proc: bool,
    pub remove_gobj: bool,
    pub remove_proc: bool,
    pub change_priority: bool,
    pub how: u32,
    pub p_link: u8,
    pub priority: u8,
    pub position: Option<GObjId>,
}

impl Delayed {
    fn any(&self) -> bool {
        self.in_delayed_proc || self.remove_gobj || self.remove_proc || self.change_priority
    }
}

/// Where `create` puts a GObj among others of its process link.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum Place {
    /// After the last with priority no higher, searching from the tail (`GObj_Create`).
    FromTail,
    /// Before the first with a higher priority, searching from the head.
    FromHead,
    /// Right after `position`.
    After(GObjId),
    /// Right before `position`.
    Before(GObjId),
}

impl Place {
    fn code(self) -> u32 {
        match self {
            Place::FromTail => 0,
            Place::FromHead => 1,
            Place::After(_) => 2,
            Place::Before(_) => 3,
        }
    }

    fn from_code(code: u32, position: Option<GObjId>) -> Place {
        match code {
            0 => Place::FromTail,
            1 => Place::FromHead,
            2 => Place::After(position.expect("a position to insert after")),
            _ => Place::Before(position.expect("a position to insert before")),
        }
    }
}

/// The scheduler's state. Its fields are public so a captured game's state can be rebuilt
/// (`tools/capture`); game code goes through the methods.
#[derive(melee_derive::CloneFrom, Debug, melee_derive::StateEq)]
pub struct World<K: Kinds> {
    pub gobjs: Pool<GObj<K>, { pool_kind(Kind::GObj) }>,
    pub procs: Pool<Proc<K>, { pool_kind(Kind::GObjProc) }>,
    pub plink_head: [Option<GObjId>; P_LINKS],
    pub plink_tail: [Option<GObjId>; P_LINKS],
    pub gx_head: [Option<GObjId>; GX_LINKS + 1],
    pub gx_tail: [Option<GObjId>; GX_LINKS + 1],
    pub proc_head: [Option<ProcId>; PROC_PRIORITIES],
    /// Per priority and process link, the last queued process of that link
    /// (`HSD_GObj_ProcList`).
    pub proc_last: [Option<ProcId>; PROC_PRIORITIES * P_LINKS],
    /// `HSD_GObj_804D783C`: 0, 1, 2, advancing every `run_procs`.
    pub phase: u8,
    pub current_proc: Option<ProcId>,
    pub current_s_link: usize,
    pub next_proc: Option<ProcId>,
    pub current_gobj: Option<GObjId>,
    pub delayed: Delayed,
    /// Process links whose processes don't run, by bit (the scene loop's pause mask that
    /// Melee points `HSD_GObjLibInitData.unk_2` at).
    pub skip_links: u64,
    /// The GObj whose render callback a camera's pass runs (`HSD_GObj_804D7814`), and the
    /// max-link GObj (a camera's) whose callback runs (`HSD_GObj_804D7818`).
    pub rendering: Option<GObjId>,
    pub rendering_camera: Option<GObjId>,
}

impl<K: Kinds> Default for World<K> {
    fn default() -> Self {
        World::new()
    }
}

/// What the game does for the scheduler: run processes and release what GObjs carry.
pub trait Host<K: Kinds> {
    fn world(&mut self) -> &mut World<K>;
    /// Runs a process's function for its GObj (`on_invoke`).
    fn invoke(&mut self, gobj: GObjId, proc: K::Proc);
    /// Releases user data as its kind's remove function does (`user_data_remove_func`).
    fn remove_user_data(&mut self, gobj: GObjId, kind: u8, data: K::UserData);
    /// Releases an object as its kind's function does (`HSD_GObj_804D7810`).
    fn remove_obj(&mut self, kind: u8, obj: K::Obj);
}

impl<K: Kinds> World<K> {
    /// An empty scheduler, as `HSD_GObjInit` leaves it.
    #[doc(alias = "HSD_GObjInit")]
    pub fn new() -> Self {
        World {
            gobjs: Pool::default(),
            procs: Pool::default(),
            plink_head: [None; P_LINKS],
            plink_tail: [None; P_LINKS],
            gx_head: [None; GX_LINKS + 1],
            gx_tail: [None; GX_LINKS + 1],
            proc_head: [None; PROC_PRIORITIES],
            proc_last: [None; PROC_PRIORITIES * P_LINKS],
            phase: 0,
            current_proc: None,
            current_s_link: 0,
            next_proc: None,
            current_gobj: None,
            delayed: Delayed::default(),
            skip_links: 0,
            rendering: None,
            rendering_camera: None,
        }
    }

    pub fn gobj(&self, g: GObjId) -> &GObj<K> {
        &self.gobjs[g.0]
    }

    pub fn gobj_mut(&mut self, g: GObjId) -> &mut GObj<K> {
        &mut self.gobjs[g.0]
    }

    pub fn proc(&self, p: ProcId) -> &Proc<K> {
        &self.procs[p.0]
    }

    pub fn proc_mut(&mut self, p: ProcId) -> &mut Proc<K> {
        &mut self.procs[p.0]
    }

    /// The first GObj of a process link (`HSD_GObjPLinkHead`).
    pub fn first_in(&self, p_link: u8) -> Option<GObjId> {
        self.plink_head[p_link as usize]
    }

    /// The last GObj of a process link (`plinklow_gobjs`).
    pub fn last_in(&self, p_link: u8) -> Option<GObjId> {
        self.plink_tail[p_link as usize]
    }

    /// The first GObj of a render link (`HSD_GObjGXLinkHead`).
    pub fn first_rendered(&self, gx_link: u8) -> Option<GObjId> {
        self.gx_head[gx_link as usize]
    }

    /// The first process of a priority (`HSD_GObj_GObjProcHead`).
    pub fn first_proc(&self, s_link: u8) -> Option<ProcId> {
        self.proc_head[s_link as usize]
    }

    /// The last queued process of a process link at a priority (`HSD_GObj_ProcList`).
    pub fn proc_last_for(&self, p_link: u8, s_link: u8) -> Option<ProcId> {
        self.proc_last[Self::proc_index(p_link, s_link)]
    }

    /// The GObj whose process is running (`HSD_GObj_CurrentInvokedProcGObj`).
    pub fn current_gobj(&self) -> Option<GObjId> {
        self.current_gobj
    }

    /// The running process (`HSD_GObj_CurrentInvokedProc`).
    pub fn current_proc(&self) -> Option<ProcId> {
        self.current_proc
    }

    /// Links a GObj after `hiprio`, or first, `GObj_PReorder`.
    #[doc(alias = "GObj_PReorder")]
    fn p_reorder(&mut self, g: GObjId, hiprio: Option<GObjId>) {
        let link = self.gobjs[g.0].p_link as usize;
        self.gobjs[g.0].prev = hiprio;
        if let Some(h) = hiprio {
            self.gobjs[g.0].next = self.gobjs[h.0].next;
            self.gobjs[h.0].next = Some(g);
        } else {
            self.gobjs[g.0].next = self.plink_head[link];
            self.plink_head[link] = Some(g);
        }
        match self.gobjs[g.0].next {
            Some(n) => self.gobjs[n.0].prev = Some(g),
            None => self.plink_tail[link] = Some(g),
        }
    }

    fn place(&mut self, g: GObjId, place: Place) {
        let link = self.gobjs[g.0].p_link as usize;
        let priority = self.gobjs[g.0].p_priority;
        match place {
            Place::FromTail => {
                let mut at = self.plink_tail[link];
                while let Some(a) = at.filter(|a| self.gobjs[a.0].p_priority > priority) {
                    at = self.gobjs[a.0].prev;
                }
                self.p_reorder(g, at);
            }
            Place::FromHead => {
                let mut at = self.plink_head[link];
                while let Some(a) = at.filter(|a| self.gobjs[a.0].p_priority < priority) {
                    at = self.gobjs[a.0].next;
                }
                let before = match at {
                    Some(a) => self.gobjs[a.0].prev,
                    None => self.plink_tail[link],
                };
                self.p_reorder(g, before);
            }
            Place::After(p) => self.p_reorder(g, Some(p)),
            Place::Before(p) => {
                let before = self.gobjs[p.0].prev;
                self.p_reorder(g, before);
            }
        }
    }

    fn p_unlink(&mut self, g: GObjId) {
        let (prev, next, link) = {
            let o = &self.gobjs[g.0];
            (o.prev, o.next, o.p_link as usize)
        };
        match prev {
            Some(p) => self.gobjs[p.0].next = next,
            None => self.plink_head[link] = next,
        }
        match next {
            Some(n) => self.gobjs[n.0].prev = prev,
            None => self.plink_tail[link] = prev,
        }
    }

    /// A new GObj on `p_link`, `CreateGObj`.
    #[doc(alias = "CreateGObj")]
    pub fn create_at(&mut self, place: Place, classifier: u16, p_link: u8, priority: u8) -> GObjId {
        assert!(
            (p_link as usize) < P_LINKS,
            "p_link {p_link} past the maximum"
        );
        // `GObj_Create` writes all but its last word, the remove function null.
        super::heap::with(|h| h.image.claim_next(&[(0, 0x30)]));
        let g = GObjId(self.gobjs.alloc(GObj {
            classifier,
            p_link,
            gx_link: GX_LINK_NONE,
            p_priority: priority,
            render_priority: 0,
            obj_kind: 0xFF,
            user_data_kind: 0xFF,
            next: None,
            prev: None,
            next_gx: None,
            prev_gx: None,
            proc: None,
            render: None,
            gxlink_prios: 0,
            obj: None,
            user_data: None,
        }));
        let at = self.gobjs.addr(g.0);
        super::heap::with(|h| h.image.word(at + 0x30, 0));
        self.place(g, place);
        g
    }

    /// A new GObj after the last of its priority, `GObj_Create`.
    #[doc(alias = "GObj_Create")]
    pub fn create(&mut self, classifier: u16, p_link: u8, priority: u8) -> GObjId {
        self.create_at(Place::FromTail, classifier, p_link, priority)
    }

    fn proc_index(p_link: u8, s_link: u8) -> usize {
        p_link as usize + s_link as usize * P_LINKS
    }

    /// Puts a process into its priority's run order, after the processes of GObjs before its
    /// own, `HSD_GObjProc_QueueProc`.
    #[doc(alias = "HSD_GObjProc_QueueProc")]
    fn queue_proc(&mut self, p: ProcId) {
        let g = self.procs[p.0].gobj;
        let s_link = self.procs[p.0].s_link;
        let mut p_link = self.gobjs[g.0].p_link;
        let index = Self::proc_index(p_link, s_link);
        let mut dst = None;
        if self.proc_last[index].is_some() {
            let mut cur = Some(g);
            'gobjs: while let Some(c) = cur {
                let mut d = self.gobjs[c.0].proc;
                while let Some(dp) = d {
                    if self.procs[dp.0].s_link == s_link {
                        if self.proc_last[index] == Some(dp) {
                            self.proc_last[index] = Some(p);
                        }
                        dst = Some(dp);
                        break 'gobjs;
                    }
                    d = self.procs[dp.0].child;
                }
                cur = self.gobjs[c.0].prev;
            }
        } else {
            self.proc_last[index] = Some(p);
        }
        if dst.is_none() {
            while p_link != 0 {
                p_link -= 1;
                if let Some(dp) = self.proc_last[Self::proc_index(p_link, s_link)] {
                    dst = Some(dp);
                    break;
                }
            }
        }
        match dst {
            Some(d) => {
                self.procs[p.0].next = self.procs[d.0].next;
                self.procs[d.0].next = Some(p);
                self.procs[p.0].prev = Some(d);
            }
            None => {
                self.procs[p.0].next = self.proc_head[s_link as usize];
                self.proc_head[s_link as usize] = Some(p);
                self.procs[p.0].prev = None;
            }
        }
        if let Some(n) = self.procs[p.0].next {
            self.procs[n.0].prev = Some(p);
        }
        self.procs[p.0].child = self.gobjs[g.0].proc;
        self.gobjs[g.0].proc = Some(p);
        let pr = &self.procs[p.0];
        if self.delayed.in_delayed_proc
            && pr.prev == self.current_proc
            && pr.next == self.next_proc
            && s_link as usize == self.current_s_link
        {
            self.next_proc = Some(p);
        }
    }

    /// Takes a process out of its priority's run order, `HSD_GObjProc_UnqueueProc`.
    #[doc(alias = "HSD_GObjProc_UnqueueProc")]
    fn unqueue_proc(&mut self, p: ProcId) {
        let (prev, next, s_link, g) = {
            let pr = &self.procs[p.0];
            (pr.prev, pr.next, pr.s_link, pr.gobj)
        };
        let p_link = self.gobjs[g.0].p_link;
        if self.delayed.in_delayed_proc && Some(p) == self.next_proc {
            self.next_proc = next;
        }
        let index = Self::proc_index(p_link, s_link);
        if self.proc_last[index] == Some(p) {
            self.proc_last[index] = match prev {
                Some(pv) if self.gobjs[self.procs[pv.0].gobj.0].p_link == p_link => Some(pv),
                _ => None,
            };
        }
        match prev {
            Some(pv) => self.procs[pv.0].next = next,
            None => self.proc_head[s_link as usize] = next,
        }
        if let Some(n) = next {
            self.procs[n.0].prev = prev;
        }
    }

    /// `HSD_GObjProc_UnlinkProcFromGObj`.
    #[doc(alias = "HSD_GObjProc_UnlinkProcFromGObj")]
    fn unlink_proc(&mut self, p: ProcId) {
        let g = self.procs[p.0].gobj;
        self.unqueue_proc(p);
        let child = self.procs[p.0].child;
        if self.gobjs[g.0].proc == Some(p) {
            self.gobjs[g.0].proc = child;
        } else {
            let mut cur = self.gobjs[g.0].proc.expect("the process's GObj lists it");
            while self.procs[cur.0].child != Some(p) {
                cur = self.procs[cur.0]
                    .child
                    .expect("the process's GObj lists it");
            }
            self.procs[cur.0].child = child;
        }
    }

    /// Adds a process to a GObj, `HSD_GObj_SetupProc`.
    #[doc(alias = "HSD_GObj_SetupProc")]
    pub fn setup_proc(&mut self, g: GObjId, on_invoke: K::Proc, priority: u8) -> ProcId {
        assert!(
            (priority as usize) < PROC_PRIORITIES,
            "process priority {priority} past the maximum"
        );
        // Its flags' bitfields leave the byte's low bits, and the padding after it.
        super::heap::with(|h| h.image.claim_next(&[(0, 0xD), (0x10, 0x18)]));
        let p = ProcId(self.procs.alloc(Proc {
            child: None,
            next: None,
            prev: None,
            s_link: priority,
            flags_1: false,
            flags_2: false,
            flags_3: 3,
            gobj: g,
            on_invoke,
        }));
        self.queue_proc(p);
        p
    }

    /// Removes a process, or, if it is the one running, marks it to go when it returns,
    /// `HSD_GObjProc_RemoveProc`.
    #[doc(alias = "HSD_GObjProc_RemoveProc")]
    pub fn remove_proc(&mut self, p: ProcId) {
        if !self.delayed.in_delayed_proc && Some(p) == self.current_proc {
            self.delayed.remove_proc = true;
        } else {
            self.unlink_proc(p);
            self.procs.free(p.0);
        }
    }

    /// `HSD_GObjProc_RemoveAllProcs`.
    #[doc(alias = "HSD_GObjProc_RemoveAllProcs")]
    pub fn remove_all_procs(&mut self, g: GObjId) {
        let mut cur = self.gobjs[g.0].proc;
        while let Some(p) = cur {
            cur = self.procs[p.0].child;
            self.remove_proc(p);
        }
    }

    /// Moves a GObj to another process link and priority, requeueing its processes, or
    /// defers that if its process is running, `HSD_GObjPLink_ChangeGObjPri_Unk`.
    #[doc(alias = "HSD_GObjPLink_ChangeGObjPri_Unk")]
    pub fn change_priority(&mut self, g: GObjId, place: Place, p_link: u8, priority: u8) {
        assert!(
            (p_link as usize) < P_LINKS,
            "p_link {p_link} past the maximum"
        );
        if !self.delayed.in_delayed_proc && Some(g) == self.current_gobj {
            self.delayed.change_priority = true;
            self.delayed.how = place.code();
            self.delayed.p_link = p_link;
            self.delayed.priority = priority;
            self.delayed.position = match place {
                Place::After(p) | Place::Before(p) => Some(p),
                _ => None,
            };
            return;
        }
        // Unqueue every process, reversing the chain.
        let mut cur = self.gobjs[g.0].proc;
        let mut reversed = None;
        while let Some(c) = cur {
            self.unqueue_proc(c);
            let child = self.procs[c.0].child;
            self.procs[c.0].child = reversed;
            reversed = Some(c);
            cur = child;
        }
        self.gobjs[g.0].proc = None;
        self.p_unlink(g);
        self.gobjs[g.0].p_link = p_link;
        self.gobjs[g.0].p_priority = priority;
        self.place(g, place);
        let flags_new = if self.phase == 0 { 2 } else { self.phase - 1 };
        let flags_cur = if flags_new == 0 { 2 } else { flags_new - 1 };
        let mut cur = reversed;
        while let Some(c) = cur {
            let child = self.procs[c.0].child;
            self.queue_proc(c);
            if self.procs[c.0].flags_3 == flags_cur {
                self.procs[c.0].flags_3 = flags_new;
            }
            cur = child;
        }
    }

    /// Links a GObj into its render link after `hiprio`, or first, `GObj_GXReorder`.
    #[doc(alias = "GObj_GXReorder")]
    fn gx_reorder(&mut self, g: GObjId, hiprio: Option<GObjId>) {
        let link = self.gobjs[g.0].gx_link as usize;
        self.gobjs[g.0].prev_gx = hiprio;
        if let Some(h) = hiprio {
            self.gobjs[g.0].next_gx = self.gobjs[h.0].next_gx;
            self.gobjs[h.0].next_gx = Some(g);
        } else {
            self.gobjs[g.0].next_gx = self.gx_head[link];
            self.gx_head[link] = Some(g);
        }
        match self.gobjs[g.0].next_gx {
            Some(n) => self.gobjs[n.0].prev_gx = Some(g),
            None => self.gx_tail[link] = Some(g),
        }
    }

    fn gx_insert(&mut self, g: GObjId, place: Place) {
        let link = self.gobjs[g.0].gx_link as usize;
        let priority = self.gobjs[g.0].render_priority;
        match place {
            Place::FromTail => {
                let mut at = self.gx_tail[link];
                while let Some(a) = at.filter(|a| self.gobjs[a.0].render_priority > priority) {
                    at = self.gobjs[a.0].prev_gx;
                }
                self.gx_reorder(g, at);
            }
            Place::FromHead => {
                let mut at = self.gx_head[link];
                while let Some(a) = at.filter(|a| self.gobjs[a.0].render_priority < priority) {
                    at = self.gobjs[a.0].next_gx;
                }
                let before = match at {
                    Some(a) => self.gobjs[a.0].prev_gx,
                    None => self.gx_tail[link],
                };
                self.gx_reorder(g, before);
            }
            Place::Before(p) => {
                let before = self.gobjs[p.0].prev_gx;
                self.gx_reorder(g, before);
            }
            Place::After(_) => unreachable!("no render insertion goes after a position"),
        }
    }

    /// Gives a GObj a render callback on `gx_link`, `GObj_SetupGXLink`.
    #[doc(alias = "GObj_SetupGXLink")]
    pub fn setup_gx_link(&mut self, g: GObjId, render: K::Render, gx_link: u8, priority: u8) {
        assert!(
            (gx_link as usize) < GX_LINKS,
            "gx_link {gx_link} past the maximum"
        );
        let o = &mut self.gobjs[g.0];
        o.render = Some(render);
        o.gx_link = gx_link;
        o.render_priority = priority;
        self.gx_insert(g, Place::FromTail);
    }

    /// The same on the link past the last, which the scene loop renders separately,
    /// `GObj_SetupGXLinkMax`; `sorted` inserts from the head (`GObj_SetupGXLinkMaxSorted`).
    #[doc(alias = "GObj_SetupGXLinkMax")]
    #[doc(alias = "GObj_SetupGXLinkMaxSorted")]
    pub fn setup_gx_link_max(&mut self, g: GObjId, render: K::Render, priority: u8, sorted: bool) {
        let o = &mut self.gobjs[g.0];
        o.render = Some(render);
        o.gx_link = GX_LINKS as u8;
        o.render_priority = priority;
        self.gx_insert(
            g,
            if sorted {
                Place::FromHead
            } else {
                Place::FromTail
            },
        );
    }

    /// Takes a GObj off its render link, `HSD_GObjGXLink_8039084C`.
    #[doc(alias = "HSD_GObjGXLink_8039084C")]
    pub fn gx_unlink(&mut self, g: GObjId) {
        let (prev, next, link) = {
            let o = &self.gobjs[g.0];
            assert!(o.gx_link != GX_LINK_NONE, "GObj has no render link");
            (o.prev_gx, o.next_gx, o.gx_link as usize)
        };
        match prev {
            Some(p) => self.gobjs[p.0].next_gx = next,
            None => self.gx_head[link] = next,
        }
        match next {
            Some(n) => self.gobjs[n.0].prev_gx = prev,
            None => self.gx_tail[link] = prev,
        }
        let o = &mut self.gobjs[g.0];
        o.gx_link = GX_LINK_NONE;
        o.render_priority = 0;
        o.prev_gx = None;
        o.next_gx = None;
    }

    /// Moves a GObj to another render link and priority, `HSD_GObjGXLink_80390908`.
    #[doc(alias = "HSD_GObjGXLink_80390908")]
    pub fn gx_relink(&mut self, g: GObjId, gx_link: u8, priority: u8) {
        assert!(
            (gx_link as usize) < GX_LINKS,
            "gx_link {gx_link} past the maximum"
        );
        self.gx_unlink(g);
        self.gobjs[g.0].gx_link = gx_link;
        self.gobjs[g.0].render_priority = priority;
        self.gx_insert(g, Place::FromHead);
    }

    /// Moves a GObj to render just before `other`, `HSD_GObjGXLink_803909D8`.
    #[doc(alias = "HSD_GObjGXLink_803909D8")]
    pub fn gx_relink_before(&mut self, g: GObjId, other: GObjId) {
        let (link, priority) = (
            self.gobjs[other.0].gx_link,
            self.gobjs[other.0].render_priority,
        );
        self.gx_unlink(g);
        self.gobjs[g.0].gx_link = link;
        self.gobjs[g.0].render_priority = priority;
        self.gx_insert(g, Place::Before(other));
    }

    /// Pauses or resumes a GObj's processes (`flags_1`), `HSD_GObj_80390C5C`/`80390C84`.
    #[doc(alias = "HSD_GObj_80390C5C")]
    #[doc(alias = "HSD_GObj_80390C84")]
    pub fn set_paused(&mut self, g: GObjId, paused: bool) {
        let mut cur = self.gobjs[g.0].proc;
        while let Some(p) = cur {
            self.procs[p.0].flags_1 = paused;
            cur = self.procs[p.0].child;
        }
    }

    /// Clears `flags_2` on a GObj's processes, `HSD_GObj_80390CAC`.
    #[doc(alias = "HSD_GObj_80390CAC")]
    pub fn clear_flags_2(&mut self, g: GObjId) {
        let mut cur = self.gobjs[g.0].proc;
        while let Some(p) = cur {
            self.procs[p.0].flags_2 = false;
            cur = self.procs[p.0].child;
        }
    }

    /// Marks a GObj's processes as already run this tick, `HSD_GObj_80390CD4`.
    #[doc(alias = "HSD_GObj_80390CD4")]
    pub fn mark_ran(&mut self, g: GObjId) {
        let mut cur = self.gobjs[g.0].proc;
        while let Some(p) = cur {
            self.procs[p.0].flags_3 = self.phase;
            cur = self.procs[p.0].child;
        }
    }

    /// Attaches user data, `GObj_InitUserData`.
    #[doc(alias = "GObj_InitUserData")]
    pub fn init_user_data(&mut self, g: GObjId, kind: u8, data: K::UserData) {
        let o = &mut self.gobjs[g.0];
        assert!(o.user_data_kind == 0xFF, "GObj already has user data");
        o.user_data_kind = kind;
        o.user_data = Some(data);
        // Its remove function, which melee-sim's user data names only by its kind: the
        // image knows the ones `Kinds::user_data_remover` gives.
        let at = self.gobjs.addr(g.0);
        let remover = K::user_data_remover(&data);
        super::heap::with(|h| match remover {
            Some(r) => h.image.word(at + 0x30, r),
            None => h
                .image
                .claim(at + 0x30, at + 0x34, "a GObj's user data remover"),
        });
    }

    fn take_user_data(&mut self, g: GObjId) -> Option<(u8, K::UserData)> {
        let o = &mut self.gobjs[g.0];
        if o.user_data_kind == 0xFF {
            return None;
        }
        let kind = std::mem::replace(&mut o.user_data_kind, 0xFF);
        Some((kind, o.user_data.take().expect("user data of a kind")))
    }

    /// Attaches an HSD object, `HSD_GObjObject_80390A70`.
    #[doc(alias = "HSD_GObjObject_80390A70")]
    pub fn set_obj(&mut self, g: GObjId, kind: u8, obj: K::Obj) {
        let o = &mut self.gobjs[g.0];
        assert!(o.obj_kind == 0xFF, "GObj already has an object");
        o.obj_kind = kind;
        o.obj = Some(obj);
    }

    /// Detaches the HSD object without releasing it, `HSD_GObjObject_80390ADC`.
    #[doc(alias = "HSD_GObjObject_80390ADC")]
    pub fn take_obj(&mut self, g: GObjId) -> Option<(u8, K::Obj)> {
        let o = &mut self.gobjs[g.0];
        if o.obj_kind == 0xFF {
            return None;
        }
        let kind = std::mem::replace(&mut o.obj_kind, 0xFF);
        Some((kind, o.obj.take().expect("an object of a kind")))
    }

    /// The first GObj on `p_link` with this classifier, `HSD_GObjObject_80390A3C`.
    #[doc(alias = "HSD_GObjObject_80390A3C")]
    pub fn find(&self, classifier: u16, p_link: u8) -> Option<GObjId> {
        let mut cur = self.plink_head[p_link as usize];
        while let Some(c) = cur {
            if self.gobjs[c.0].classifier == classifier {
                break;
            }
            cur = self.gobjs[c.0].next;
        }
        cur
    }
}

/// Releases a GObj's user data through the game and leaves it without, `GObj_RemoveUserData`.
#[doc(alias = "GObj_RemoveUserData")]
pub fn remove_user_data<K: Kinds, H: Host<K>>(host: &mut H, g: GObjId) {
    if let Some((kind, data)) = host.world().take_user_data(g) {
        host.remove_user_data(g, kind, data);
    }
}

/// Destroys a GObj: its user data and object through the game, then its processes and links,
/// `HSD_GObjFree`. A GObj whose process is running goes when the process returns.
#[doc(alias = "HSD_GObjFree")]
#[doc(alias = "GObj_RemoveUserData")]
#[doc(alias = "HSD_GObjObject_80390B0C")]
pub fn free<K: Kinds, H: Host<K>>(host: &mut H, g: GObjId) {
    let w = host.world();
    if !w.delayed.in_delayed_proc && Some(g) == w.current_gobj {
        w.delayed.remove_gobj = true;
        return;
    }
    if let Some((kind, data)) = w.take_user_data(g) {
        host.remove_user_data(g, kind, data);
    }
    if let Some((kind, obj)) = host.world().take_obj(g) {
        host.remove_obj(kind, obj);
    }
    let w = host.world();
    w.remove_all_procs(g);
    if w.gobjs[g.0].gx_link != GX_LINK_NONE {
        w.gx_unlink(g);
    }
    w.p_unlink(g);
    w.gobjs.free(g.0);
}

/// Runs every process once, by priority and then run order, applying deferred changes after
/// each, `HSD_GObj_RunProcs`.
#[doc(alias = "HSD_GObj_RunProcs")]
pub fn run_procs<K: Kinds, H: Host<K>>(host: &mut H) {
    let w = host.world();
    let skip = w.skip_links;
    w.phase = if w.phase >= 2 { 0 } else { w.phase + 1 };
    for s_link in 0..PROC_PRIORITIES {
        let w = host.world();
        w.current_s_link = s_link;
        let mut cur = w.proc_head[s_link];
        while let Some(p) = cur {
            let w = host.world();
            w.next_proc = w.procs[p.0].next;
            if w.procs[p.0].flags_3 != w.phase {
                w.procs[p.0].flags_3 = w.phase;
                let g = w.procs[p.0].gobj;
                let pr = &w.procs[p.0];
                if skip & (1u64 << w.gobjs[g.0].p_link) == 0 && !pr.flags_1 && !pr.flags_2 {
                    let f = pr.on_invoke;
                    w.current_gobj = Some(g);
                    w.current_proc = Some(p);
                    host.invoke(g, f);
                    let w = host.world();
                    w.next_proc = w.procs[p.0].next;
                    if w.delayed.any() {
                        w.delayed.in_delayed_proc = true;
                        let owner = w.procs[p.0].gobj;
                        if w.delayed.remove_gobj {
                            free(host, owner);
                        } else {
                            let d = w.delayed;
                            if d.change_priority {
                                let place = Place::from_code(d.how, d.position);
                                w.change_priority(owner, place, d.p_link, d.priority);
                            }
                            if d.remove_proc {
                                w.remove_proc(p);
                            }
                        }
                        host.world().delayed = Delayed::default();
                    }
                    let w = host.world();
                    w.current_gobj = None;
                    w.current_proc = None;
                }
            }
            cur = host.world().next_proc;
        }
    }
}
