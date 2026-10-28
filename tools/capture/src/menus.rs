//! The boot's and the menus' statics, compared with memory: the game's data and the power-on
//! state, the modes' machine, the boot mode's and the memory card scene's statics, the card's
//! state as the game keeps it, the preload cache with its heaps and their blocks, and what the
//! modes' `on_init`s set elsewhere. Only a session from power-on models them.

use melee_sim::game::mirror::{Cx, Mem, Mirror};
use melee_sim::game::structs::{PlayerInitData, VsModeData};
use melee_sim::lb::dvd::{PreloadEntry, SceneCache};
use melee_sim::lb::memory;

use crate::MEM1;
use crate::compare::Comparer;
use crate::import::sym;

/// Statics whose names other units' share, by address.
const STATE_MACHINE: u32 = 0x8047_9D30;
const LOAD_DATA: u32 = 0x804D_6940;
const LEAVE_DATA: u32 = 0x804D_6948;
const MEMCARD_ENTER: u32 = 0x8048_0DA8;
const CARD_GAME: u32 = 0x8043_3318;
const CARD_NEW: u32 = 0x8043_2A68;
const SNAP: u32 = 0x8043_3380;
const MANIFEST: u32 = 0x803B_AB74;
const OPENING_STATE: u32 = 0x8049_E548;
const TRAINING: u32 = 0x8047_3814;
const HOMERUN: u32 = 0x8049_7618;
const TOY_X194: u32 = 0x804A_284C;
const LBL_804D3878: u32 = 0x804D_3878;
const GAME_DATA: u32 = 0x8045_A6C0;
const MAIN_LIB: u32 = 0x8046_B0F0;
const PRELOAD_CACHE: u32 = 0x8043_2078;
const LB_HEAP: u32 = 0x8043_1FA0;

impl<'a> Comparer<'a> {
    fn mem(&self) -> Mem<'a> {
        Mem {
            bytes: self.mem,
            base: MEM1,
        }
    }

    /// Everything a session's boot and its scenes keep outside the match's state.
    pub fn menus(&mut self) {
        // The game's data and the power-on state.
        let m = self.mem();
        let game = self.game;
        game.game_data.compare(&m, GAME_DATA, "game data", self);
        let ml = &game.main_lib;
        self.check(
            "gmMainLib_8046B0F0",
            "power-on state",
            (
                u32::from(ml.skip_intro),
                u32::from(ml.resetting),
                u32::from(ml.progressive),
                u32::from(ml.xc),
                ml.x10,
                0,
            ),
            (
                self.u32(MAIN_LIB),
                self.u32(MAIN_LIB + 4),
                self.u32(MAIN_LIB + 8),
                self.u32(MAIN_LIB + 0xC),
                self.u32(MAIN_LIB + 0x10),
                self.u32(MAIN_LIB + 0x14),
            ),
        );
        // The modes' machine.
        let sm = &game.machine;
        let routing = |r: &melee_sim::game::gm::machine::Routing| {
            [
                r.curr_mode,
                r.pending_mode,
                r.prev_mode,
                r.curr_state_id,
                r.prev_state_id,
                r.next_state_id,
            ]
        };
        let theirs: Vec<u8> = (0..12).map(|i| self.u8(STATE_MACHINE + i)).collect();
        let ours = [routing(&sm.routing), routing(&sm.backup_routing)].concat();
        self.check("state_machine", "routing", ours, theirs);
        self.check(
            "state_machine",
            "pending change and override",
            (
                u8::from(sm.pending_mode_change),
                sm.get_override.map_or(0, |c| c.0),
            ),
            (self.u8(STATE_MACHINE + 0xC), self.u32(STATE_MACHINE + 0x10)),
        );
        let info = sm.scene_info.map_or(0, |(mode, state)| {
            let states = match mode {
                melee_sim::game::gm::machine::GM_BOOT => sym("gm_Mode_Boot_States"),
                melee_sim::game::gm::machine::GM_MEMCARD => sym("gm_Mode_MemCard_States"),
                melee_sim::game::gm::machine::GM_OPENING_MV => sym("gm_Mode_Opening_States"),
                melee_sim::game::gm::machine::GM_TITLE => sym("gm_Mode_Title_States"),
                melee_sim::game::gm::machine::GM_PROGRESSIVE_SCAN => sym("gm_Mode_ProgScan_States"),
                melee_sim::game::gm::machine::GM_MENU => sym("gm_Mode_Menu_States"),
                melee_sim::game::gm::machine::GM_VS => sym("gm_Mode_Vs_States"),
                melee_sim::game::gm::machine::GM_CAMERA_MODE => sym("gm_Mode_Camera_States"),
                melee_sim::game::gm::tournament::GM_TOURNAMENT => sym("gm_Mode_Tournament_States"),
                0x10 => sym("gm_Mode_SuperSuddenDeath_States"),
                0x11 => sym("gm_Mode_InvisibleVs_States"),
                0x12 => sym("gm_Mode_SlowMo_States"),
                0x13 => sym("gm_Mode_LightningVs_States"),
                0x1D => sym("gm_Mode_TinyVs_States"),
                0x1E => sym("gm_Mode_GiantVs_States"),
                0x1F => sym("gm_Mode_StaminaVs_States"),
                0x2A => sym("gm_Mode_CameraVs_States"),
                0x2C => sym("gm_Mode_SingleButtonVs_States"),
                melee_sim::game::ty::GM_TOY_GALLERY => sym("gm_Mode_ToyGallery_States"),
                melee_sim::game::ty::GM_TOY_LOTTERY => sym("gm_Mode_ToyLottery_States"),
                melee_sim::game::ty::GM_TOY_COLLECTION => sym("gm_Mode_ToyCollection_States"),
                melee_sim::game::gm::machine::GM_CHALLENGER_APPROACH => {
                    sym("gm_Mode_ChallengerApproach_States")
                }
                m => panic!("game mode {m:#x}'s states"),
            };
            states + 0x18 * u32::from(state) + 0xC
        });
        self.check(
            "gm_804D6720",
            "scene info",
            info,
            self.u32(sym("gm_804D6720")),
        );
        self.check(
            "gm_GetCurrentGameMode",
            "game mode",
            game.game_mode,
            sm.routing.curr_mode,
        );
        // The boot mode's.
        let b = &game.boot_mode;
        self.check(
            "load_data",
            "x0, x4, mode",
            (b.load_x0, b.load_x4, b.load_mode),
            (
                self.u32(LOAD_DATA),
                self.u8(LOAD_DATA + 4),
                self.u8(LOAD_DATA + 5),
            ),
        );
        self.check(
            "leave_data",
            "x0, mode",
            (b.leave_x0, b.leave_mode),
            (self.u32(LEAVE_DATA), self.u8(LEAVE_DATA + 4)),
        );
        self.check(
            "gm_8049E548",
            "opening state",
            b.opening_state,
            self.u8(OPENING_STATE + 0xE),
        );
        self.memcard_scene();
        self.card_state();
        self.preload_cache();
        // What the modes' `on_init`s set outside the game's data.
        let m = self.mem();
        for (i, p) in game.modes.training_players.iter().enumerate() {
            p.compare(
                &m,
                TRAINING + 0x14 + PlayerInitData::SIZE * i as u32,
                &format!("gm_80473814.saved_players[{i}]"),
                self,
            );
        }
        game.modes
            .homerun
            .compare(&m, HOMERUN, "gmHomeRun_VsModeData", self);
        let _ = VsModeData::SIZE;
        let toy: Vec<u8> = (0..0x25A).map(|i| self.u8(TOY_X194 + i)).collect();
        let ours = game.toys.table.bytes()[..0x25A].to_vec();
        if toy != ours {
            let at = (0..0x25A).find(|&i| toy[i] != ours[i]).unwrap();
            self.diff("Toy_804A284C", &format!("byte {at:#x}"), ours[at], toy[at]);
        }
        self.check(
            "lbl_804D3878",
            "the bank files' language",
            game.audio.lb.lang,
            self.u32(LBL_804D3878) as i32,
        );
        self.check(
            "HSD_SObjLib_804D7960",
            "the sprites' object kind",
            melee_sim::hsd::sobj::OBJ_SOBJ,
            self.u8(sym("HSD_SObjLib_804D7960")),
        );
        self.opening_title();
        self.main_menu();
        self.gallery();
        self.rules_menus();
        self.options();
        self.toys();
        self.data_menus();
        self.sound_test();
        self.attract();
        self.select_screens();
        self.camera_mode();
        self.stamina_mode();
        self.tournament();
    }

    /// The memory card scene's statics and its windows.
    fn memcard_scene(&mut self) {
        let mc = &self.game.memcard;
        let e = MEMCARD_ENTER;
        self.check(
            "memcard enter_data",
            "question, channel, mode",
            (mc.question, mc.chan, mc.mode_id),
            (self.u32(e) as i32, self.u8(e + 4), self.u8(e + 5)),
        );
        self.check(
            "memcard enter_data",
            "answer and its mode",
            (mc.answer, mc.exit_mode),
            (self.u32(e + 8), self.u8(e + 0xC)),
        );
        self.check(
            "memcard enter_data",
            "buttons, decision, card in, cursor",
            (mc.has_buttons, mc.decision, mc.card_was_in, mc.cursor),
            (
                self.u32(e + 0x10) as i32,
                self.u32(e + 0x14) as i32,
                self.u32(e + 0x18),
                self.u8(e + 0x1C),
            ),
        );
        self.check(
            "gm_804D6870",
            "debug viewer",
            (mc.debug_viewer, mc.debug_msg),
            (self.u8(sym("gm_804D6870")), self.u16(sym("gm_804D6872"))),
        );
        // The windows' list head (`gm_803DD4D0`), then each window and its buttons.
        let head = sym("gm_803DD4D0");
        self.check(
            "gm_803DD4D0",
            "kind",
            mc.head_kind,
            self.u32(head + 0xC) as i32,
        );
        let mut prev = head;
        let mut at = self.u32(head);
        for (i, w) in mc.windows.iter().enumerate() {
            let p = format!("memcard window {i}");
            if at != w.addr {
                self.diff(&p, "address", format!("{:#x}", w.addr), format!("{at:#x}"));
                return;
            }
            self.check(&p, "back link", prev, self.u32(at + 4));
            self.check(
                &p,
                "kind, message, place",
                (w.kind, w.msg, w.x.to_bits(), w.y.to_bits()),
                (
                    self.u32(at + 0xC) as i32,
                    self.u32(at + 0x10) as i32,
                    self.u32(at + 0x14),
                    self.u32(at + 0x18),
                ),
            );
            self.compare_handle(
                &format!("{p} model"),
                "GObjId",
                w.gobj.map(|g| g.0),
                self.u32(at + 0x1C),
            );
            self.compare_handle(
                &format!("{p} text"),
                "TextId",
                w.text.map(|t| t.0),
                self.u32(at + 0x20),
            );
            let mut bat = self.u32(at + 8);
            let mut bprev = 0;
            for (j, bt) in w.buttons.iter().enumerate() {
                let bp = format!("{p} button {j}");
                if bat != bt.addr {
                    self.diff(
                        &bp,
                        "address",
                        format!("{:#x}", bt.addr),
                        format!("{bat:#x}"),
                    );
                    return;
                }
                self.check(&bp, "back link", bprev, self.u32(bat + 4));
                self.check(
                    &bp,
                    "id, message, place",
                    (bt.id, bt.msg, bt.x.to_bits(), bt.y.to_bits()),
                    (
                        self.u32(bat + 8) as i32,
                        self.u32(bat + 0xC) as i32,
                        self.u32(bat + 0x10),
                        self.u32(bat + 0x14),
                    ),
                );
                self.compare_handle(
                    &format!("{bp} model"),
                    "GObjId",
                    bt.gobj.map(|g| g.0),
                    self.u32(bat + 0x20),
                );
                self.compare_handle(
                    &format!("{bp} text"),
                    "TextId",
                    bt.text.map(|t| t.0),
                    self.u32(bat + 0x24),
                );
                bprev = bat;
                bat = self.u32(bat);
            }
            self.check(&p, "buttons' end", 0, bat);
            prev = at;
            at = self.u32(at);
        }
        self.check("memcard windows", "list's end", 0, at);
        // The windows' scene, while the memory card scene runs: past it, the pointer names
        // the scene's freed heap, which nothing reads until the windows' setup
        // (`gm_801AEBB0`) sets it again (docs/engine.md, "Uninitialized values").
        use melee_sim::game::gm::machine::{GM_BOOT, GM_MEMCARD};
        if matches!(self.game.machine.scene_info, Some((m, _)) if m == GM_BOOT || m == GM_MEMCARD) {
            let scene = mc.scene;
            let theirs = self.u32(sym("gm_804D6868"));
            self.compare_data("gm_804D6868", scene, theirs);
        }
        self.check(
            "gm_804D686C",
            "canvas",
            mc.canvas,
            self.u32(sym("gm_804D686C")) as i32,
        );
    }

    /// The card as the game keeps it: `lbcardgame`'s state and the save's manifest,
    /// `lbcardnew`'s work areas and file results, and the snapshots' probes.
    fn card_state(&mut self) {
        let c = &self.game.card_game;
        let s = CARD_GAME;
        self.check(
            "lbcardgame state",
            "probe, swap, status, dirty, writer, failed, enable",
            (
                u32::from(c.probe_status),
                u32::from(c.unk_status),
                c.card_status,
                u32::from(c.dirty),
                c.x10,
                u32::from(c.failed),
                u32::from(c.enable),
            ),
            (
                self.u32(s),
                self.u32(s + 4),
                self.u32(s + 8) as i32,
                self.u32(s + 0xC),
                self.u32(s + 0x10) as i32,
                self.u32(s + 0x14),
                self.u32(s + 0x18),
            ),
        );
        let comment: Vec<u8> = (0..0x40).map(|i| self.u8(s + 0x1C + i)).collect();
        self.check("lbcardgame state", "comment", c.comment.to_vec(), comment);
        let (icons, scene) = (c.icon_data, c.scene_data);
        let theirs = self.u32(s + 0x5C);
        self.compare_data("lbcardgame state icon_data", icons, theirs);
        let theirs = self.u32(s + 0x64);
        self.compare_data("lbcardgame state scene_data", scene, theirs);
        self.check(
            "lbcardgame state",
            "indicator's place",
            c.jobj_translate_idx,
            self.u32(s + 0x60) as i32,
        );
        // The manifest's files, pointed at the card data once at boot.
        let set = c.manifest_set;
        for i in 1..9u32 {
            let ours = if !set {
                0
            } else if i == 1 {
                GAME_DATA + 0x1868
            } else {
                GAME_DATA + 0x1868 + 0x1790 + 0x1F2C * (i - 2)
            };
            self.check(
                "lbcardgame manifest",
                &format!("file {i}'s data"),
                ours,
                self.u32(MANIFEST + 0xC * i + 8),
            );
        }
        let t = self.game.card_tasks;
        self.check(
            "lbcardnew state",
            "work areas",
            (t.work_area, t.lib_area),
            (self.u32(CARD_NEW), self.u32(CARD_NEW + 4)),
        );
        let theirs: Vec<i32> = (0..9)
            .map(|i| self.u32(CARD_NEW + 0x38 + 8 * i) as i32)
            .collect();
        self.check(
            "lbcardnew state",
            "files' results (lb_8001B6E0)",
            t.file_errors.to_vec(),
            theirs,
        );
        let sn = self.game.snap;
        let addr = |p: Option<u32>| p.unwrap_or(0);
        self.check(
            "lbsnap state",
            "snapshot, slot, probes, changes",
            (
                addr(sn.snap),
                addr(sn.slot),
                sn.card_state,
                sn.state_changed,
            ),
            (
                self.u32(SNAP),
                self.u32(SNAP + 0x48),
                [self.u32(SNAP + 0x4C) as i32, self.u32(SNAP + 0x50) as i32],
                [
                    self.u32(SNAP + 0x54) as i32,
                    self.u32(SNAP + 0x58) as i32,
                    self.u32(SNAP + 0x5C) as i32,
                ],
            ),
        );
        let theirs = self.u32(SNAP + 0x44);
        self.compare_data("lbsnap state icon_data", sn.icon_data, theirs);
        if let Some(slot) = sn.slot {
            let theirs = [self.u32(slot) as i32, self.u32(slot + 0x408) as i32];
            let ours = [sn.slots[0].card_result, sn.slots[1].card_result];
            self.check("lbsnap slots", "card results", ours, theirs);
            // What a listing wrote (`lbSnap_8001D40C`): the rest holds the block's bytes.
            for (i, l) in sn.slots.iter().enumerate() {
                let at = slot + 0x408 * i as u32;
                let field = |s: &mut Self, name: &str, ours: Option<i32>, off: u32| {
                    if let Some(v) = ours {
                        let theirs = s.u32(at + off) as i32;
                        s.check("lbsnap slots", &format!("slot {i} {name}"), v, theirs);
                    }
                };
                field(self, "num", l.num, 4);
                field(self, "free blocks", l.free_blocks, 8);
                field(self, "free files", l.free_files, 0xC);
                for (k, e) in l.entries.iter().enumerate() {
                    let e_at = at + 0x10 + 8 * k as u32;
                    let name = format!("slot {i} entry {k}");
                    if let Some(t) = e.time {
                        let theirs = self.u32(e_at);
                        self.check("lbsnap slots", &format!("{name} time"), t, theirs);
                    }
                    if let Some(f) = e.file_no {
                        let theirs = self.u16(e_at + 4) as i16;
                        self.check("lbsnap slots", &format!("{name} file"), f, theirs);
                    }
                    if let Some(b) = e.blocks {
                        let theirs = self.u16(e_at + 6);
                        self.check("lbsnap slots", &format!("{name} blocks"), b, theirs);
                    }
                }
            }
        }
    }

    /// The preload cache, its heaps (`lbHeap_80431FA0`) and their blocks
    /// (`lbMemory_804318B0`).
    fn preload_cache(&mut self) {
        let Some(d) = self.game.audio.dvd.as_ref() else {
            return;
        };
        let entry = |e: Option<u8>| e.map_or(0, memory::entry_addr);
        let handle = |h: Option<u8>| h.map_or(0, memory::handle_addr);
        // `lbMemory_804318B0`.
        let mm = &d.memory;
        let b = memory::BASE;
        self.check(
            "lbMemory",
            "ARAM range",
            (mm.aram_lo, mm.aram_hi),
            (self.u32(b), self.u32(b + 4)),
        );
        for (i, e) in mm.entries.iter().enumerate() {
            let at = memory::entry_addr(i as u8);
            self.check(
                "lbMemory",
                &format!("entry {i}"),
                (entry(e.next), e.addr, e.size),
                (self.u32(at), self.u32(at + 4), self.u32(at + 8)),
            );
        }
        let after = memory::entry_addr(memory::ENTRIES as u8);
        self.check(
            "lbMemory",
            "free entries and counts",
            (entry(mm.free_mem), mm.num_allocs, mm.max_num_allocs),
            (
                self.u32(after),
                self.u32(after + 4) as i32,
                self.u32(after + 8) as i32,
            ),
        );
        for (i, h) in mm.heaps.iter().enumerate() {
            let at = memory::handle_addr(i as u8);
            self.check(
                "lbMemory",
                &format!("handle {i}"),
                (handle(h.next), h.lo, h.hi, entry(h.blocks)),
                (
                    self.u32(at),
                    self.u32(at + 4),
                    self.u32(at + 8),
                    self.u32(at + 0xC),
                ),
            );
        }
        let at = memory::handle_addr(memory::HANDLES as u8);
        self.check(
            "lbMemory",
            "free handles, ARAM heap",
            (handle(mm.free_heap), handle(mm.aram_heap)),
            (self.u32(at), self.u32(at + 4)),
        );
        // The copy manager (`mgr`, but for its alarm, an event on our clock) and the
        // compaction's state.
        self.check(
            "lbMemory",
            "copy manager",
            (
                mm.copy_src,
                mm.copy_dst,
                mm.copy_size,
                mm.copy_offset,
                mm.copy_cb_arg,
                mm.copy_cb,
            ),
            (
                self.u32(at + 0x30),
                self.u32(at + 0x34),
                self.u32(at + 0x38),
                self.u32(at + 0x3C),
                self.u32(at + 0x40),
                self.u32(at + 0x44),
            ),
        );
        self.check(
            "lbMemory",
            "compaction",
            (mm.compact_arg, mm.compact_cursor, mm.compact_cb),
            (
                self.u32(at + 0x48),
                self.u32(at + 0x4C),
                self.u32(at + 0x50),
            ),
        );
        // `lbHeap_80431FA0`.
        let h = &d.heaps;
        self.check(
            "lbHeap",
            "ranges",
            (h.arena_lo, h.arena_hi, h.aram_lo, h.aram_hi),
            (
                self.u32(LB_HEAP),
                self.u32(LB_HEAP + 4),
                self.u32(LB_HEAP + 8),
                self.u32(LB_HEAP + 0xC),
            ),
        );
        for (i, e) in h.heaps.iter().enumerate() {
            let at = LB_HEAP + 0x10 + 0x1C * i as u32;
            self.check(
                "lbHeap",
                &format!("heap {i}"),
                (
                    e.id,
                    e.handle.map_or(u32::MAX, memory::handle_addr),
                    e.start,
                    e.size,
                    e.kind,
                    e.transient,
                    e.status,
                ),
                (
                    self.u32(at) as i32,
                    self.u32(at + 4),
                    self.u32(at + 8),
                    self.u32(at + 0xC),
                    self.u32(at + 0x10) as i32,
                    self.u32(at + 0x14) as i32,
                    self.u32(at + 0x18) as i32,
                ),
            );
        }
        // `preloadCache`.
        let c = &d.cache;
        let p = PRELOAD_CACHE;
        self.check(
            "preloadCache",
            "class",
            c.persistent_heaps,
            self.u32(p) as i32,
        );
        // `PreloadedGameModeState`, its two `bool`s a word each.
        let scene = |cmp: &Comparer, at: u32| SceneCache {
            is_heap_persistent: [cmp.u32(at) != 0, cmp.u32(at + 4) != 0],
            game_cache: melee_sim::lb::dvd::GameCache {
                mode_kind: cmp.u8(at + 8),
                x1: cmp.u8(at + 9),
                x2: cmp.u8(at + 0xA),
                x3: cmp.u8(at + 0xB),
                stkind: cmp.u32(at + 0xC) as i32,
                entries: core::array::from_fn(|i| melee_sim::lb::dvd::CacheFighter {
                    char_id: cmp.u32(at + 0x10 + 8 * i as u32) as i32,
                    color: cmp.u8(at + 0x14 + 8 * i as u32),
                    x5: cmp.u8(at + 0x15 + 8 * i as u32),
                }),
            },
            mode_scene_changes: cmp.u32(at + 0x50) as i32,
        };
        let theirs = scene(self, p + 4);
        self.check("preloadCache", "scene", c.scene, theirs);
        let theirs = scene(self, p + 0x58);
        self.check("preloadCache", "new scene", c.new_scene, theirs);
        for (i, e) in c.entries.iter().enumerate() {
            let at = p + 0xAC + 0x1C * i as u32;
            let theirs = PreloadEntry {
                state: self.u8(at) as i8,
                kind: self.u8(at + 1) as i8,
                heap: self.u8(at + 2) as i8,
                load_state: self.u8(at + 3) as i8,
                class: self.u8(at + 4),
                entry_num: self.u16(at + 6) as i16,
                load_score: self.u16(at + 8) as i16,
                size: self.u32(at + 0xC),
                raw_data: None,
                archive: None,
                effect_index: self.u32(at + 0x18) as i32,
            };
            let ours = PreloadEntry {
                raw_data: None,
                archive: None,
                ..*e
            };
            self.check("preloadCache", &format!("entry {i}"), ours, theirs);
            self.check(
                "preloadCache",
                &format!("entry {i}'s blocks"),
                (entry(e.raw_data), entry(e.archive)),
                (self.u32(at + 0x10), self.u32(at + 0x14)),
            );
        }
        let end = p + 0xAC + 0x1C * 80;
        self.check(
            "preloadCache",
            "compacting heap, preloaded",
            (c.persistent_heap, u32::from(c.preloaded)),
            (self.u32(end) as i32, self.u32(end + 4)),
        );
    }
}
