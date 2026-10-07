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
        other => unported("process", other),
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

/// The game's particle callbacks (efLib's and the stages'), by token. None is ported yet, so a
/// particle that reaches one stops the simulation.
pub struct ParticleHooks;

impl crate::hsd::particle::Hooks for ParticleHooks {
    fn hook_create(&mut self, _: &mut Objects, _: &Archives, f: UserFunc, _: ParticleId) {
        unported("particle hookCreate", f.0)
    }
    fn hook_delete(&mut self, _: &mut Objects, _: &Archives, f: UserFunc, _: ParticleId) {
        unported("particle hookDelete", f.0)
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
        unported("particle setUserData", f.0)
    }
    fn write_user_data(&mut self, _: &mut Objects, d: UserData, _: u8, _: f32) {
        unported("particle user data", d.0)
    }
    fn callback(&mut self, _: &mut Objects, _: &Archives, c: Callback, _: ParticleId) -> i32 {
        unported("particle callback", c.0)
    }
    fn generator_created(
        &mut self,
        _: &mut Objects,
        _: &Archives,
        h: GeneratorHook,
        _: GeneratorId,
    ) {
        unported("generator hook", h.0)
    }
}

/// The original calls a function no port covers yet.
pub fn unported_call(name: &str) -> ! {
    panic!("{name} isn't ported")
}
