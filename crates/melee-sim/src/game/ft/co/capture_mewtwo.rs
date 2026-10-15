//! Confusion's victim (`ftCo_CaptureMewtwo.c`, `ftCo_ThrownMewtwo.c`): caught by the move's
//! grab, turned to face Mewtwo's way and held at him, then at once thrown (a state whose
//! callbacks do nothing) until his script's throw (`ftMt_SpecialS_Anim`) knocks it away.

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::motion::flags::FREEZE_STATE;
use super::ids::ms;
use crate::hsd::gobj::GObjId;

/// The thrown fighter's follow (`ftCo_800DE508`).
const THROWN_FOLLOW: u32 = 0x800D_E508;
/// The grabs' victim callbacks, on the ground and in the air.
pub(crate) const CAPTURE: u32 = 0x800B_CF18;
pub(crate) const CAPTURE_AIR: u32 = 0x800B_D000;

impl Running<'_> {
    /// Caught by Confusion, `ftCo_800BCF18` (on the ground) and `ftCo_800BD000` (in the air;
    /// the move's `grabbed_cb`): it lets go of what it held, faces Mewtwo's way, is held at
    /// him, is captured, and is thrown at once ([`enter_thrown_mewtwo`](Self::enter_thrown_mewtwo)).
    #[doc(alias = "ftCo_800BCF18")]
    #[doc(alias = "ftCo_800BD000")]
    pub fn enter_capture_mewtwo(&mut self, gobj: GObjId, mewtwo: GObjId, air: bool) {
        let (msid, thrown) = if air {
            (ms::CAPTURE_MEWTWO_AIR, ms::THROWN_MEWTWO_AIR)
        } else {
            (ms::CAPTURE_MEWTWO, ms::THROWN_MEWTWO)
        };
        self.damage_hooks(gobj);
        self.drop_heavy_item(gobj);
        self.release_victim(gobj);
        let facing = self.game.fighter(mewtwo).facing_dir;
        let fp = self.game.fighter_mut(gobj);
        fp.x1A5C = Some(mewtwo);
        fp.victim_gobj = Some(mewtwo);
        fp.x221B.x221B_b5 = false;
        fp.x221B.x221B_b7 = false;
        fp.facing_dir = facing;
        self.attach_to_thrower(mewtwo, gobj);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, Some(mewtwo));
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        self.game.fighters[f.0].x1A6A = 0x1FF;
        self.anim_update(gobj);
        self.stop_dead(gobj);
        self.enter_thrown_mewtwo(gobj, thrown);
    }

    /// Thrown by Confusion, `ftCo_800BD0E8`: facing Mewtwo's way, frozen with him if he's
    /// frozen (`x2222_b6`), following its throw point.
    #[doc(alias = "ftCo_800BD0E8")]
    pub fn enter_thrown_mewtwo(&mut self, gobj: GObjId, msid: i32) {
        let thrower = self
            .game
            .fighter(gobj)
            .victim_gobj
            .expect("thrown by no one");
        let facing = self.game.fighter(thrower).facing_dir;
        let fp = self.game.fighter_mut(gobj);
        fp.facing_dir = facing;
        fp.set_mv_i32(0, 0);
        let flags = if fp.x2222_b6 { FREEZE_STATE } else { 0 };
        self.change_motion_state(gobj, msid, flags, 0.0, 1.0, 0.0, Some(thrower));
        let fp = self.game.fighter_mut(gobj);
        fp.accessory1_cb = Some(CodeRef(THROWN_FOLLOW));
        fp.x1A6A = 0x1FF;
        self.anim_update(gobj);
    }

    /// `ftCo_ThrownMewtwo_Anim` and the thrown states' other callbacks: nothing.
    #[doc(alias = "ftCo_ThrownMewtwo_Anim")]
    #[doc(alias = "ftCo_ThrownMewtwo_IASA")]
    #[doc(alias = "ftCo_ThrownMewtwo_Phys")]
    #[doc(alias = "ftCo_ThrownMewtwo_Coll")]
    #[doc(alias = "ftCo_ThrownMewtwoAir_Anim")]
    #[doc(alias = "ftCo_ThrownMewtwoAir_IASA")]
    #[doc(alias = "ftCo_ThrownMewtwoAir_Phys")]
    #[doc(alias = "ftCo_ThrownMewtwoAir_Coll")]
    pub fn co_thrown_mewtwo_nothing(&mut self, _gobj: GObjId) {}
}

/// Runs one of the victim's callbacks, if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x800B_D17C..=0x800B_D198 if addr.is_multiple_of(4) => g.co_thrown_mewtwo_nothing(gobj),
        _ => return false,
    }
    true
}
