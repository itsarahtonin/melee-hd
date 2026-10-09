//! melee-sim's TObj (texture objects) against ssbm-rs's port, function by function: the GX
//! index conversions on every valid input, texture matrices (`MakeTextureMtx`, and
//! `TObjSetupMtx`'s dirty check) on random transforms in both float modes, the current-texture
//! lookup, and the TObj and MObj classes' amnesia. `mobj.rs` checks loading, animation and the
//! rest on the disc's materials.

#[path = "support/display.rs"]
mod display;

use leaf_check::fp::{FpMode, set_fp_mode};
use leaf_check::{Floats, MODES, Rand, handle, hsd_reference, reference, write_f32s};
use melee_sim::hsd::mobj::{self, MObjs};
use melee_sim::hsd::tobj::{self, TObjId, TObjs};
use melee_sim::quat::Quat;
use melee_sim::vec::Vec3;
use ssbm_rt::Handle;
use ssbm_types::fns;
use ssbm_types::records::{_HSD_ClassInfo, HSD_TObj};
use ssbm_types::tu::sysdolphin__baselib__mobj as mobj_tu;
use ssbm_types::tu::sysdolphin__baselib__tobj as tobj_tu;

#[test]
fn index_conversions_match() {
    let Some(ctx) = reference() else { return };
    let ctx = &ctx;
    for i in 0..8u32 {
        assert_eq!(
            tobj::tg_tex_to_index(4 + i),
            fns::HSD_TGTex2Index(ctx, 4 + i as i32)
        );
        assert_eq!(
            tobj::texcoord_to_index(i),
            fns::HSD_TexCoord2Index(ctx, i as i32)
        );
        assert_eq!(
            tobj::index_to_texcoord(i),
            fns::HSD_Index2TexCoord(ctx, i) as u32
        );
        assert_eq!(
            tobj::index_to_texmap(i),
            fns::HSD_Index2TexMap(ctx, i) as u32
        );
        assert_eq!(
            tobj::texmap_to_index(i),
            fns::HSD_TexMap2Index(ctx, i as i32)
        );
        assert_eq!(
            tobj::texmap_pt_texmtx(i),
            tobj_tu::HSD_TexMapID2PTTexMtx(ctx, i as i32)
        );
        // The last coordinate has no generation source (the original asserts).
        if i < 7 {
            assert_eq!(
                tobj::texcoord_to_texgen_src(i),
                fns::HSD_TexCoordID2TexGenSrc(ctx, i as i32) as u32
            );
        }
    }
    for i in 0..=10u32 {
        let m = fns::HSD_Index2TexMtx(ctx, i) as u32;
        assert_eq!(tobj::index_to_texmtx(i), m, "index {i}");
        assert_eq!(
            tobj::texmtx_to_index(m),
            fns::HSD_TexMtx2Index(ctx, m as i32)
        );
        assert_eq!(tobj::texmtx_to_index(m), i);
    }
}

/// A TObj's transform fields, written to both sides.
struct Transform {
    rotate: [f32; 4],
    scale: [f32; 3],
    translate: [f32; 3],
    repeat: [u8; 2],
    wrap_t: u32,
    flags: u32,
}

impl Transform {
    fn random(f: &mut Floats, r: &mut Rand) -> Transform {
        let repeat = |r: &mut Rand| match r.below(4) {
            0 => 1,
            1 => 255,
            _ => 1 + r.below(255) as u8,
        };
        // Every coordinate mode the setup's dirty check can meet, except shadows, whose GX
        // matrix needs a current camera.
        let coord = [0, 1, 2, 4][r.below(4) as usize];
        // Scales around the threshold below which a texture's scale counts as zero.
        let mut scale = |r: &mut Rand| {
            if r.below(8) == 0 {
                let near = 0x2EDB_E6FFu32
                    .wrapping_add(r.below(0x200) as u32)
                    .wrapping_sub(0x100);
                f32::from_bits(near | if r.below(2) == 0 { 0x8000_0000 } else { 0 })
            } else {
                f.next()
            }
        };
        let scale = [scale(r), scale(r), f.next()];
        Transform {
            rotate: [f.next(), f.next(), f.next(), f.next()],
            scale,
            translate: [f.next(), f.next(), f.next()],
            repeat: [repeat(r), repeat(r)],
            wrap_t: r.below(4) as u32,
            flags: coord
                | if r.below(4) == 0 { tobj::BUMP } else { 0 }
                | if r.below(3) != 0 { tobj::MTX_DIRTY } else { 0 },
        }
    }

    fn put_theirs(&self, ctx: &ssbm_rt::Ctx, t: u32) {
        write_f32s(ctx, t + 0x18, &self.rotate);
        write_f32s(ctx, t + 0x28, &self.scale);
        write_f32s(ctx, t + 0x34, &self.translate);
        ctx.write_u32(t + 0x44, self.wrap_t);
        ctx.write_u8(t + 0x48, self.repeat[0]);
        ctx.write_u8(t + 0x49, self.repeat[1]);
        ctx.write_u32(t + 0x4C, self.flags);
    }

    fn put_ours(&self, tobjs: &mut TObjs, t: TObjId) {
        let o = &mut tobjs[t];
        let [x, y, z, w] = self.rotate;
        o.rotate = Quat::new(x, y, z, w);
        o.scale = Vec3::new(self.scale[0], self.scale[1], self.scale[2]);
        o.translate = Vec3::new(self.translate[0], self.translate[1], self.translate[2]);
        o.wrap_t = self.wrap_t;
        o.repeat_s = self.repeat[0];
        o.repeat_t = self.repeat[1];
        o.flags = self.flags;
    }
}

#[test]
fn texture_matrices_match() {
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    display::enable_gx(ctx);
    display::readable_panics(ctx);
    // A TObj of HAL's class, whose methods TObjSetupMtx calls.
    let t = fns::HSD_TObjAlloc(ctx).addr();
    let mut rand = Rand(0x5DEE_CE66_D1CE_4E5B);
    let mut counts = [0u64; 2];
    for mode in MODES {
        set_fp_mode(mode);
        let mut floats = Floats::new(8.0);
        let mut tobjs = TObjs::default();
        let o = tobj::alloc(&mut tobjs);
        for n in 0..40_000 {
            let tf = Transform::random(&mut floats, &mut rand);
            tf.put_theirs(ctx, t);
            tf.put_ours(&mut tobjs, o);
            let setup = n % 2 == 1;
            if setup {
                tobj_tu::TObjSetupMtx(ctx, handle(ctx, t));
                tobj::setup_mtx(&mut tobjs, o);
            } else {
                tobj_tu::MakeTextureMtx(ctx, handle(ctx, t));
                tobj::make_mtx(&mut tobjs, o);
            }
            counts[usize::from(setup)] += 1;
            let theirs: Vec<u32> = (0..12).map(|i| ctx.read_u32(t + 0x74 + 4 * i)).collect();
            let ours: Vec<u32> = tobjs[o]
                .mtx
                .m
                .as_flattened()
                .iter()
                .map(|v| v.to_bits())
                .collect();
            assert_eq!(ours, theirs, "case {n} in {mode:?} (setup {setup})");
            assert_eq!(
                tobjs[o].flags,
                ctx.read_u32(t + 0x4C),
                "case {n} flags in {mode:?}"
            );
        }
    }
    set_fp_mode(FpMode::Hardware);
    eprintln!("{} matrices made, {} setups", counts[0], counts[1]);
}

/// Random chains of TObjs with every coordinate mode, bump and toon textures (which no
/// material on the disc has), and stale maps, matrices and coordinates.
#[test]
fn assign_resources_match() {
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    display::readable_panics(ctx);
    let mut rand = Rand(0x0DDB_A11C_0FFE_E123);
    let mut tobjs = TObjs::default();
    for round in 0..5000 {
        let n = 1 + rand.below(12) as usize;
        let theirs: Vec<u32> = (0..n).map(|_| fns::HSD_TObjAlloc(ctx).addr()).collect();
        let ours: Vec<TObjId> = (0..n).map(|_| tobj::alloc(&mut tobjs)).collect();
        for i in 0..n {
            let mut flags = rand.below(6) as u32 | (rand.bits() as u32 & 0x7FFF_FFF0);
            if rand.below(3) != 0 {
                flags &= !tobj::BUMP;
            }
            let (id, mtxid, coord) = (
                rand.below(9) as u32,
                rand.below(90) as u32,
                rand.below(8) as u32,
            );
            let t = theirs[i];
            ctx.write_u32(t + 0x4C, flags);
            ctx.write_u32(t + 0xC, id);
            ctx.write_u32(t + 0x14, mtxid);
            ctx.write_u32(t + 0xA4, coord);
            ctx.write_u32(t + 8, theirs.get(i + 1).copied().unwrap_or(0));
            let o = &mut tobjs[ours[i]];
            (o.flags, o.id, o.mtxid, o.coord) = (flags, id, mtxid, coord);
            o.next = ours.get(i + 1).copied();
        }
        let tc = fns::HSD_TObjAssignResources(ctx, handle(ctx, theirs[0]));
        let oc = tobj::assign_resources(&mut tobjs, Some(ours[0]));
        assert_eq!(tc as u32, oc, "round {round}: coordinates used");
        for i in 0..n {
            let t = theirs[i];
            let o = &tobjs[ours[i]];
            assert_eq!(
                (o.id, o.mtxid, o.coord),
                (
                    ctx.read_u32(t + 0xC),
                    ctx.read_u32(t + 0x14),
                    ctx.read_u32(t + 0xA4)
                ),
                "round {round}: TObj {i} of {n}"
            );
        }
        assert_eq!(fns::HSD_TObjAssignResources(ctx, handle(ctx, 0)), 0);
        assert_eq!(tobj::assign_resources(&mut tobjs, None), 0);
        for (t, o) in theirs.into_iter().zip(ours) {
            fns::HSD_TObjFree(ctx, handle::<HSD_TObj>(ctx, t));
            tobj::free(&mut tobjs, Some(o));
        }
    }
}

#[test]
fn current_by_type_and_amnesia_match() {
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    let mut rand = Rand(0x1234_5678_9ABC_DEF1);
    let mut tobjs = TObjs::default();
    let mut mobjs = MObjs::default();
    // Amnesia calls the parent class's, which class initialization links: as in the game, an
    // MObj has been made by then.
    fns::HSD_MObjRemove(ctx, fns::HSD_MObjAlloc(ctx));
    for round in 0..200 {
        let n = 1 + rand.below(8) as usize;
        let theirs: Vec<u32> = (0..n).map(|_| fns::HSD_TObjAlloc(ctx).addr()).collect();
        let ours: Vec<TObjId> = (0..n).map(|_| tobj::alloc(&mut tobjs)).collect();
        for i in 0..n {
            let flags = rand.below(6) as u32 | (rand.bits() as u32 & 0xFFFF_FFF0);
            ctx.write_u32(theirs[i] + 0x4C, flags);
            tobjs[ours[i]].flags = flags;
            let next = theirs.get(i + 1).copied().unwrap_or(0);
            ctx.write_u32(theirs[i] + 8, next);
            tobjs[ours[i]].next = ours.get(i + 1).copied();
        }
        let head = rand.below(n as u64 + 1) as usize;
        fns::tobj_head(ctx).set(handle(ctx, theirs.get(head).copied().unwrap_or(0)));
        tobjs.head = ours.get(head).copied();
        let which = |t: u32| theirs.iter().position(|&x| x == t);
        for from in 0..=n {
            for mapping in 0..6 {
                let t = fns::_HSD_TObjGetCurrentByType(
                    ctx,
                    handle(ctx, theirs.get(from).copied().unwrap_or(0)),
                    mapping,
                )
                .addr();
                let o = tobj::current_by_type(&tobjs, ours.get(from).copied(), mapping);
                assert_eq!(
                    o.and_then(|o| ours.iter().position(|&x| x == o)),
                    which(t),
                    "round {round} from {from} mapping {mapping}"
                );
            }
        }
        // Forgetting another class leaves tobj_head; forgetting TObj's clears it.
        let info = |a: u32| handle::<_HSD_ClassInfo>(ctx, a);
        let (tinfo, minfo) = (fns::hsdTObj(ctx).addr(), fns::hsdMObj(ctx).addr());
        tobj_tu::TObjAmnesia(ctx, info(minfo));
        assert_eq!(
            fns::tobj_head(ctx).get().addr(),
            theirs.get(head).copied().unwrap_or(0)
        );
        tobj_tu::TObjAmnesia(ctx, info(tinfo));
        tobj::amnesia(&mut tobjs);
        assert_eq!(fns::tobj_head(ctx).get().addr(), 0);
        assert_eq!(tobjs.head, None);
        // MObj's forgets the toon and shadow textures.
        fns::tobj_toon(ctx).set(handle(ctx, theirs[0]));
        fns::tobj_shadows(ctx).set(handle(ctx, theirs[n - 1]));
        mobjs.toon = Some(ours[0]);
        mobjs.shadows = Some(ours[n - 1]);
        mobj_tu::MObjAmnesia(ctx, info(tinfo));
        assert_ne!(fns::tobj_toon(ctx).get().addr(), 0);
        mobj_tu::MObjAmnesia(ctx, info(minfo));
        mobj::amnesia(&mut mobjs);
        assert_eq!(
            (
                fns::tobj_toon(ctx).get().addr(),
                fns::tobj_shadows(ctx).get().addr()
            ),
            (0, 0)
        );
        assert_eq!((mobjs.toon, mobjs.shadows), (None, None));
        for (t, o) in theirs.into_iter().zip(ours) {
            fns::HSD_TObjFree(ctx, handle::<HSD_TObj>(ctx, t));
            tobj::free(&mut tobjs, Some(o));
        }
    }
}
