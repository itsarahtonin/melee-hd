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
        super::gr::grlib::QUAKE_LOOP_PROC => super::gr::grlib::quake_proc(game, gobj, true),
        super::gr::grlib::QUAKE_PROC => super::gr::grlib::quake_proc(game, gobj, false),
        super::playback::SEED_PROC => game.game.playback_restore_seed(),
        // The recording codes' processes send the frame over EXI and change nothing.
        super::playback::RECORD_FRAME_START_PROC | super::playback::RECORD_ITEMS_PROC => {}
        super::gm::vs::PROC => game.vs_proc(),
        crate::lb::bgflash::PROC => game.game.bgflash.proc(),
        crate::lb::sound_obj::PROC => game.sound_object_proc(gobj),
        crate::lb::bgflash::OVERLAY_PROC => game.bg_flash_proc(),
        // fn_801A4BD0: the scene GObj's first process, empty.
        super::gm::vs::SCENE_PROC => {}
        super::gm::pause::PROC => super::gm::pause::proc(game, gobj),
        addr if super::hud::run_proc(game, gobj, addr) => {}
        super::sfx::addr::PROC => game.crowd_proc(),
        addr if last::run(game, gobj, addr) => {}
        addr if super::gr::battle::run(game, gobj, addr) => {}
        addr if super::gr::stadium::run(game, gobj, addr) => {}
        addr if super::gr::story::run(game, gobj, addr) => {}
        addr if super::gr::oldpupupu::run(game, gobj, addr) => {}
        addr if super::gr::izumi::run(game, gobj, addr) => {}
        addr if super::gr::oldyoshi::run(game, gobj, addr) => {}
        addr if super::gr::kraid::run(game, gobj, addr) => {}
        addr if super::gr::fourside::run(game, gobj, addr) => {}
        addr if super::gr::greatbay::run(game, gobj, addr) => {}
        addr if super::gr::oldkongo::run(game, gobj, addr) => {}
        addr if super::gr::shrine::run(game, gobj, addr) => {}
        addr if super::gr::yorster::run(game, gobj, addr) => {}
        addr if super::gr::garden::run(game, gobj, addr) => {}
        addr if super::gr::kongo::run(game, gobj, addr) => {}
        addr if super::gr::greens::run(game, gobj, addr) => {}
        addr if super::gr::zebes::run(game, gobj, addr) => {}
        0x8006_A1BC => fighter::proc_hitlag(game, gobj),
        0x8006_A360 => fighter::proc_anim(game, gobj),
        0x8006_ABA0 => fighter::proc_cpu(game, gobj),
        0x8006_AD10 => game.proc_input(gobj),
        0x8006_B82C => fighter::proc_update(game, gobj),
        0x8006_C27C => fighter::proc_map(game, gobj),
        0x8006_C5F4 => fighter::proc_ik(game, gobj),
        0x8006_C624 => fighter::proc_accessory(game, gobj),
        0x8006_C80C => fighter::proc_coll_pos(game, gobj),
        0x8006_CA5C => fighter::proc_grab_coll(game, gobj),
        0x8006_CB94 => super::ft::attack::proc_attack_coll(game, gobj),
        0x8006_D1EC => super::ft::resolve::proc_coll_resolve(game, gobj),
        0x8006_D9AC => fighter::proc_dynamics(game, gobj),
        0x8006_DA4C => fighter::proc_player(game, gobj),
        0x8006_D9EC => super::ft::camera::proc_camera(game, gobj),
        0x8009_F480 => super::ft::create_model::light_proc(game, gobj),
        addr if super::it::item::run_proc(game, gobj, addr) => {}
        addr if super::it::kinds::run_event(game, gobj, addr) => {}
        super::ef::UPDATE_PROC => super::ef::update(game, gobj),
        super::ef::PARTICLES_PROC_MAIN => super::ef::particles_proc_main(game),
        super::ef::PARTICLES_PROC_AUX => super::ef::particles_proc_aux(game),
        other => unported("process", other),
    }
}

/// Runs a GObj's render callback (`GObj_RenderFunc`) for a render pass.
pub fn run_render(game: &mut Running, gobj: GObjId, cb: CodeRef, pass: i32) {
    if !super::render::run(game, gobj, cb, pass) {
        unported("render callback", cb.0)
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
        f if super::gr::stadium::run_event(game, f) => {}
        f if super::gr::story::run_event(game, f) => {}
        f if super::gr::oldpupupu::run_event(game, f) => {}
        f if super::gr::izumi::run_event(game, f) => {}
        f if super::gr::oldyoshi::run_event(game, f) => {}
        f if super::gr::kraid::run_event(game, f) => {}
        f if super::gr::fourside::run_event(game, f) => {}
        f if super::gr::greatbay::run_event(game, f) => {}
        f if super::gr::oldkongo::run_event(game, f) => {}
        f if super::gr::shrine::run_event(game, f) => {}
        f if super::gr::yorster::run_event(game, f) => {}
        f if super::gr::garden::run_event(game, f) => {}
        f if super::gr::kongo::run_event(game, f) => {}
        f if super::gr::greens::run_event(game, f) => {}
        f if super::gr::zebes::run_event(game, f) => {}
        super::gm::vs::ON_FRAME => game.vs_on_frame(),
        // db_RunEveryFrame: returns at once below the debug ROM's level.
        super::gm::vs::DB_RUN_EVERY_FRAME => game.game.assert_retail(),
        other => unported("function", other),
    }
}

/// Runs a stage's demo setup (`StageData::on_demo_init`), which takes the demo's number.
pub fn run_demo_init(game: &mut Running, f: u32, arg: i32) {
    use super::gr::{
        battle, fourside, garden, greatbay, greens, izumi, kongo, kraid, last, oldkongo, oldpupupu,
        oldyoshi, shrine, stadium, story, yorster, zebes,
    };
    match f {
        0x8021_A620 => last::on_demo_init(game, arg),
        0x8021_9C98 => battle::on_demo_init(game.game),
        0x801D_1018 => stadium::on_demo_init(game),
        0x801E_302C => story::on_demo_init(game),
        0x8021_07DC => oldpupupu::on_demo_init(game),
        0x801C_BB84 => izumi::on_demo_init(game),
        0x8020_E798 => oldyoshi::on_demo_init(game),
        0x801F_DFF8 => kraid::on_demo_init(game),
        0x801F_2D0C => fourside::on_demo_init(game),
        0x801F_423C => greatbay::on_demo_init(game),
        0x8020_F468 => oldkongo::on_demo_init(game),
        0x8020_1C60 => shrine::on_demo_init(game),
        0x8020_1FA0 => yorster::on_demo_init(game),
        0x8020_2B6C => garden::on_demo_init(game),
        0x801D_5238 => kongo::on_demo_init(game),
        0x8021_3458 => greens::on_demo_init(game),
        0x801D_84A0 => zebes::on_demo_init(game),
        other => unported("demo setup", other),
    }
}

/// `efLib_PauseAll`.
pub const EF_PAUSE_ALL: u32 = 0x8005_BA40;
/// `efLib_ResumeAll`.
pub const EF_RESUME_ALL: u32 = 0x8005_BAC4;

/// Runs a callback a fighter holds that takes its GObj (`void (*)(HSD_GObj*)`): an action
/// state's animation, input, physics, collision and camera callbacks, accessories, hitlag
/// hooks and the character's own.
pub fn run_fighter_cb(g: &mut Running, gobj: GObjId, cb: CodeRef) {
    match cb.0 {
        addr if super::ft::fox::run(g, gobj, addr) => {}
        addr if super::ft::seak::run(g, gobj, addr) => {}
        addr if super::ft::mars::run(g, gobj, addr) => {}
        addr if super::ft::zelda::run(g, gobj, addr) => {}
        addr if super::ft::ness::run(g, gobj, addr) => {}
        addr if super::ft::purin::run(g, gobj, addr) => {}
        addr if super::ft::samus::run(g, gobj, addr) => {}
        addr if super::ft::peach::run(g, gobj, addr) => {}
        addr if super::ft::link::run(g, gobj, addr) => {}
        addr if super::ft::clink::run(g, gobj, addr) => {}
        addr if super::ft::yoshi::run(g, gobj, addr) => {}
        addr if super::ft::mewtwo::run(g, gobj, addr) => {}
        addr if super::ft::koopa::run(g, gobj, addr) => {}
        addr if super::ft::gamewatch::run(g, gobj, addr) => {}
        addr if super::ft::kirby::run(g, gobj, addr) => {}
        super::it::kinds::samusgrapple::FT_DEATH => {
            super::it::kinds::samusgrapple::ft_death(g, gobj)
        }
        super::it::kinds::samusgrapple::FT_FRAME => {
            super::it::kinds::samusgrapple::ft_frame(g, gobj)
        }
        super::it::kinds::samusgrapple::FT_LINE_UP => {
            super::it::kinds::samusgrapple::ft_line_up(g, gobj)
        }
        super::it::kinds::linkhookshot::FT_DEATH => {
            super::it::kinds::linkhookshot::ft_death(g, gobj)
        }
        super::it::kinds::linkhookshot::FT_FRAME => {
            super::it::kinds::linkhookshot::ft_frame(g, gobj)
        }
        super::it::kinds::linkhookshot::FT_LINE_UP => {
            super::it::kinds::linkhookshot::ft_line_up(g, gobj)
        }
        addr if super::ft::falco::run(g, gobj, addr) => {}
        addr if super::ft::captain::run(g, gobj, addr) => {}
        addr if super::ft::luigi::run(g, gobj, addr) => {}
        addr if super::ft::mario::run(g, gobj, addr) => {}
        addr if super::ft::drmario::run(g, gobj, addr) => {}
        addr if super::ft::pikachu::run(g, gobj, addr) => {}
        addr if super::ft::donkey::run(g, gobj, addr) => {}
        addr if super::ft::co::cargo::run(g, gobj, addr) => {}
        addr if super::ft::co::hammer::run(g, gobj, addr) => {}
        addr if super::ft::co::kinoko::run(g, gobj, addr) => {}
        addr if super::ft::co::warpstar::run(g, gobj, addr) => {}
        addr if super::ft::co::bury::run(g, gobj, addr) => {}
        addr if super::ft::pichu::run(g, gobj, addr) => {}
        addr if super::ft::emblem::run(g, gobj, addr) => {}
        addr if super::ft::popo::run(g, gobj, addr) => {}
        addr if super::ft::co::dispatch::run(g, gobj, addr) => {}
        addr if super::ft::co::yoshi_egg::run(g, gobj, addr) => {}
        addr if super::ft::co::capture_mewtwo::run(g, gobj, addr) => {}
        addr if super::ft::co::damage_bind::run(g, gobj, addr) => {}
        addr if super::ft::co::capture_koopa::run(g, gobj, addr) => {}
        addr if super::ft::co::capture_kirby::run(g, gobj, addr) => {}
        addr if super::ft::co::item_get::run(g, gobj, addr) => {}
        addr if super::ft::co::lift::run(g, gobj, addr) => {}
        addr if super::ft::co::barrel::run(g, gobj, addr) => {}
        addr if super::ft::co::barrel_wait::run(g, gobj, addr) => {}
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
        0x800C_B2F8 | 0x800D_2DD0 => g.co_jump_anim(gobj),
        0x800C_B528 => g.co_knee_bend_anim(gobj),
        0x800C_C388 => g.co_jump_aerial_anim(gobj),
        0x800C_CA00 => g.co_fall_anim(gobj),
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
        // Camera.
        0x8007_61C8 => g.update_camera_box(gobj),
        // Hitlag.
        0x8008_E4F0 => g.co_damage_hitlag(gobj),
        0x8008_E714 => g.co_damage_exit_hitlag(gobj),
        // The throws'.
        0x800D_D8C4 => g.co_throw_f_anim(gobj),
        0x800D_DA10 | 0x800D_DB40 | 0x800D_DCB4 => g.co_throw_anim(gobj),
        0x800D_D930 => g.co_throw_phys(gobj),
        0x800D_D990 | 0x800D_DAC0 | 0x800D_DBF0 | 0x800D_DD64 => g.co_throw_coll(gobj),
        0x800D_DC70 => g.co_throw_hi_cam(gobj),
        0x800D_D92C | 0x800D_DA5C | 0x800D_DB8C | 0x800D_DD00 => g.co_throw_nothing(gobj),
        // Thrown.
        0x800D_E69C | 0x800D_E6C8 | 0x800D_E6F4 | 0x800D_E76C => g.co_thrown_anim(gobj),
        0x800D_E6BC | 0x800D_E6C0 | 0x800D_E6C4 | 0x800D_E6E8 | 0x800D_E6EC | 0x800D_E6F0
        | 0x800D_E714 | 0x800D_E718 | 0x800D_E71C | 0x800D_E78C | 0x800D_E794 => {
            g.co_throw_nothing(gobj)
        }
        0x800D_E720 => g.co_thrown_hi_cam(gobj),
        0x800D_E508 => g.thrown_follow(gobj),
        0x800D_E798 => g.thrown_on_change(gobj),
        // The grab's: fn_800D9CE8, the holds' hooks.
        0x800D_9CE8 => g.catch_landed(gobj),
        0x800D_A190 | 0x800D_A4A0 | 0x800D_A678 => g.catch_follow(gobj),
        0x800D_A490 | 0x800D_A668 => g.catch_on_damage(gobj),
        // ftCo_CatchPull_Anim, ftCo_CatchWait_Anim, ftCo_CatchAttack_Anim, ftCo_CatchCut_Anim
        0x800D_9E80 => g.co_catch_pull_anim(gobj),
        0x800D_A318 => g.co_grab_nothing(gobj),
        0x800D_A568 => g.co_catch_attack_anim(gobj),
        0x800D_A744 | 0x800D_C7F0 => g.co_catch_cut_anim(gobj),
        // The holds' interrupts.
        0x800D_9F90 | 0x800D_A5A4 | 0x800D_A780 | 0x800D_AC74 | 0x800D_B008 | 0x800D_C82C => {
            g.co_grab_nothing(gobj)
        }
        0x800D_A31C => g.co_catch_wait_iasa(gobj),
        0x800D_BA4C | 0x800D_BD30 => g.co_capture_wait_iasa(gobj),
        0x800D_C160 => g.co_capture_jump_iasa(gobj),
        // The holds' physics and collision.
        0x800D_9F94 | 0x800D_A5A8 => g.co_catch_hold_phys(gobj),
        0x800D_9FDC | 0x800D_A418 | 0x800D_A5F0 => g.co_catch_hold_coll(gobj),
        0x800D_A784 => g.co_catch_cut_phys(gobj),
        0x800D_A7E4 => g.co_catch_cut_coll(gobj),
        // The victim's.
        0x800D_AC70 | 0x800D_B004 => g.co_grab_nothing(gobj),
        0x800D_ADE8 | 0x800D_BA6C => g.co_capture_hi_phys(gobj),
        0x800D_AE94 => g.co_capture_pulled_hi_coll(gobj),
        0x800D_BA8C => g.co_capture_wait_hi_coll(gobj),
        0x800D_B00C => g.co_capture_pulled_lw_phys(gobj),
        0x800D_B1F8 => g.co_capture_pulled_lw_coll(gobj),
        0x800D_B908 | 0x800D_BD10 => g.co_capture_wait_anim(gobj),
        0x800D_BD50 => g.co_capture_wait_lw_phys(gobj),
        0x800D_BE9C => g.co_capture_wait_lw_coll(gobj),
        0x800D_C830 => g.co_capture_cut_phys(gobj),
        0x800D_C890 => g.co_capture_cut_coll(gobj),
        0x800D_C110 => g.co_capture_jump_anim(gobj),
        0x800D_C224 => g.co_capture_jump_phys(gobj),
        0x800D_C264 => g.co_capture_jump_coll(gobj),
        0x8008_AB50 | 0x8008_BC00 => g.attack_counts_anew(gobj),
        super::ft::phys::SPEED_TOWARD_FACING => g.speed_toward_facing(gobj),
        super::ft::phys::SPEED_AWAY_FROM_FACING => g.speed_away_from_facing(gobj),
        // Sheik's knockback hooks.
        super::ft::co::hit::SEAK_ON_KNOCKBACK_ENTER => g.seak_on_knockback_enter(gobj),
        super::ft::co::hit::SEAK_ON_KNOCKBACK_EXIT => g.seak_on_knockback_exit(gobj),
        // Zelda's, the same.
        super::ft::zelda::addr::ON_KNOCKBACK_ENTER => g.seak_on_knockback_enter(gobj),
        super::ft::zelda::addr::ON_KNOCKBACK_EXIT => g.seak_on_knockback_exit(gobj),
        // efLib_PauseAll, a state's hitlag hook that freezes the fighter's effects.
        EF_PAUSE_ALL => super::ef::pause_all(g.game, Some(gobj)),
        // efLib_ResumeAll, a state's hook for hitlag's end.
        EF_RESUME_ALL => super::ef::resume_all(g.game, Some(gobj)),
        other => unported("fighter callback", other),
    }
}

/// Runs a HUD callback (`IfStatusCb`: an announcement's start or end) by address.
pub fn run_hud_callback(r: &mut Running, f: CodeRef, arg: i32) {
    match f.0 {
        0x8016_B784 => r.vs_hud_on(arg),
        0x8016_B7B4 => r.vs_ready(arg),
        0x8016_B7F8 => r.vs_go(arg),
        0x8016_B88C => r.vs_game_end(arg),
        other => unported("HUD callback", other),
    }
}

/// The original reaches a function no port covers yet: stopping beats diverging.
fn unported(what: &str, addr: u32) -> ! {
    panic!("{what} {addr:#010x} isn't ported")
}

/// What an original function takes or returns in its registers, for [`call`]: an integer, a
/// float as a register holds it, a GObj, a fighter (a `Fighter*`), a joint, or a vector or
/// float passed or returned by pointer.
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub enum Val {
    None,
    Int(i32),
    Float(f64),
    GObj(Option<GObjId>),
    Fighter(super::handles::FighterId),
    Vec(crate::vec::Vec3),
    JObj(Option<crate::hsd::jobj::JObjId>),
    /// What an effect spawn returns: an effect, a generator, or null.
    Spawned(super::ef::Spawned),
    /// A float passed by pointer.
    F32Ref(f32),
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
    if let Some(v) = super::ft::fox::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::seak::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::mars::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::zelda::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::purin::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::samus::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::peach::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::link::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::clink::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::yoshi::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::mewtwo::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::koopa::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::gamewatch::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::kirby::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::ness::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::falco::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::captain::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::luigi::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::mario::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::drmario::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::pikachu::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::donkey::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::co::bury::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::pichu::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::emblem::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::ft::popo::call(r, f, args) {
        return v;
    }
    if let Some(v) = super::sfx::call(r, f, args) {
        return v;
    }
    if let Some(v) = held_item_call(r, f, args) {
        return v;
    }
    unported("function", f)
}

/// The held item's accessors and the hands' hold poses, which the characters' item hooks and
/// the common states reach by address.
fn held_item_call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    use super::it::lib26b1;
    let gobj = |i: usize| args[i].gobj().expect("a null GObj");
    Some(match f {
        // `ftAnim_80070FB4`, `ftAnim_80070C48`, `ftAnim_80070CC4`.
        0x8007_0FB4 => {
            r.set_part_anim_pose(gobj(0), args[1].int(), args[2].int());
            Val::None
        }
        0x8007_0C48 => {
            r.show_part_anim_pose(gobj(0), args[1].int());
            Val::None
        }
        0x8007_0CC4 => {
            r.end_part_anim(gobj(0), args[1].int());
            Val::None
        }
        // `itGetKind`, `it_8026B30C`, `itGetHoldKind`.
        0x8026_B300 => Val::Int(lib26b1::kind(r.game, gobj(0))),
        0x8026_B30C => Val::Int(lib26b1::use_kind(r.game, r.data, gobj(0))),
        0x8026_B320 => Val::Int(lib26b1::hold_kind(r.game, r.data, gobj(0))),
        _ => return None,
    })
}

/// A ported function stood in for in a check (feature `stubs`), as [`call`] stands in for an
/// unported one: its value if a stand-in is registered, the call logged with its arguments.
#[cfg(feature = "stubs")]
pub fn stubbed(f: u32, args: &[Val]) -> Option<Val> {
    let v = stubs::get(f)?;
    stubs::log(f, args);
    Some(v)
}

/// `OSReport`'s address.
pub const OS_REPORT: u32 = 0x8034_56A8;

/// `OSReport`: a line on the debugger's console, which the simulation leaves out. A check that
/// stands in for it (feature `stubs`) sees the call, as the original's.
#[doc(alias = "OSReport")]
pub fn os_report() {
    #[cfg(feature = "stubs")]
    let _ = stubbed(OS_REPORT, &[]);
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
