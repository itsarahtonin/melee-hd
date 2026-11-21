//! The common grabs' Link parts (`ftCo_0D8E.c`): his and Young Link's tether grabs (the
//! hookshot's frames are the shared [`Running::catch_tether`]'s), and the hookshot put away.

use super::super::super::it::kinds::linkhookshot::HOOKSHOT;
use super::super::super::state::Running;
use super::super::samus::tether::{TetherGrab, TetherKind};
use crate::hsd::gobj::GObjId;

/// `ftCo_LinkCatchAttrs`' frames, in his attributes: the grab's hookshot out, shot, drawn in
/// and put away, the dash grab's and the air grab's likewise; the hookshot's item kind.
const FRAMES: [[usize; 4]; 3] = [
    [0x84, 0x88, 0x8C, 0x90],
    [0x94, 0x98, 0x9C, 0xA0],
    [0xA4, 0xA8, 0xAC, 0xB0],
];
const HOOKSHOT_KIND: usize = 0xBC;
/// The stage blocks his shot up to eight units past his thumb.
const REACH: f64 = 8.0;

/// Link's hookshot grabs, with his shot's and draw-in's sounds.
pub static LINK: TetherGrab = TetherGrab {
    item: &HOOKSHOT,
    kind: TetherKind::Attr(HOOKSHOT_KIND),
    frames: FRAMES,
    reach: REACH,
    sparks: [0, 0],
    sfx: Some([0x27149, 0x2714C]),
};

/// Young Link's, with his own sounds.
pub static CLINK: TetherGrab = TetherGrab {
    item: &HOOKSHOT,
    kind: TetherKind::Attr(HOOKSHOT_KIND),
    frames: FRAMES,
    reach: REACH,
    sparks: [0, 0],
    sfx: Some([0x111B9, 0x111BC]),
};

impl Running<'_> {
    /// `ftCo_800D94D8`: the hookshot put away (`it_802A2B10`) and forgotten.
    #[doc(alias = "ftCo_800D94D8")]
    pub fn link_hookshot_put_away(&mut self, gobj: GObjId) {
        self.tether_forget(gobj, &HOOKSHOT);
        self.link_boomerang_exists(gobj);
    }
}
