//! Creating fighters (`fighter.c`): the fighters' first setup for a match, a fighter's
//! creation from its character's data, and its spawn, which a rebirth repeats.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::camera;
use super::super::code::{self, unported_call};
use super::super::handles::{AttrsId, CodeRef, FighterId};
use super::super::state::{Game, OBJ_JOBJ, Running, UserData};
use super::super::structs::Fighter;
use super::data::{FIGHTER_ATTRS_SIZE, figatree_frames, kind_tables};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::hsd::pobj::{self, PObjClass};
use crate::hsd::shadow;

/// The fighter GObjs' classifier and process link (`HSD_GOBJ_CLASS_FIGHTER`).
const GOBJ_CLASS_FIGHTER: u16 = 4;
const P_LINK_FIGHTER: u8 = 8;
/// The user data kind of fighters.
const USER_DATA_FIGHTER: u8 = 4;
/// `ftDrawCommon_80080E18`, the fighters' render callback, on GX link 5.
const RENDER: u32 = 0x8008_0E18;
/// The fighter processes and their priorities, as `Fighter_Create` sets them up.
const PROCS: [(u32, u8); 15] = [
    (0x8006_A1BC, 0),  // Fighter_procHitlag
    (0x8006_A360, 1),  // Fighter_procAnim
    (0x8006_ABA0, 2),  // Fighter_procCpu
    (0x8006_AD10, 3),  // Fighter_procInput
    (0x8006_B82C, 4),  // Fighter_procUpdate
    (0x8006_C27C, 6),  // Fighter_procMap
    (0x8006_C5F4, 7),  // Fighter_procIK
    (0x8006_C624, 8),  // Fighter_procAccessory
    (0x8006_C80C, 9),  // Fighter_procCollPos
    (0x8006_CA5C, 12), // Fighter_procGrabColl
    (0x8006_CB94, 13), // Fighter_procAttackColl
    (0x8006_D1EC, 14), // Fighter_procCollResolve
    (0x8006_D9AC, 16), // Fighter_procDynamics
    (0x8006_D9EC, 18), // Fighter_procCamera
    (0x8006_DA4C, 22), // Fighter_procPlayer
];

/// What `Fighter_Create` is asked to make (`plAllocInfo`): the fighter kind, its player slot,
/// `x5`, and whether it is the slot's second fighter and a transformation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllocInfo {
    pub kind: i32,
    pub slot: u8,
    pub x5: i8,
    pub sub: bool,
    pub transformation: bool,
}

/// Functions of other subsystems fighter creation calls, by address.
pub mod addr {
    /// `efAsync_LoadSync`: the character's effects.
    pub const EFASYNC_LOAD_SYNC: u32 = 0x8006_737C;
    /// `Fighter_Create` itself, for checks that stand it in.
    pub const FIGHTER_CREATE: u32 = 0x8006_8E98;
    /// `Fighter_FirstInitialize_80067A84`, likewise.
    pub const FIRST_INITIALIZE: u32 = 0x8006_7A84;
}

impl Running<'_> {
    /// The fighters' setup at a match's start, `Fighter_FirstInitialize_80067A84` (and
    /// `Fighter_800679B0`): the fighter pools empty, the kinds' data forgotten, `PlCo.dat`'s
    /// common data, the fighters' light and metal and clear models, the spawn counter, and
    /// each kind's own setup. The pools need no setting up here.
    #[doc(alias = "Fighter_FirstInitialize_80067A84")]
    #[doc(alias = "Fighter_800679B0")]
    pub fn fighter_first_init(&mut self) {
        #[cfg(feature = "stubs")]
        if code::stubbed(addr::FIRST_INITIALIZE, &[]).is_some() {
            return;
        }
        self.game.reset_fighter_data();
        // `Fighter_LoadCommonData`'s `lbArchive_LoadSymbols`: from the disc.
        self.disc_load("PlCo.dat", super::data::MAIN_MEMORY);
        self.game.load_fighter_common(self.data);
        self.game.reset_fighter_ref_counts();
        self.create_fighter_light();
        self.load_metal_models();
        self.game.ft.spawn_counter = 1;
        for kind in 0..super::data::KIND_COUNT as i32 {
            match kind_tables(kind).on_first_init {
                0 => {}
                // ftKb_Init_800EE528: Kirby's copied costumes.
                0x800E_E528 => self.game.ft.kirby.reset(),
                // ftPr_Init_8013C2F8: Jigglypuff's hats.
                0x8013_C2F8 => self.game.ft.purin_hats = [None; 5],
                other => unported_call(&format!("fighter first-init callback {other:#010x}")),
            }
        }
    }

    /// Creates a fighter, `Fighter_Create`: its GObj and blocks, its data and costume, its
    /// model, parts, animations and collision, its processes, its spawn, and its first state.
    #[doc(alias = "Fighter_Create")]
    pub fn fighter_create(&mut self, info: AllocInfo) -> GObjId {
        let gobj = self
            .game
            .world
            .create(GOBJ_CLASS_FIGHTER, P_LINK_FIGHTER, 0);
        self.game.world.setup_gx_link(gobj, CodeRef(RENDER), 5, 0);
        // `HSD_ObjAlloc` hands the block back uncleared; a fresh heap's is zero (see
        // `docs/engine.md`).
        let f = FighterId(self.game.fighters.alloc(Fighter::default()));
        // The allocator model's address (`hsd::heap`), else one the frontend seeded, else 0.
        let seeded = self.game.ft.address_seed.pop_front();
        let address = match self.game.fighters.addr(f.0) {
            0 => seeded.unwrap_or(0),
            a => a,
        };
        self.game.ft.addresses.insert(f.0, address);
        let attrs = AttrsId(self.game.fighter_attrs.alloc(vec![0; FIGHTER_ATTRS_SIZE]));
        self.game.fighters[f.0].dat_attrs_backup = Some(attrs);
        self.game
            .world
            .init_user_data(gobj, USER_DATA_FIGHTER, UserData::Fighter(f));
        if let Some(file) = self.game.load_fighter_data(self.data, info.kind) {
            self.disc_load(file, super::data::MAIN_MEMORY);
        }
        self.init_load(gobj, info);
        let kind = self.game.fighters[f.0].kind;
        let effects = kind_tables(kind).effect_file;
        code::call(
            self,
            addr::EFASYNC_LOAD_SYNC,
            &[code::Val::Int(i32::from(effects))],
        );
        let costume = i32::from(self.game.fighters[f.0].costume_id);
        if let Some(file) = self.game.load_costume(self.data, kind, costume) {
            self.disc_load(&file, super::data::MAIN_MEMORY);
        }
        self.load_costume_joint(gobj);
        self.init_anim_buffers(f);
        self.init_parts(f);
        self.setup_parts(gobj);
        self.costume_anims(gobj);
        self.load_second_model(gobj);
        self.set_depth_scale(f);
        self.init_parts_vis(gobj);
        self.game.reset_part_anims(f);
        self.init_bone_dynamics(f);
        self.load_blend_skeleton(gobj);
        self.measure_bones(f);
        self.create_own_light(f);
        self.on_load(gobj);
        // `Fighter_Create_Inline2`.
        let fp = &self.game.fighters[f.0];
        if !fp.no_normal_motion {
            let tree = self.second_figatree(f, 0x23);
            self.game.fighters[f.0].x2EC = figatree_frames(self.data, tree);
            if !self.game.fighters[f.0].is_sandbag {
                for (anim, slot) in [(7, 0), (8, 1), (9, 2), (0x25, 3)] {
                    let tree = self.second_figatree(f, anim);
                    let frames = figatree_frames(self.data, tree);
                    let fp = &mut self.game.fighters[f.0];
                    match slot {
                        0 => fp.x2DC = frames,
                        1 => fp.x2E0 = frames,
                        2 => fp.x2E4 = frames,
                        _ => fp.x2E8 = frames,
                    }
                }
            }
        }
        self.init_hurt_capsules(f);
        let cam_box = camera::alloc_box(&mut self.game.camera.boxes, 0);
        self.game.fighters[f.0].x890_cameraBox = Some(cam_box);
        let root = self.root_jobj(gobj);
        self.create_shadow(f, root);
        for (proc, priority) in PROCS {
            self.game.world.setup_proc(gobj, CodeRef(proc), priority);
        }
        self.fighter_spawn(gobj);
        match kind {
            // ftMh_MS_341_8014FE10, ftCh_Init_80155FCC: the hands' entrances.
            0x1B => unported_call("ftMh_MS_341_8014FE10"),
            0x1C => unported_call("ftCh_Init_80155FCC"),
            _ if info.transformation => self.sleep(gobj),
            _ if self.game.player_flags_bit3(i32::from(info.slot)) => self.enter_entry(gobj),
            _ => {
                if self.game.fighters[f.0].no_normal_motion {
                    panic!("the original's assertion failed: ellegal flag fp->no_normal_motion");
                }
                self.fall_or_wait(gobj);
            }
        }
        self.game.ft_disable_input(gobj);
        gobj
    }

    /// The fighter's identity and settings from its player slot, `Fighter_UnkInitLoad_80068914`.
    #[doc(alias = "Fighter_UnkInitLoad_80068914")]
    fn init_load(&mut self, gobj: GObjId, info: AllocInfo) {
        let f = self.game.fighter_id(gobj);
        let slot = i32::from(info.slot);
        let g = &*self.game;
        let scale = g.player_model_scale(slot);
        let port = g.player_port(slot) as u8;
        let sub_color = g.player_sub_color(slot);
        let always_metal = g.player_is_metal(slot);
        let flags_bit6 = g.player_flags_bit6(slot);
        let flags_bit7 = g.player_flags_bit7(slot);
        let more_bit1 = g.player_more_flags_bit1(slot);
        let more_bit2 = g.player_more_flags_bit2(slot);
        let more_bit6 = g.player_more_flags_bit6(slot);
        let ae_bit0 = g.player_flags_ae_bit0(slot);
        let fp = &mut self.game.fighters[f.0];
        fp.kind = info.kind;
        fp.player_idx = info.slot;
        fp.is_sub_fighter = info.sub;
        fp.x34_scale.x = scale;
        fp.x61C = info.x5;
        fp.pad_port = port;
        fp.sub_color = sub_color;
        fp.is_always_metal = always_metal;
        fp.x2226_b3 = flags_bit6;
        fp.x2226_b6 = flags_bit7;
        fp.x2225_b5 = more_bit1;
        fp.x2225_b7 = more_bit2;
        fp.x2228_b3 = more_bit6;
        fp.x2229_b1 = ae_bit0;
        assert!(
            sub_color <= 4,
            "the original's assertion failed: fighter sub color num over!"
        );
        if sub_color != 0 {
            let at = self.common().at(0x6DC + 4 * u32::from(sub_color - 1));
            let c = [
                self.data.u8(at),
                self.data.u8(at.at(1)),
                self.data.u8(at.at(2)),
                self.data.u8(at.at(3)),
            ];
            let mul = |v: u8| (i32::from(v) * i32::from(c[3]) / 255) as u8;
            let rgba = &mut self.game.fighters[f.0].x610_color_rgba[0];
            rgba.r = mul(c[0]);
            rgba.g = mul(c[1]);
            rgba.b = mul(c[2]);
            rgba.a = c[3];
        }
        let mut costume = self.game.player_costume(slot) as i32;
        if costume >= i32::from(kind_tables(info.kind).costume_count) {
            costume = 0;
        }
        let team = self.game.player_team(slot) as u8;
        let ft_data = self.game.ft.data.data_list[info.kind as usize];
        let fp = &mut self.game.fighters[f.0];
        fp.costume_id = costume as u8;
        fp.team = team;
        fp.gobj = Some(gobj);
        fp.ft_data = ft_data;
        self.copy_attributes(f);
        let ft_data = ft_data.expect("a fighter kind without data");
        let anims = self.data.ptr(ft_data.at(0xC));
        let anim_flags = self.data.ptr(ft_data.at(0x10));
        let fp = &mut self.game.fighters[f.0];
        fp.x2CC = Default::default();
        fp.x2D0 = Default::default();
        fp.x18 = 0x155;
        fp.x1C_actionStateList = super::super::handles::RawPtr(MOTION_STATE_LIST);
        fp.x20_actionStateList = super::super::handles::RawPtr(kind_tables(info.kind).state_table);
        fp.x24 = anims;
        fp.x28 = anim_flags;
        fp.input.lstick[2].y = 0.0;
        fp.input.lstick[2].x = 0.0;
        fp.input.cstick[2].y = 0.0;
        fp.input.cstick[2].x = 0.0;
        fp.input.triggers[2] = 0.0;
        fp.input.held_buttons[2] = 0;
        reset_inputs(fp);
        fp.x594 = [0; 4];
        fp.x21FC_flag[0] = 1;
        fp.invisible = false;
        fp.x221E_b1 = false;
        fp.x221E_b2 = false;
        fp.x221F_b1 = false;
        fp.x221F_b2 = false;
        fp.x209A = 0;
        fp.x221E_b5 = false;
        fp.x221F_b0 = false;
        fp.x21EC = None;
        fp.has_prev_input = false;
        fp.input_disabled = false;
        fp.is_sleeping = false;
        fp.x2220_b0 = 0;
        fp.x2221_b2 = false;
        fp.no_normal_motion = false;
        fp.x2229_b6 = false;
        fp.no_kb = false;
        fp.x222A_b0 = false;
        fp.x222A_b1 = false;
        fp.x2228_b5 = false;
        fp.used_tether = false;
        fp.x2221_b3 = false;
        fp.x2222_b0 = false;
        fp.can_multijump = false;
        fp.x2222_b4 = false;
        fp.x2222_b5 = false;
        fp.x2222_b6 = false;
        fp.x2223_b1 = false;
        fp.x40 = 0.0;
        fp.can_walljump = false;
        fp.x60C = Default::default();
        fp.x2225_b3 = false;
        fp.is_sandbag = false;
        fp.x2226_b0 = false;
        fp.x2226_b1 = false;
        fp.x2227_b0 = false;
        fp.x2224_b0 = false;
        fp.smash_attrs.x2135 = -1;
        fp.x2184 = None;
        fp.x2229_b3 = false;
    }

    /// The fighter's attributes, item pickup offsets and `x2C4` from its character's data,
    /// `ftCo_800D0FA0`.
    #[doc(alias = "ftCo_800D0FA0")]
    pub(super) fn copy_attributes(&mut self, f: FighterId) {
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let attrs = self.data.ptr(d).expect("ftData::x0");
        let pickup = self.data.ptr(d.at(0x40)).expect("ftData::x40");
        let x2c4 = self.data.ptr(d.at(0x50)).expect("ftData::x50");
        let fp = &mut self.game.fighters[f.0];
        fp.co_attrs = super::data::copy_from_data(self.data, attrs);
        fp.x294_itPickup = super::data::copy_from_data(self.data, pickup);
        fp.x2C4 = super::data::copy_from_data(self.data, x2c4);
    }

    /// Loads the fighter's costume model as its GObj's joint tree,
    /// `Fighter_UnkUpdateCostumeJoint_800686E4`: shapes of the fighters' class, the root of
    /// the fighters' joint class (`ftParts_80073758`, whose methods melee-sim's joints don't
    /// need, but whose count the allocator model keeps).
    #[doc(alias = "Fighter_UnkUpdateCostumeJoint_800686E4")]
    #[doc(alias = "ftParts_80073758")]
    #[doc(alias = "ftPartsPObjSetDefaultClass")]
    #[doc(alias = "ftPartsPObjClearDefaultClass")]
    fn load_costume_joint(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let joint = self.game.ft.data.costumes[fp.kind as usize][usize::from(fp.costume_id)].joint;
        self.game.fighters[f.0].x108_costume_joint = joint;
        pobj::set_default_class(&mut self.game.objs.pobjs, PObjClass::Fighter);
        let j = jobj::load_joint(&mut self.game.objs, self.data, joint);
        pobj::set_default_class(&mut self.game.objs.pobjs, PObjClass::Hsd);
        let j = j.expect("a costume without a model");
        // `hsdChangeClass(jobj, &ftJObj)`.
        let jobjs = &mut self.game.objs.jobjs;
        crate::hsd::heap::change_class(jobjs[j].class, crate::hsd::heap::Class::FtJObj);
        jobjs[j].class = crate::hsd::heap::Class::FtJObj;
        self.game
            .world
            .set_obj(gobj, OBJ_JOBJ, super::super::state::Obj::JObj(j));
    }

    /// The fighter's depth scale: Flat Zone's for its flat fighters, else 1,
    /// `Fighter_80068E64`.
    #[doc(alias = "Fighter_80068E64")]
    fn set_depth_scale(&mut self, f: FighterId) {
        // `Gr_Kind_Flatzone`.
        let z = if self.game.stage_info.grkind == 27 {
            self.common().f32(0x7E4)
        } else {
            1.0
        };
        self.game.fighters[f.0].x34_scale.z = z;
    }

    /// The fighter's height ratio and its bone 2 to bone 1 offset,
    /// `Fighter_UnkUpdateVecFromBones_8006876C`.
    #[doc(alias = "Fighter_UnkUpdateVecFromBones_8006876C")]
    fn measure_bones(&mut self, f: FighterId) {
        let b2 = self
            .bone(f, self.bone_index(&self.game.fighters[f.0], 2))
            .joint;
        let b1 = self
            .bone(f, self.bone_index(&self.game.fighters[f.0], 1))
            .joint;
        let j2 = b2.expect("a fighter without part 2");
        let t = self.game.objs.jobjs[j2].translate;
        self.game.fighters[f.0].x1A6C = mem(fp::fdivs(reg(t.y), reg(8.55)));
        let v = crate::lb::joint::world_position(&mut self.game.objs, self.data, b2, None);
        let v2 = crate::lb::joint::world_position(&mut self.game.objs, self.data, b1, None);
        let fp = &mut self.game.fighters[f.0];
        fp.x1A70.x = mem(fp::fsubs(reg(v2.x), reg(v.x)));
        fp.x1A70.y = mem(fp::fsubs(reg(v2.y), reg(v.y)));
        fp.x1A70.z = mem(fp::fsubs(reg(v2.z), reg(v.z)));
    }

    /// The fighter's shadow, cast from its model, `lbShadow_8000ED54`.
    #[doc(alias = "lbShadow_8000ED54")]
    fn create_shadow(&mut self, f: FighterId, root: crate::hsd::jobj::JObjId) {
        let objs = &mut self.game.objs;
        let s = shadow::alloc(objs);
        let camera = objs.shadows[s.0].camera;
        crate::hsd::cobj::set_projection_type(&mut objs.cobjs, camera, 3);
        crate::hsd::cobj::set_near(&mut objs.cobjs, camera, mem(reg(0.001)));
        crate::hsd::cobj::set_far(&mut objs.cobjs, camera, 5000.0);
        crate::hsd::cobj::set_flags(&mut objs.cobjs, camera, 1);
        shadow::set_size(objs, s, 0x100, 0x100);
        let sh = &mut objs.shadows[s.0];
        sh.intensity = 0xC0;
        sh.scale_s = 0.45;
        sh.scale_t = -0.45;
        shadow::add_object(objs, Some(s), Some(root));
        shadow::set_active(objs, s, true);
        let ls = &mut self.game.fighters[f.0].x20A4;
        ls.x0_b0 = false;
        ls.x0_b1 = false;
        ls.x0_b3 = false;
        ls.x0_b4 = false;
        ls.x0_b5 = false;
        ls.shadow = Some(s);
    }
}

/// `ftData_MotionStateList`, the common action states.
const MOTION_STATE_LIST: u32 = 0x803C_2800;

/// Clears the controls and their timers, `Fighter_UnkInitLoad_80068914_Inner1`.
fn reset_inputs(fp: &mut Fighter) {
    let i = &mut fp.input;
    i.triggers[0] = 0.0;
    i.triggers[1] = 0.0;
    i.cstick[0].x = 0.0;
    i.cstick[0].y = 0.0;
    i.cstick[1].x = 0.0;
    i.cstick[1].y = 0.0;
    i.lstick[0].x = 0.0;
    i.lstick[0].y = 0.0;
    i.lstick[1].x = 0.0;
    i.lstick[1].y = 0.0;
    i.held_buttons[1] = 0;
    i.released_buttons = 0;
    i.pressed_buttons = 0;
    i.held_buttons[0] = 0;
    for t in [
        &mut fp.activity_timer,
        &mut fp.active_duration,
        &mut fp.active_sticky,
        &mut fp.active_timer,
    ] {
        t.lstick.x = 254;
        t.lstick.y = 254;
        t.trigger = 254;
    }
    fp.x67C = 255;
    fp.x67D = 255;
    fp.x67E = 255;
    fp.x67F = 255;
    fp.x680 = 255;
    fp.x681 = 255;
    fp.x682 = 255;
    fp.x683 = 255;
    fp.x684 = 255;
    fp.x685 = 255;
    fp.x686 = 255;
    fp.x687 = 255;
    fp.x688 = 255;
    fp.x689 = 255;
    fp.x68A = 255;
    fp.x68B = 255;
}

impl Game {
    /// Clears the fighter's controls and stops it reading its controller,
    /// `ftLib_DisableInput` (with `Fighter_ResetInputData_80068854`).
    #[doc(alias = "ftLib_DisableInput")]
    #[doc(alias = "Fighter_ResetInputData_80068854")]
    pub fn ft_disable_input(&mut self, gobj: GObjId) {
        let fp = self.fighter_mut(gobj);
        reset_inputs(fp);
        fp.input_disabled = true;
    }

    /// The next spawn number, `Fighter_NewSpawn_80068E40`: the counter skips 0 when it wraps.
    #[doc(alias = "Fighter_NewSpawn_80068E40")]
    pub fn new_spawn_number(&mut self) -> u32 {
        let n = self.ft.spawn_counter;
        self.ft.spawn_counter = n.wrapping_add(1);
        if self.ft.spawn_counter == 0 {
            self.ft.spawn_counter = 1;
        }
        n
    }

    /// Finds `PlCo.dat`'s common data, `Fighter_LoadCommonData`.
    #[doc(alias = "Fighter_LoadCommonData")]
    pub fn load_fighter_common(&mut self, data: &melee_data::archive::Archives) {
        let archive = self.ft.data.files.archive("PlCo.dat");
        let offset = data
            .get(archive)
            .public("ftLoadCommonData")
            .expect("the original's assertion failed: Cannot find symbol ftLoadCommonData.");
        self.ft.load_common = Some(DataRef { archive, offset });
    }
}

impl Running<'_> {
    /// Puts the fighter to sleep, out of the match until it's woken (a transformation's
    /// other half, Zelda while Sheik plays), `ftCo_800BFD04`: the sleeping state, invisible,
    /// out of the camera's framing.
    #[doc(alias = "ftCo_800BFD04")]
    pub fn sleep(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, super::co::ids::ms::SLEEP, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.invisible = true;
        fp.x221E_b1 = true;
        fp.x221E_b2 = true;
        fp.x2219_b1 = true;
        let b = fp.x890_cameraBox.expect("a fighter without a camera box");
        fp.is_sleeping = true;
        fp.x221F_b1 = true;
        self.game.camera.boxes[b].state = super::super::camera::INACTIVE;
    }
}
