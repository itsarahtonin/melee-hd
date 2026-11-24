//! The characters' hooks' shared bodies: the item hooks (`fighter.c`'s inlines
//! `Fighter_OnItemPickup`, `Fighter_OnItemInvisible`, `Fighter_OnItemVisible` and
//! `Fighter_OnItemDrop`, and the characters' own copies of them with their own hold poses),
//! the knockback hooks' textures (`Fighter_OnKnockbackEnter` and `_Exit`), and creation's item
//! articles and multi-jump attributes. Each character's hook keeps its own alias and passes its
//! parts and poses.

use super::super::code::Val;
use super::super::handles::FighterId;
use super::super::state::Running;
use super::fox::ext;
use crate::hsd::gobj::GObjId;

/// The hold pose a hand takes for each of `itGetHoldKind`'s kinds (0 to 4), or none.
pub type HoldPoses = [Option<i32>; 5];

/// `Fighter_OnItemPickup`'s poses: kinds 1 to 4 take poses 1, 0, 2 and 3.
pub const HOLD_POSES: HoldPoses = [None, Some(1), Some(0), Some(2), Some(3)];

/// What a hook showing or hiding the held item does with none held.
#[derive(Clone, Copy)]
pub enum NoItem {
    /// Stops: the original reads the item regardless.
    Stops,
    /// Nothing (Jigglypuff's hooks, and Kirby's showing it, as their ports test for one).
    Nothing,
}

impl Running<'_> {
    /// The held item, if there is one; with none, `none` says whether that stops.
    fn held_item(&self, gobj: GObjId, none: NoItem, what: &str) -> Option<GObjId> {
        let item = self.game.fighter(gobj).item_gobj;
        if item.is_none() && matches!(none, NoItem::Stops) {
            panic!("{what} no item");
        }
        item
    }

    /// Picking up an item (`Fighter_OnItemPickup(gobj, flag, pose_part, show_part)`, or a
    /// character's own copy with its `poses`): part `pose_part` takes the item's hold pose,
    /// unless the item is heavy, and part `show_part` shows it when `flag` says so.
    pub(crate) fn item_pickup(
        &mut self,
        gobj: GObjId,
        flag: bool,
        poses: &HoldPoses,
        pose_part: i32,
        show_part: i32,
    ) {
        let item = self
            .game
            .fighter(gobj)
            .item_gobj
            .expect("picking up no item");
        if super::super::it::lib26b1::is_heavy(self.game, self.data, item) {
            return;
        }
        let kind = self
            .ext(ext::IT_GET_HOLD_KIND, &[Val::GObj(Some(item))])
            .int();
        let pose = usize::try_from(kind)
            .ok()
            .and_then(|k| poses.get(k).copied().flatten());
        if let Some(p) = pose {
            self.ext(
                ext::FTANIM_SET_PART_ANIM_POSE,
                &[Val::GObj(Some(gobj)), Val::Int(pose_part), Val::Int(p)],
            );
        }
        if flag {
            self.ext(
                ext::FTANIM_SHOW_PART_ANIM_POSE,
                &[Val::GObj(Some(gobj)), Val::Int(show_part)],
            );
        }
    }

    /// `Fighter_OnItemPickup(gobj, flag, hand, hand)`: hand `hand` takes the item's hold
    /// pose, unless it is heavy, and closes on it when `flag` says so.
    pub(crate) fn item_pickup_hand(&mut self, gobj: GObjId, flag: bool, hand: i32) {
        self.item_pickup(gobj, flag, &HOLD_POSES, hand, hand);
    }

    /// Hiding the held item (`Fighter_OnItemInvisible(gobj, part)`): part `part` lets go of
    /// its pose, unless the item is heavy.
    pub(crate) fn hide_held_item(&mut self, gobj: GObjId, part: i32, none: NoItem) {
        let Some(item) = self.held_item(gobj, none, "hiding") else {
            return;
        };
        if !super::super::it::lib26b1::is_heavy(self.game, self.data, item) {
            self.ext(
                ext::FTANIM_END_PART_ANIM,
                &[Val::GObj(Some(gobj)), Val::Int(part)],
            );
        }
    }

    /// `Fighter_OnItemInvisible(gobj, hand)`.
    pub(crate) fn item_invisible_hand(&mut self, gobj: GObjId, hand: i32) {
        self.hide_held_item(gobj, hand, NoItem::Stops);
    }

    /// Showing the held item again (`Fighter_OnItemVisible(gobj, part)`): part `part` shows
    /// its pose, unless the item is heavy.
    pub(crate) fn show_held_item(&mut self, gobj: GObjId, part: i32, none: NoItem) {
        let Some(item) = self.held_item(gobj, none, "showing") else {
            return;
        };
        if !super::super::it::lib26b1::is_heavy(self.game, self.data, item) {
            self.ext(
                ext::FTANIM_SHOW_PART_ANIM_POSE,
                &[Val::GObj(Some(gobj)), Val::Int(part)],
            );
        }
    }

    /// `Fighter_OnItemVisible(gobj, hand)`.
    pub(crate) fn item_visible_hand(&mut self, gobj: GObjId, hand: i32) {
        self.show_held_item(gobj, hand, NoItem::Stops);
    }

    /// Dropping the item (`Fighter_OnItemDrop(gobj, flag, pose_part, end_part)`): no hold
    /// pose on part `pose_part`, and part `end_part` lets go of it when `flag` says so.
    pub(crate) fn item_drop(&mut self, gobj: GObjId, flag: bool, pose_part: i32, end_part: i32) {
        self.ext(
            ext::FTANIM_SET_PART_ANIM_POSE,
            &[Val::GObj(Some(gobj)), Val::Int(pose_part), Val::Int(-1)],
        );
        if flag {
            self.ext(
                ext::FTANIM_END_PART_ANIM,
                &[Val::GObj(Some(gobj)), Val::Int(end_part)],
            );
        }
    }

    /// `Fighter_OnItemDrop(gobj, flag, hand, hand)`.
    pub(crate) fn item_drop_hand(&mut self, gobj: GObjId, flag: bool, hand: i32) {
        self.item_drop(gobj, flag, hand, hand);
    }

    /// The hurt textures in knockback, or back after it (`Fighter_OnKnockbackEnter(gobj,
    /// part)` at frame 3, `Fighter_OnKnockbackExit` at 0): costume texture `part`, then the
    /// first.
    pub(crate) fn knockback_textures(&mut self, gobj: GObjId, part: i32, frame: f32) {
        self.texture_anim(gobj, part, frame);
        self.texture_anim(gobj, 0, frame);
    }

    /// The character's items registered with their articles (`ftData::x48_items`, in order).
    pub(crate) fn register_items(&mut self, f: FighterId, kinds: &[i32]) {
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        for (i, &kind) in kinds.iter().enumerate() {
            let article = self.data.ptr(items.at(4 * i as u32));
            super::super::it::item::set_char_article(self.game, article, kind);
        }
    }

    /// Multi-jumps, with its attributes as its multi-jump attributes (`x2D0`).
    pub(crate) fn multi_jump_from_attrs(&mut self, f: FighterId) {
        let fp = &mut self.game.fighters[f.0];
        fp.can_multijump = true;
        fp.multi_jump_attrs = fp.dat_attrs;
    }
}
