//! Dispatch of the original's function pointers. A [`CodeRef`] is the original function's
//! address; each table here maps the addresses a kind of pointer can hold to their ports.

use super::camera;
use super::ft::fighter;
use super::handles::CodeRef;
use super::state::Running;
use melee_data::archive::Archives;

use crate::hsd::Objects;
use crate::hsd::generator::GeneratorId;
use crate::hsd::gobj::GObjId;
use crate::hsd::particle::{Callback, GeneratorHook, ParticleId, UserData, UserFunc};

/// Runs a GObj process (`HSD_GObjEvent`), or a callback of the same shape (a stage object's
/// setup and callbacks).
pub fn run_proc(game: &mut Running, gobj: GObjId, proc: CodeRef) {
    use super::gr::{dynamic_attr, ground, last};
    match proc.0 {
        camera::PROC => camera::proc(game, gobj),
        0x801C_1CD0 => ground::proc_anim(game, gobj),
        0x801C_1D38 => ground::proc_callback(game, gobj),
        0x801C_0C2C => ground::proc_watch(game, gobj),
        0x801C_461C => ground::proc_lights(game, gobj),
        // mpLib_800587FC: the stage collision GObj's process.
        0x8005_87FC => {
            game.game
                .geometry
                .as_mut()
                .expect("no collision geometry")
                .measure_floors();
            dynamic_attr::update(game.game);
        }
        0x801C_ADBC => super::gr::zako::proc(game, gobj),
        super::playback::SEED_PROC => game.game.playback_restore_seed(),
        // The recording codes' processes send the frame over EXI and change nothing.
        super::playback::RECORD_FRAME_START_PROC | super::playback::RECORD_ITEMS_PROC => {}
        super::gm::vs::PROC => game.vs_proc(),
        // fn_801A4BD0: the scene GObj's first process, empty.
        super::gm::vs::SCENE_PROC => {}
        super::gm::pause::PROC => super::gm::pause::proc(game, gobj),
        addr if last::run(game, gobj, addr) => {}
        addr if super::gr::battle::run(game, gobj, addr) => {}
        0x8006_A1BC => fighter::proc_hitlag(game, gobj),
        0x8006_A360 => fighter::proc_anim(game, gobj),
        0x8006_ABA0 => fighter::proc_cpu(game, gobj),
        0x8006_AD10 => game.proc_input(gobj),
        0x8006_B82C => fighter::proc_update(game, gobj),
        0x8006_C27C => fighter::proc_map(game, gobj),
        0x8006_C624 => fighter::proc_accessory(game, gobj),
        0x8006_C80C => fighter::proc_coll_pos(game, gobj),
        0x8006_CA5C => fighter::proc_grab_coll(game, gobj),
        addr if super::it::item::run_proc(game, gobj, addr) => {}
        addr if super::it::kinds::run_event(game, gobj, addr) => {}
        super::ef::UPDATE_PROC => super::ef::update(game, gobj),
        super::ef::PARTICLES_PROC_MAIN => super::ef::particles_proc_main(game),
        super::ef::PARTICLES_PROC_AUX => super::ef::particles_proc_aux(game),
        other => unported("process", other),
    }
}

/// Runs a callback that answers a question about its GObj (`HSD_GObjPredicate`: an item's
/// state callbacks, which say whether to destroy it).
pub fn run_pred(game: &mut Running, gobj: GObjId, f: CodeRef) -> bool {
    match super::it::kinds::run_pred(game, gobj, f.0) {
        Some(b) => b,
        None => unported("predicate", f.0),
    }
}

/// Runs a function without arguments (`Event`: a stage's setup, load and start hooks).
pub fn run_event(game: &mut Running, f: u32) {
    use super::gr::{battle, last};
    match f {
        0x8021_9CA4 => battle::on_init(game),
        0x8021_9D54 => battle::on_load(game),
        0x8021_9D58 => battle::on_start(game),
        0x8021_A740 => last::on_init(game),
        0x8021_A7C4 => last::on_load(game),
        0x8021_A7C8 => last::on_start(game),
        super::gm::vs::ON_FRAME => game.vs_on_frame(),
        // db_RunEveryFrame: returns at once below the debug ROM's level.
        super::gm::vs::DB_RUN_EVERY_FRAME => game.game.assert_retail(),
        other => unported("function", other),
    }
}

/// Runs a callback a fighter holds that takes its GObj (`void (*)(HSD_GObj*)`): an action
/// state's animation, input, physics, collision and camera callbacks, accessories, hitlag
/// hooks and the character's own.
pub fn run_fighter_cb(g: &mut Running, gobj: GObjId, cb: CodeRef) {
    match cb.0 {
        // Animation.
        0x8008_A494 => g.co_wait_anim(gobj),
        0x8008_B8D4 | 0x8008_BA98 | 0x8008_C520 | 0x8009_99D8 => g.co_ground_move_anim(gobj),
        0x800B_FE6C => g.co_sleep_anim(gobj),
        0x8008_D010 => g.co_attack_air_anim(gobj),
        0x8008_D78C | 0x800D_5D3C => g.co_landing_anim(gobj),
        0x8008_F7F0 => g.co_damage_anim(gobj),
        0x8008_FD98 => g.co_damage_fly_anim(gobj),
        0x8009_9BD0 => g.co_escape_air_anim(gobj),
        0x8009_A8D8 => g.co_cliff_wait_anim(gobj),
        0x800C_9970 => g.co_turn_anim(gobj),
        0x800C_AC9C => g.co_run_brake_anim(gobj),
        0x800C_B2F8 => g.co_jump_anim(gobj),
        0x800C_B528 => g.co_knee_bend_anim(gobj),
        0x800C_C388 => g.co_jump_aerial_anim(gobj),
        0x800C_CA00 => g.co_fall_anim(gobj),
        0x800D_3E00 => g.co_dead_down_anim(gobj),
        // Physics.
        0x8008_A644 => g.co_wait_phys(gobj),
        0x800D_5F18 | 0x8008_D7B0 | 0x800C_B6AC | 0x8008_B940 | 0x8008_BB04 | 0x8008_BF84
        | 0x8008_CD28 | 0x800C_9BEC | 0x800E_8864 => g.co_ground_phys(gobj),
        0x800C_CD58 | 0x8009_093C | 0x8008_D5B4 | 0x800C_C634 => g.co_air_phys(gobj),
        0x8008_ADF0 | 0x8008_C7F0 => g.co_attack_ground_phys(gobj),
        0x8008_B600 => g.co_attack_dash_phys(gobj),
        0x8009_2870 | 0x8009_2B70 => g.co_guard_phys(gobj),
        0x8009_9CEC => g.co_escape_air_phys(gobj),
        0x800D_A3D0 => g.co_catch_wait_phys(gobj),
        0x800D_DA60 | 0x800D_DB90 | 0x800D_DD04 => g.co_throw_phys(gobj),
        0x800D_E790 => g.co_no_phys(gobj),
        0x800C_B474 => g.co_jump_phys(gobj),
        0x8008_FB18 => g.co_damage_phys(gobj),
        0x8008_FF84 => g.co_damage_fly_phys(gobj),
        // Collision.
        0x8008_A678 | 0x800D_5F38 => g.ground_coll_teeter(gobj),
        0x8008_AE10 | 0x8008_B638 | 0x8008_CD48 => g.ground_coll(gobj),
        // Hitlag.
        0x8008_E4F0 => g.co_damage_hitlag(gobj),
        other => unported("fighter callback", other),
    }
}

/// The original reaches a function no port covers yet: stopping beats diverging.
fn unported(what: &str, addr: u32) -> ! {
    panic!("{what} {addr:#010x} isn't ported")
}

/// What an original function takes or returns in its registers, for [`call`]: an integer, a
/// float as a register holds it, a GObj, or a vector passed or returned by pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Val {
    None,
    Int(i32),
    Float(f64),
    GObj(Option<GObjId>),
    Vec(crate::vec::Vec3),
    JObj(Option<crate::hsd::jobj::JObjId>),
    /// What an effect spawn returns: an effect, a generator, or null.
    Spawned(super::ef::Spawned),
}

impl Val {
    pub fn int(self) -> i32 {
        match self {
            Val::Int(v) => v,
            other => panic!("expected an integer, got {other:?}"),
        }
    }

    pub fn float(self) -> f64 {
        match self {
            Val::Float(v) => v,
            other => panic!("expected a float, got {other:?}"),
        }
    }

    pub fn gobj(self) -> Option<GObjId> {
        match self {
            Val::GObj(g) => g,
            other => panic!("expected a GObj, got {other:?}"),
        }
    }

    pub fn vec(self) -> crate::vec::Vec3 {
        match self {
            Val::Vec(v) => v,
            other => panic!("expected a vector, got {other:?}"),
        }
    }
}

/// Calls one of the original's functions by address: the seam where the match reaches code
/// ported elsewhere (fighters, items, effects, the HUD). Arguments come in the original's
/// order, an output vector's pointer standing for the vector it returns; until a subsystem's
/// port adds its arms here, reaching one stops the simulation.
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Val {
    #[cfg(feature = "stubs")]
    if let Some(v) = stubs::get(f) {
        stubs::log(f, args);
        return v;
    }
    if let Some(v) = super::ef::call(r, f, args) {
        return v;
    }
    unported("function", f)
}

/// Stand-ins for unported functions in checks (feature `stubs`): a stubbed function returns
/// its value instead of stopping, and the calls are logged with their arguments for the check
/// to compare with the original's. The simulation itself never registers any.
#[cfg(feature = "stubs")]
pub mod stubs {
    use super::Val;
    use std::cell::RefCell;

    thread_local! {
        static STUBS: RefCell<Vec<(u32, Val)>> = const { RefCell::new(Vec::new()) };
        static LOG: RefCell<Vec<(u32, Vec<Val>)>> = const { RefCell::new(Vec::new()) };
    }

    /// Makes `f` return `ret` on this thread.
    pub fn set(f: u32, ret: Val) {
        STUBS.with(|s| {
            let mut s = s.borrow_mut();
            s.retain(|&(a, _)| a != f);
            s.push((f, ret));
        });
    }

    pub fn clear() {
        STUBS.with(|s| s.borrow_mut().clear());
        LOG.with(|l| l.borrow_mut().clear());
    }

    pub fn get(f: u32) -> Option<Val> {
        STUBS.with(|s| s.borrow().iter().find(|&&(a, _)| a == f).map(|&(_, v)| v))
    }

    pub fn log(f: u32, args: &[Val]) {
        LOG.with(|l| l.borrow_mut().push((f, args.to_vec())));
    }

    /// The stubbed calls made since the last take.
    pub fn take() -> Vec<(u32, Vec<Val>)> {
        LOG.with(|l| std::mem::take(&mut *l.borrow_mut()))
    }
}

/// The game's particle callbacks, by token: efLib's hooks into the particle system (the
/// user functions `lbl_803BF810` and the new-generator hook) and its joint particle callbacks.
/// No game code sets particle user data or callbacks, so reaching one stops the simulation.
pub struct ParticleHooks;

impl crate::hsd::particle::Hooks for ParticleHooks {
    fn hook_create(&mut self, objs: &mut Objects, _: &Archives, f: UserFunc, p: ParticleId) {
        super::ef::eflib::hook_create(objs, f, p);
    }
    fn hook_delete(&mut self, _: &mut Objects, _: &Archives, f: UserFunc, _: ParticleId) {
        // `lbl_803BF810` has no delete hook.
        if f != super::ef::PARTICLE_RENDER_FUNCS {
            unported("particle hookDelete", f.0)
        }
    }
    fn set_user_data(
        &mut self,
        _: &mut Objects,
        _: &Archives,
        f: UserFunc,
        _: ParticleId,
        _: u8,
        _: f32,
    ) -> bool {
        // Nor a `setUserData`.
        if f != super::ef::PARTICLE_RENDER_FUNCS {
            unported("particle setUserData", f.0)
        }
        false
    }
    fn write_user_data(&mut self, _: &mut Objects, d: UserData, _: u8, _: f32) {
        unported("particle user data", d.0)
    }
    fn callback(&mut self, _: &mut Objects, _: &Archives, c: Callback, _: ParticleId) -> i32 {
        unported("particle callback", c.0)
    }
    fn generator_created(
        &mut self,
        objs: &mut Objects,
        data: &Archives,
        h: GeneratorHook,
        g: GeneratorId,
    ) {
        super::ef::eflib::generator_created(objs, data, h, g);
    }
    fn joint_particles(
        &mut self,
        objs: &mut Objects,
        data: &Archives,
        dynamic: bool,
        bank: i32,
        idx: i32,
        jobj: crate::hsd::jobj::JObjId,
    ) {
        if dynamic {
            super::ef::cb_dptcl(objs, data, self, bank, idx, jobj);
        } else {
            super::ef::cb_sptcl(objs, data, self, bank, idx, jobj);
        }
    }
}

/// The original calls a function no port covers yet.
pub fn unported_call(name: &str) -> ! {
    panic!("{name} isn't ported")
}
