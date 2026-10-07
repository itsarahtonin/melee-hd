//! The match's state in a capture: the player blocks, the pad library (its queue, rumble and
//! statuses) and the button maps, the scene loop's state and frame clocks, the VS match's
//! controller, the player library's counters and the game-mode layer's globals.

use melee_sim::game::gm::pause::PauseScreen;
use melee_sim::game::gm::vs::Ranks;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::mirror::{Cx, Mem, Mirror};
use melee_sim::game::structs::{_StaticPlayer, gm_80479D58_t};
use melee_sim::hsd::gobj::GObjId;
use melee_sim::input::controller::{PadData, PadLib, Status};
use melee_sim::input::pad::PadStatus;
use melee_sim::input::rumble::{Motor, Node, NodeId, Port, Rumble, Script, Table};
use melee_sim::lb::pacing::{Clock, Pacing};

use crate::compare::Comparer;
use crate::import::{Importer, sym};
use crate::{DataMap, MEM1};

const STATUS_SIZE: u32 = 0x44;
const PAD_SIZE: u32 = 0xC;
const NODE_SIZE: u32 = 0x20;
const RUMBLE_NODES: u32 = 12;

/// Reads big-endian memory.
struct R<'a> {
    mem: &'a [u8],
}

impl R<'_> {
    fn u8(&self, a: u32) -> u8 {
        self.mem[(a - MEM1) as usize]
    }

    fn u16(&self, a: u32) -> u16 {
        let i = (a - MEM1) as usize;
        u16::from_be_bytes(self.mem[i..i + 2].try_into().unwrap())
    }

    fn u32(&self, a: u32) -> u32 {
        let i = (a - MEM1) as usize;
        u32::from_be_bytes(self.mem[i..i + 4].try_into().unwrap())
    }

    fn u64(&self, a: u32) -> u64 {
        u64::from(self.u32(a)) << 32 | u64::from(self.u32(a + 4))
    }

    fn f32(&self, a: u32) -> f32 {
        f32::from_bits(self.u32(a))
    }

    /// An `HSD_PadStatus`.
    fn status(&self, a: u32) -> Status {
        let b = |o: u32| self.u8(a + o);
        let f = |o: u32| self.f32(a + o);
        Status {
            button: self.u32(a),
            last_button: self.u32(a + 4),
            trigger: self.u32(a + 8),
            repeat: self.u32(a + 0xC),
            release: self.u32(a + 0x10),
            repeat_count: self.u32(a + 0x14) as i32,
            stick_x: b(0x18) as i8,
            stick_y: b(0x19) as i8,
            sub_stick_x: b(0x1A) as i8,
            sub_stick_y: b(0x1B) as i8,
            analog_l: b(0x1C),
            analog_r: b(0x1D),
            analog_a: b(0x1E),
            analog_b: b(0x1F),
            nml_stick_x: f(0x20),
            nml_stick_y: f(0x24),
            nml_sub_stick_x: f(0x28),
            nml_sub_stick_y: f(0x2C),
            nml_analog_l: f(0x30),
            nml_analog_r: f(0x34),
            nml_analog_a: f(0x38),
            nml_analog_b: f(0x3C),
            cross_dir: b(0x40),
            err: b(0x41) as i8,
        }
    }

    /// A `PADStatus`.
    fn pad(&self, a: u32) -> PadStatus {
        let b = |o: u32| self.u8(a + o);
        PadStatus {
            button: self.u16(a),
            stick_x: b(2) as i8,
            stick_y: b(3) as i8,
            substick_x: b(4) as i8,
            substick_y: b(5) as i8,
            trigger_left: b(6),
            trigger_right: b(7),
            analog_a: b(8),
            analog_b: b(9),
            err: b(0xA) as i8,
        }
    }

    /// `HSD_PadLibData` and its queue.
    fn pad_lib(&self) -> PadLib {
        let at = sym("HSD_PadLibData");
        let b = |o: u32| self.u8(at + o);
        let qnum = b(0);
        let queue_at = self.u32(at + 8);
        let queue: Vec<PadData> = (0..u32::from(qnum))
            .map(|q| core::array::from_fn(|i| self.pad(queue_at + (4 * q + i as u32) * PAD_SIZE)))
            .collect();
        PadLib {
            qnum,
            qread: b(1),
            qwrite: b(2),
            qcount: b(3),
            qtype: b(4),
            queue,
            repeat_start: self.u32(at + 0xC) as i32,
            repeat_interval: self.u32(at + 0x10) as i32,
            adc_type: b(0x14) as i8,
            adc_th: b(0x15) as i8,
            adc_angle: self.f32(at + 0x18),
            clamp_stick_type: b(0x1C),
            clamp_stick_shift: b(0x1D),
            clamp_stick_max: b(0x1E) as i8,
            clamp_stick_min: b(0x1F) as i8,
            clamp_analog_lr_shift: b(0x20),
            clamp_analog_lr_max: b(0x21),
            clamp_analog_lr_min: b(0x22),
            clamp_analog_ab_shift: b(0x23),
            clamp_analog_ab_max: b(0x24),
            clamp_analog_ab_min: b(0x25),
            scale_stick: b(0x26) as i8,
            scale_analog_lr: b(0x27),
            scale_analog_ab: b(0x28),
            cross_dir: b(0x29),
            reset_switch_status: b(0x2A),
            reset_switch: b(0x2B),
        }
    }

    /// The rumble library, its nodes at `gmMain_8046B1F8`.
    fn rumble(&self, data: &DataMap) -> Result<Rumble, String> {
        let info = sym("HSD_PadLibData") + 0x2C;
        let ports = sym("HSD_Rumble_804C22E0");
        let nodes = sym("gmMain_8046B1F8");
        let node = |a: u32| -> Result<Option<NodeId>, String> {
            if a == 0 {
                return Ok(None);
            }
            let off = a.wrapping_sub(nodes);
            if !off.is_multiple_of(NODE_SIZE) || off / NODE_SIZE >= RUMBLE_NODES {
                return Err(format!("rumble node {a:#x} outside gmMain_8046B1F8"));
            }
            Ok(Some(NodeId((off / NODE_SIZE) as u16)))
        };
        let vibration = sym("mnVibration_804D4FF0");
        let script = |a: u32| -> Result<Option<Script>, String> {
            if a == 0 {
                return Ok(None);
            }
            if (vibration..vibration + 4).contains(&a) {
                return Ok(Some(Script::Static {
                    table: Table::Vibration,
                    index: (a - vibration) / 2,
                }));
            }
            data.get(a)
                .map(|d| Some(Script::Data(d)))
                .ok_or_else(|| format!("rumble script {a:#x} outside loaded archives"))
        };
        let motor = |v: u8| match v {
            0 => Motor::StopHard,
            1 => Motor::Stop,
            _ => Motor::Rumble,
        };
        let mut r = Rumble::new(RUMBLE_NODES as u16);
        r.max_list = self.u16(info);
        r.suspended = self.u8(info + 2) != 0;
        r.free = node(self.u32(info + 4))?;
        for (i, p) in r.ports.iter_mut().enumerate() {
            let a = ports + PAD_SIZE * i as u32;
            *p = Port {
                last_status: motor(self.u8(a)),
                status: motor(self.u8(a + 1)),
                direct_status: motor(self.u8(a + 2)),
                nb_list: self.u16(a + 4),
                list: node(self.u32(a + 8))?,
            };
        }
        for (i, n) in r.nodes.iter_mut().enumerate() {
            let a = nodes + NODE_SIZE * i as u32;
            *n = Node {
                next: node(self.u32(a))?,
                id: self.u32(a + 4),
                paused: self.u8(a + 8) != 0,
                pri: self.u8(a + 9),
                status: motor(self.u8(a + 0xA)),
                loop_count: self.u16(a + 0xC),
                wait: self.u16(a + 0xE),
                frame: self.u32(a + 0x10) as i32,
                stack: script(self.u32(a + 0x14))?,
                listp: script(self.u32(a + 0x18))?,
                headp: script(self.u32(a + 0x1C))?,
            };
        }
        Ok(r)
    }

    /// `lb_804329F0`'s clocks.
    fn pacing(&self) -> Pacing {
        let at = sym("lb_804329F0");
        let clock = |i: u32| {
            let c = at + 0x18 * i;
            Clock {
                period: self.u64(c) as i64,
                elapsed: self.u64(c + 8) as i64,
                due: self.u32(c + 0x10) != 0,
            }
        };
        Pacing {
            clocks: [clock(0), clock(1)],
            sampling_rate: self.u32(at + 0x30),
            requested: self.u64(at + 0x38),
            tick: self.u64(at + 0x40) as i64,
            alarm: self.u32(at + 0x48) != 0,
        }
    }

    /// `gm_16F1.c`'s live standings.
    fn ranks(&self) -> Ranks {
        let a = sym("lbl_8046DBC8");
        let r = sym("lbl_804D65A8");
        Ranks {
            taken: self.u8(a),
            x1: self.u8(a + 1),
            first: core::array::from_fn(|i| {
                let e = a + 2 + 2 * i as u32;
                (self.u8(e), self.u8(e + 1))
            }),
            rank: core::array::from_fn(|i| self.u8(r + i as u32)),
            lowest: self.u32(sym("lbl_804D65B0")) as i32,
        }
    }

    /// The running scene's frame function: `gm_804D6720`'s scene kind's entry in `scenes`.
    fn on_frame(&self) -> Option<CodeRef> {
        let info = self.u32(sym("gm_804D6720"));
        if info == 0 {
            return None;
        }
        let kind = self.u8(info);
        let table = sym("scenes");
        (0..0x384 / 0x14)
            .map(|i| table + 0x14 * i)
            .find(|&e| self.u8(e) == kind)
            .and_then(|e| melee_sim::game::mirror::code(self.u32(e + 4)))
    }
}

impl<'a> Importer<'a> {
    fn r(&self) -> R<'a> {
        let cap: &'a crate::Capture = self.cap;
        R {
            mem: &cap.call.mem1,
        }
    }

    /// The pad library: queue, settings, rumble and the three statuses.
    pub fn pads(&mut self) {
        let r = self.r();
        let lib = r.pad_lib();
        let rumble = r.rumble(self.data);
        let status = |name: &str, i: u32| r.status(sym(name) + STATUS_SIZE * i);
        let master = core::array::from_fn(|i| status("HSD_PadMasterStatus", i as u32));
        let copy = core::array::from_fn(|i| status("HSD_PadCopyStatus", i as u32));
        let game = core::array::from_fn(|i| status("HSD_PadGameStatus", i as u32));
        let p = &mut self.game.pads;
        p.lib = lib;
        p.master = master;
        p.copy = copy;
        p.game = game;
        match rumble {
            Ok(rumble) => p.rumble = rumble,
            Err(e) => self.unhandled.push(e),
        }
        let table = r.u32(sym("lb_804D63C0"));
        self.game.rumble_data = (table != 0).then(|| self.data.get(table)).flatten();
    }

    /// The match's state: the player blocks, pads and button maps (as [`Importer::globals`]
    /// imports them), the scene loop, the clocks, the VS controller, the player library's
    /// counters and the game-mode globals.
    pub fn match_state(&mut self) {
        self.globals();
        self.pads();
        self.playback();
        let cap = self.cap;
        let mem = Mem {
            bytes: &cap.call.mem1,
            base: MEM1,
        };
        self.game.scene = gm_80479D58_t::import(&mem, sym("gm_80479D58"), self);
        let r = self.r();
        let pacing = r.pacing();
        let ranks = r.ranks();
        let on_frame = r.on_frame();
        let common = r.u32(sym("pl_804D6470"));
        let stale = r.u16(sym("staleAttackInstance"));
        let attack = r.u16(sym("unk_804D6480"));
        let db_level = r.u32(sym("DbLevel")) as i32;
        let scene_index = r.u8(sym("state_machine") + 3);
        let spawn_flags = r.u8(sym("lbl_8046B488") + 0x10);
        let scene_gobj = r.u32(sym("gm_804D672C"));
        let pause = sym("lbl_80479B10");
        let pause_words: [u32; 5] = core::array::from_fn(|i| r.u32(pause + 4 * i as u32));
        let pause_slot = r.u32(pause + 0x14) as i32;
        let pause_archive = r.u32(sym("lbl_804D6700"));
        let pause_archive_data = if pause_archive != 0 {
            r.u32(pause_archive + 0x20)
        } else {
            0
        };
        let pause_gobj = r.u32(sym("lbl_804D6704"));
        self.game.pacing = pacing;
        let _ = common;
        self.game.pl.load_common = self.data.public("PdPm.dat", "plLoadCommonData");
        self.game.pl.stale_attack_instance = stale;
        self.game.pl.attack_instance = attack;
        let jobj = |s: &mut Self, a: u32| (a != 0).then(|| s.jobj(a));
        let pause = PauseScreen {
            background: jobj(self, pause_words[0]),
            analog_stick: jobj(self, pause_words[1]),
            lras: jobj(self, pause_words[2]),
            z: jobj(self, pause_words[3]),
            analog_stick_outline: jobj(self, pause_words[4]),
            slot: pause_slot,
            archive: (pause_archive_data != 0)
                .then(|| self.data.get(pause_archive_data).map(|d| d.archive))
                .flatten(),
            gobj: self.import_handle("GObjId", pause_gobj).map(GObjId),
        };
        let g = &mut self.game.gm;
        g.db_level = db_level;
        g.on_frame = on_frame;
        g.scene_index = scene_index;
        g.ranks = ranks;
        g.spawn_flags = spawn_flags;
        g.pause = pause;
        self.game.gm.scene_gobj = self.import_handle("GObjId", scene_gobj).map(GObjId);
        // The scheduler reads the scene loop's pause mask through `HSD_GObjLibInitData`.
        let mask_at = r.u32(sym("HSD_GObjLibInitData") + 8);
        self.game.world.skip_links = if mask_at != 0 { r.u64(mask_at) } else { 0 };
        self.hud();
    }
}

impl<'a> Comparer<'a> {
    fn r(&self) -> R<'a> {
        R { mem: self.mem }
    }

    /// The player blocks, the game mode, the pad statuses and library, rumble and the button
    /// maps.
    pub fn globals(&mut self) {
        let mem = Mem {
            bytes: self.mem,
            base: MEM1,
        };
        let game = self.game;
        let players = sym("player_slots");
        for (i, p) in game.players.iter().enumerate() {
            p.compare(
                &mem,
                players + i as u32 * _StaticPlayer::SIZE,
                &format!("player {i}"),
                self,
            );
        }
        self.check(
            "state_machine",
            "game mode",
            game.game_mode,
            self.u8(sym("state_machine")),
        );
        game.controller_map
            .compare(&mem, sym("controller_map"), "controller_map", self);
        let r = self.r();
        let lib = r.pad_lib();
        let mut ours = game.pads.lib.clone();
        let mut theirs = lib;
        self.check(
            "HSD_PadLibData",
            "adc_angle",
            ours.adc_angle.to_bits(),
            theirs.adc_angle.to_bits(),
        );
        (ours.adc_angle, theirs.adc_angle) = (0.0, 0.0);
        self.check("HSD_PadLibData", "library", ours, theirs);
        for (name, statuses) in [
            ("HSD_PadMasterStatus", &game.pads.master),
            ("HSD_PadCopyStatus", &game.pads.copy),
            ("HSD_PadGameStatus", &game.pads.game),
        ] {
            for (i, s) in statuses.iter().enumerate() {
                let t = self.r().status(sym(name) + STATUS_SIZE * i as u32);
                self.check(
                    &format!("{name}[{i}]"),
                    "status",
                    status_bits(s),
                    status_bits(&t),
                );
            }
        }
        match self.r().rumble(self.data) {
            Ok(t) => self.check("rumble", "state", game.pads.rumble.clone(), t),
            Err(e) => self.diffs.push(format!("rumble: {e}")),
        }
        let table = self.r().u32(sym("lb_804D63C0"));
        let ours = game.rumble_data.map_or(0, |r| self.data.addr(r));
        self.check("lb_804D63C0", "rumble patterns", ours, table);
    }

    /// The match's state, as [`Importer::match_state`] imports it, after [`Comparer::globals`].
    pub fn match_state(&mut self) {
        self.globals();
        self.playback();
        let mem = Mem {
            bytes: self.mem,
            base: MEM1,
        };
        let game = self.game;
        game.scene
            .compare(&mem, sym("gm_80479D58"), "gm_80479D58", self);
        let r = self.r();
        self.check("lb_804329F0", "clocks", game.pacing.clone(), r.pacing());
        self.check("gm_16F1", "ranks", game.gm.ranks.clone(), r.ranks());
        let common = r.u32(sym("pl_804D6470"));
        // `pl_804D6470` is the first pointer of the root we keep.
        let kept = game
            .pl
            .load_common
            .map_or(0, |r| self.u32(self.data.addr(r)));
        self.check("pl_804D6470", "common data", kept, common);
        self.check(
            "pl counters",
            "attack instances",
            (game.pl.stale_attack_instance, game.pl.attack_instance),
            (
                r.u16(sym("staleAttackInstance")),
                r.u16(sym("unk_804D6480")),
            ),
        );
        self.check(
            "lbl_8046B488",
            "spawn flags",
            game.gm.spawn_flags,
            r.u8(sym("lbl_8046B488") + 0x10),
        );
        let scene = r.u32(sym("gm_804D672C"));
        self.compare_handle(
            "gm_804D672C",
            "GObjId",
            game.gm.scene_gobj.map(|g| g.0),
            scene,
        );
        let pause = sym("lbl_80479B10");
        let p = &game.gm.pause;
        for (i, (name, j)) in [
            ("background", p.background),
            ("analog_stick", p.analog_stick),
            ("lras", p.lras),
            ("z", p.z),
            ("analog_stick_outline", p.analog_stick_outline),
        ]
        .into_iter()
        .enumerate()
        {
            let t = r.u32(pause + 4 * i as u32);
            self.compare_handle(&format!("pause {name}"), "JObjId", j.map(|j| j.0), t);
        }
        self.check("pause", "slot", p.slot, r.u32(pause + 0x14) as i32);
        let pg = r.u32(sym("lbl_804D6704"));
        self.compare_handle("pause gobj", "GObjId", p.gobj.map(|g| g.0), pg);
        let mask_at = r.u32(sym("HSD_GObjLibInitData") + 8);
        if mask_at != 0 {
            self.check(
                "pause mask",
                "skip links",
                game.world.skip_links,
                r.u64(mask_at),
            );
        }
        self.hud();
    }
}

/// A status with its floats as bit patterns, for exact comparison.
fn status_bits(s: &Status) -> String {
    let nml = [
        s.nml_stick_x,
        s.nml_stick_y,
        s.nml_sub_stick_x,
        s.nml_sub_stick_y,
        s.nml_analog_l,
        s.nml_analog_r,
        s.nml_analog_a,
        s.nml_analog_b,
    ];
    let mut plain = *s;
    plain.nml_stick_x = 0.0;
    plain.nml_stick_y = 0.0;
    plain.nml_sub_stick_x = 0.0;
    plain.nml_sub_stick_y = 0.0;
    plain.nml_analog_l = 0.0;
    plain.nml_analog_r = 0.0;
    plain.nml_analog_a = 0.0;
    plain.nml_analog_b = 0.0;
    let bits: Vec<String> = nml.iter().map(|f| format!("{:08x}", f.to_bits())).collect();
    format!("{plain:?} nml {}", bits.join(" "))
}
