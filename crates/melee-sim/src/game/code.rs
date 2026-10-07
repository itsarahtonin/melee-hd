//! Dispatch of the original's function pointers. A [`CodeRef`] is the original function's
//! address; each table here maps the addresses a kind of pointer can hold to their ports.

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
