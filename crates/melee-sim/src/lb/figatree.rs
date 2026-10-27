//! Fighter animations (`melee/lb/lbanim.c`): a figatree holds, per bone, a run of tracks
//! (`FigaTrack`), each an FObj's keyframe data, and binding one to a joint gives the joint an
//! AObj playing those tracks.

use melee_data::archive::{Archives, DataRef};

use crate::hsd::Objects;
use crate::hsd::aobj;
use crate::hsd::fobj::{FObj, FObjId};
use crate::hsd::jobj::{self, CLASSICAL_SCALE, JObjId};

/// A `FigaTrack`'s size.
pub const TRACK_SIZE: u32 = 0xC;

/// An FObj playing a track.
fn track_fobj(objs: &mut Objects, data: &Archives, track: DataRef) -> FObjId {
    let ad_head = data.ptr(track.at(8)).map_or(0, |r| r.offset);
    FObjId(
        objs.fobjs.alloc(FObj {
            archive: track.archive,
            startframe: data.u16(track.at(2)) as i16,
            obj_type: data.u8(track.at(4)),
            frac_value: data.u8(track.at(5)),
            frac_slope: data.u8(track.at(6)),
            ad_head,
            length: u32::from(data.u16(track)),
            flags: 0,
            data_at: crate::hsd::heap::with(|h| h.image.copy_at(track.archive))
                .flatten()
                .unwrap_or(0),
            ..FObj::default()
        }),
    )
}

/// FObjs for `frames` tracks from `track` on, `lbAnim_InitFrames`.
///
/// For no tracks (a negative count; the caller skips zero) the original returns a register
/// its caller left and writes through another. No figatree on the disc has a negative count;
/// should one appear, it counts as a stale read and gives no FObjs.
#[doc(alias = "lbAnim_InitFrames")]
fn init_frames(objs: &mut Objects, data: &Archives, track: DataRef, frames: i8) -> Option<FObjId> {
    if frames <= 0 {
        objs.stale_reads += 1;
        return None;
    }
    let mut first = None;
    let mut prev: Option<FObjId> = None;
    for i in 0..frames as u32 {
        let f = track_fobj(objs, data, track.at(i * TRACK_SIZE));
        match prev {
            Some(p) => objs.fobjs[p.0].next = Some(f),
            None => first = Some(f),
        }
        prev = Some(f);
    }
    first
}

/// FObjs for the tracks other than translation (types 5 to 7) among `frames` from `track`
/// on, `fn_8001E60C`. A translation track doesn't advance the cursor, so the tracks after it
/// aren't reached: the count only bounds the loop.
///
/// With no FObj made, the original ends the chain through a pointer it never set; that counts
/// as a stale read.
#[doc(alias = "fn_8001E60C")]
fn init_frames_no_translation(
    objs: &mut Objects,
    data: &Archives,
    mut track: DataRef,
    frames: i8,
) -> Option<FObjId> {
    let mut first = None;
    let mut prev: Option<FObjId> = None;
    for _ in 0..frames.max(0) {
        let ty = data.u8(track.at(4));
        if ty != 5 && ty.wrapping_sub(6) > 1 {
            let f = track_fobj(objs, data, track);
            match prev {
                Some(p) => objs.fobjs[p.0].next = Some(f),
                None => first = Some(f),
            }
            prev = Some(f);
            track = track.at(TRACK_SIZE);
        }
    }
    if prev.is_none() {
        objs.stale_reads += 1;
    }
    first
}

/// Binds `frames` tracks of a figatree to a joint, replacing its animation,
/// `lbAnim_8001E6D8`; [`bind_tracks_no_translation`] leaves translation out.
#[doc(alias = "lbAnim_8001E6D8")]
pub fn bind_tracks(
    objs: &mut Objects,
    data: &Archives,
    j: Option<JObjId>,
    tree: DataRef,
    track: DataRef,
    frames: i8,
) {
    bind(objs, data, j, tree, frames, |objs| {
        init_frames(objs, data, track, frames)
    });
}

/// `lbAnim_8001E7E8`.
#[doc(alias = "lbAnim_8001E7E8")]
pub fn bind_tracks_no_translation(
    objs: &mut Objects,
    data: &Archives,
    j: Option<JObjId>,
    tree: DataRef,
    track: DataRef,
    frames: i8,
) {
    bind(objs, data, j, tree, frames, |objs| {
        init_frames_no_translation(objs, data, track, frames)
    });
}

fn bind(
    objs: &mut Objects,
    data: &Archives,
    j: Option<JObjId>,
    tree: DataRef,
    frames: i8,
    fobjs: impl FnOnce(&mut Objects) -> Option<FObjId>,
) {
    let Some(j) = j else { return };
    if frames == 0 {
        return;
    }
    if let Some(old) = objs.jobjs[j].aobj {
        jobj::remove_aobj(objs, Some(old));
    }
    let a = Some(aobj::alloc(&mut objs.aobjs));
    aobj::set_flags(&mut objs.aobjs, a, data.u32(tree.at(4)));
    aobj::set_rewind_frame(&mut objs.aobjs, a, 0.0);
    aobj::set_end_frame(&mut objs.aobjs, a, data.f32(tree.at(8)));
    let f = fobjs(objs);
    aobj::set_fobj(&mut objs.aobjs, &mut objs.fobjs, a, f);
    objs.jobjs[j].aobj = a;
    jobj::sort_anim(objs, a);
    if data.u32(tree) & 1 != 0 {
        jobj::set_flags(&mut objs.jobjs, Some(j), CLASSICAL_SCALE);
    } else {
        jobj::clear_flags(&mut objs.jobjs, Some(j), CLASSICAL_SCALE);
    }
}
