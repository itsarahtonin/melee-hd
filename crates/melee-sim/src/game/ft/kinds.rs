//! Each character's own callbacks the fighter code calls by kind (`ft/kinds/*`): creation's
//! (`OnLoad`), a spawn's (`OnDeath`) and the attribute changes' (`LoadSpecialAttrs`). Fox's
//! and Marth's are ported.

use super::super::handles::{AttrsId, FighterId};
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `ftFox_DatAttrs` and `MarsAttributes`: how many bytes of a character's attributes its
/// callbacks copy.
const FOX_ATTRS_SIZE: usize = 0xD4;
const MARS_ATTRS_SIZE: usize = 0x98;

/// `It_Kind_Fox_Illusion`.
const IT_KIND_FOX_ILLUSION: i32 = 56;

impl Running<'_> {
    /// The character's attributes (`ftData::ext_attr`), `size` bytes of them.
    fn ext_attrs(&self, f: FighterId, size: usize) -> Vec<u8> {
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let ext = self.data.ptr(d.at(4)).expect("ftData::ext_attr");
        let bytes = self.data.get(ext.archive).data();
        bytes[ext.offset as usize..ext.offset as usize + size].to_vec()
    }

    /// Copies the character's attributes into the fighter's backup block and makes that its
    /// attributes (`PUSH_ATTRS`).
    fn push_attrs(&mut self, f: FighterId, size: usize) {
        let src = self.ext_attrs(f, size);
        let backup = self.game.fighters[f.0]
            .dat_attrs_backup
            .expect("a fighter without an attribute block");
        self.game.fighter_attrs[backup.0][..size].copy_from_slice(&src);
        self.game.fighters[f.0].dat_attrs = Some(backup);
    }

    /// Copies the character's attributes over the fighter's (`COPY_ATTRS`).
    fn copy_attrs(&mut self, f: FighterId, size: usize) {
        let src = self.ext_attrs(f, size);
        let attrs: AttrsId = self.game.fighters[f.0]
            .dat_attrs
            .expect("a fighter without attributes");
        self.game.fighter_attrs[attrs.0][..size].copy_from_slice(&src);
    }

    /// A word of the fighter's attributes.
    fn attr_i32(&self, f: FighterId, off: usize) -> i32 {
        let a = self.game.fighters[f.0]
            .dat_attrs
            .expect("a fighter without attributes");
        let b = &self.game.fighter_attrs[a.0];
        i32::from_be_bytes(b[off..off + 4].try_into().unwrap())
    }

    /// Fox's creation, `ftFx_Init_OnLoad`: he can wall jump, has his attributes, and registers
    /// his blaster's shot, his blaster and his Illusion with the items.
    #[doc(alias = "ftFx_Init_OnLoad")]
    pub fn fox_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].can_walljump = true;
        self.push_attrs(f, FOX_ATTRS_SIZE);
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        let kinds = [
            self.attr_i32(f, 0x1C),
            self.attr_i32(f, 0x20),
            IT_KIND_FOX_ILLUSION,
        ];
        for (i, kind) in kinds.into_iter().enumerate() {
            let article = self.data.ptr(items.at(4 * i as u32));
            super::super::it::item::set_char_article(self.game, article, kind);
        }
    }

    /// Fox's spawn, `ftFx_Init_OnDeath`: no blaster out, and his first model variant.
    #[doc(alias = "ftFx_Init_OnDeath")]
    pub fn fox_on_death(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.u[..4].copy_from_slice(&[0; 4]);
        self.game.set_part_vis(gobj, 0, 0);
    }

    /// `ftFx_Init_LoadSpecialAttrs`.
    #[doc(alias = "ftFx_Init_LoadSpecialAttrs")]
    pub fn fox_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, FOX_ATTRS_SIZE);
    }

    /// Marth's creation, `ftMs_Init_OnLoad`: he has his attributes.
    #[doc(alias = "ftMs_Init_OnLoad")]
    pub fn mars_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.push_attrs(f, MARS_ATTRS_SIZE);
    }

    /// Marth's spawn, `ftMs_Init_OnDeath`: his first model variants, and `x222C` cleared.
    #[doc(alias = "ftMs_Init_OnDeath")]
    pub fn mars_on_death(&mut self, gobj: GObjId) {
        self.game.set_part_vis(gobj, 0, 0);
        self.game.set_part_vis(gobj, 1, 0);
        let fp = self.game.fighter_mut(gobj);
        fp.u[..4].copy_from_slice(&[0; 4]);
    }

    /// `ftMs_Init_LoadSpecialAttrs`.
    #[doc(alias = "ftMs_Init_LoadSpecialAttrs")]
    pub fn mars_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, MARS_ATTRS_SIZE);
    }
}
