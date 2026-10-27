//! The sound objects' behaviors that work on the object alone against ssbm-rs's ports (the
//! pan swept once, straight and mirrored, and the volume faded: `fn_800259EC`, `fn_80025A98`,
//! `fn_80025B44`, `fn_80025CBC`, `fn_80025E38`), on random objects and the edges of their
//! frames, each object a user data block behind a GObj in scratch memory.

use leaf_check::{Rand, SCRATCH, handle, reference};
use melee_sim::lb::sound_obj::{SoundObj, behavior_alone};
use ssbm_types::tu::melee__lb__lbaudio_ax as fns;

/// The GObj, and its user data (`lbAudioAx_UserData`) after it.
const GOBJ: u32 = SCRATCH;
const UD: u32 = SCRATCH + 0x100;

/// Writes `o` as the original lays it out, under a GObj whose user data it is.
fn put(ctx: &ssbm_rt::Ctx, o: &SoundObj) {
    for i in 0..0x40 {
        ctx.write_u32(GOBJ + 4 * i, 0);
    }
    ctx.write_u32(GOBJ + 0x2C, UD);
    let words = [
        (0x18, o.start_vol as u32),
        (0x1C, o.end_vol as u32),
        (0x20, o.vol as u32),
        (0x24, o.pan_left as u32),
        (0x28, o.pan_right as u32),
        (0x2C, o.pan as u32),
        (0x30, o.voice_id as u32),
        (0x34, o.current_frame as u32),
        (0x38, o.end_frame as u32),
        (0x3C, o.direction.to_bits()),
        (0x40, o.track as u32),
        (0x44, u32::from(o.x44)),
    ];
    for (off, w) in words {
        ctx.write_u32(UD + off, w);
    }
}

/// The volume, the pan and whether the pan was set, as the port left them.
fn got(ctx: &ssbm_rt::Ctx) -> (i32, i32, bool) {
    (
        ctx.read_u32(UD + 0x20) as i32,
        ctx.read_u32(UD + 0x2C) as i32,
        ctx.read_u32(UD + 0x44) != 0,
    )
}

fn random(r: &mut Rand) -> SoundObj {
    let small = |r: &mut Rand| r.below(0x90) as i32 - 8;
    let frame = |r: &mut Rand| match r.below(4) {
        0 => 0,
        1 => r.below(4) as i32 - 1,
        _ => r.below(200) as i32,
    };
    SoundObj {
        start_vol: small(r),
        end_vol: small(r),
        vol: 0x7F,
        pan_left: small(r),
        pan_right: small(r),
        pan: 0x40,
        voice_id: -1,
        current_frame: frame(r),
        end_frame: frame(r),
        direction: [1.0, -1.0, 0.5][r.below(3) as usize],
        x44: r.below(2) == 0,
        ..SoundObj::default()
    }
}

#[test]
fn sound_object_behaviors_match() {
    let Some(ctx) = reference() else { return };
    let mut r = Rand(0x50B1_EC75);
    type Port = for<'a> fn(&'a ssbm_rt::Ctx, ssbm_types::records::HSD_GObj<'a>) -> i32;
    let behaviors: [(u32, Port); 5] = [
        (0x8002_59EC, fns::fn_800259EC),
        (0x8002_5A98, fns::fn_80025A98),
        (0x8002_5B44, fns::fn_80025B44),
        (0x8002_5CBC, fns::fn_80025CBC),
        (0x8002_5E38, fns::fn_80025E38),
    ];
    for n in 0..4000 {
        let o = random(&mut r);
        for &(addr, theirs) in &behaviors {
            put(&ctx, &o);
            theirs(&ctx, handle(&ctx, GOBJ));
            let mut ours = o.clone();
            assert!(behavior_alone(&mut ours, addr));
            assert_eq!(
                (ours.vol, ours.pan, ours.x44),
                got(&ctx),
                "{addr:#x} on object {n}: {o:?}"
            );
        }
    }
}
