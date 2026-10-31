//! The debug menu's "Yoshiki Test" page (`un_803FD310`, `if_3004.c`): the card layer's calls
//! one entry each. Init sets up the card's work areas, the snapshots' buffers, an image and a
//! sprite camera; Format, Accessable, Create, Save, Load and Delete run the save's lists on
//! slot A; the snapshot entries list slot A's snapshots, read one (shown as a sprite), delete
//! one or swap two, the game spinning on each list's steps inside the callback. Only a debug
//! level reaches the page (the developer setting).
//!
//! Format, Delete and the snapshots' Delete and Swap change the card in slot A, which in the
//! player is the player's own save ([`crate::lb::save`]).

use super::debug_menu::CB_SELECT;
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::sobj;
use crate::lb::cardgame::{STATUS_NO_CARD, STATUS_OK};
use crate::lb::save::result;

/// The page's statics: `un_804D6E04` (the image Init allocates), `un_804D6E08` (the snapshot's
/// sprite) and the image pointer SnapLoad0 writes into `un_803FD274`, the sprite's
/// `HSD_ImageDesc` in `.data` (0 until then).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Yoshiki {
    pub image: u32,
    pub sprite: Option<GObjId>,
    pub desc_image: u32,
}

/// `un_803FD274`'s image: 640 by 480, `GX_TF_RGB565`.
const IMAGE_WIDTH: u16 = 0x280;
const IMAGE_HEIGHT: u16 = 0x1E0;
const IMAGE_FORMAT: u32 = 4;
/// The image's block (`HSD_MemAlloc(0x96000)`).
const IMAGE_SIZE: u32 = 0x96000;
/// The snapshot entries' indices (`un_804D6E0C`, `un_804D6E10`, `un_804D6E14`).
const SNAP_LOAD: u32 = 0x804D_6E0C;
const SNAP_DELETE: u32 = 0x804D_6E10;
const SNAP_SWAP: u32 = 0x804D_6E14;

impl Running<'_> {
    /// The Yoshiki Test's entries' callbacks by address, none for another address.
    #[doc(alias = "un_80301634")]
    #[doc(alias = "un_803016D8")]
    #[doc(alias = "un_80301734")]
    #[doc(alias = "un_8030178C")]
    #[doc(alias = "un_80301800")]
    #[doc(alias = "un_80301840")]
    #[doc(alias = "un_803018BC")]
    #[doc(alias = "un_8030191C")]
    pub fn yoshiki_item(&mut self, cb: u32, arg: i32) -> Option<i32> {
        match cb {
            // Init: whatever it's told (the page made, A, B or Start on it).
            0x8030_1634 => self.yoshiki_init(),
            // The rest act on A alone.
            0x8030_16D8 | 0x8030_1734 | 0x8030_178C | 0x8030_1800 | 0x8030_1840 | 0x8030_18BC
            | 0x8030_191C | 0x8030_1964 | 0x8030_1A64 | 0x8030_1AD4 => {
                if arg == CB_SELECT {
                    self.yoshiki_entry(cb);
                }
            }
            _ => return None,
        }
        Some(0)
    }

    /// `un_80301634`, Init: the card's work areas and the card game's archives, the snapshots'
    /// buffers (the slots' lists first), the image, and a sprite camera over 640 by 480 at
    /// render priority 12 drawing link 18 (`HSD_SObjLib_803A55DC`); the sprite forgotten. It
    /// runs on every call, so each allocates anew and leaves the last ones (Melee's bugs).
    #[doc(alias = "HSD_SObjLib_803A55DC")]
    fn yoshiki_init(&mut self) {
        self.card_alloc_work_area();
        self.card_game_load_archive(0);
        let slot = crate::hsd::heap::mem_alloc(crate::lb::snap::SLOTS_SIZE);
        let snap = crate::hsd::heap::mem_alloc_unclaimed(crate::lb::snap::SNAP_SIZE);
        self.snap_set_buffers(snap, slot);
        // Only a snapshot decoded into it writes the image, so the heap's image keeps what it
        // held until one does.
        self.game.hud.debug.yoshiki.image = crate::hsd::heap::mem_alloc_unclaimed(IMAGE_SIZE);
        let gobj = self.game.world.create(0x13, 0x14, 0);
        self.sprite_camera(gobj, IMAGE_WIDTH, IMAGE_HEIGHT, 12, false);
        self.game.world.gobj_mut(gobj).gxlink_prios = 0x40000;
        self.game.hud.debug.yoshiki.sprite = None;
    }

    /// An entry but Init on A.
    fn yoshiki_entry(&mut self, cb: u32) {
        match cb {
            // `un_803016D8`, Format.
            0x8030_16D8 => {
                self.card_format(0);
            }
            // `un_80301734`, Accessable: whether the save is there.
            0x8030_1734 => {
                self.card_probe_save();
            }
            // `un_8030178C`, Create: the save created; the card as the game knows it once it is.
            0x8030_178C => {
                if self.card_game_create() == result::READY {
                    self.game.card_game.set_card_status(STATUS_OK);
                }
            }
            // `un_80301800`, Save: the save marked to write, which the next wait takes up.
            0x8030_1800 => self.card_game_mark_changes(),
            // `un_80301840`, Load: the save loaded (or refused), every file it didn't bring
            // reset, and the card as the game knows it when it read the save.
            0x8030_1840 => {
                let r = self.card_game_load();
                self.after_card_load(r);
                if r == result::READY || r == result::DAMAGED {
                    self.game.card_game.set_card_status(STATUS_OK);
                }
            }
            // `un_803018BC`, Delete: the save deleted, and the card taken as gone whatever came
            // of it.
            0x8030_18BC => {
                self.card_delete_save();
                self.game.card_game.set_card_status(STATUS_NO_CARD);
            }
            // `un_8030191C`, SnapMount0: slot A's snapshots listed.
            0x8030_191C => {
                self.snap_list(0);
            }
            0x8030_1964 => self.yoshiki_snap_load(),
            // `un_80301A64`, SnapDelete0, and `un_80301AD4`, SnapSwap0 (its index with
            // SnapDelete0's): the list spun to its end, then slot A listed again when it went
            // well.
            0x8030_1A64 | 0x8030_1AD4 => {
                let d = self.game.hud.debug.word(SNAP_DELETE) as i32;
                let r = if cb == 0x8030_1A64 {
                    self.snap_delete(0, d)
                } else {
                    let s = self.game.hud.debug.word(SNAP_SWAP) as i32;
                    self.snap_swap(0, s, d)
                };
                if r == result::BUSY && self.card_spin(r) == result::READY {
                    self.snap_list(0);
                }
            }
            _ => unreachable!("not a Yoshiki Test entry: {cb:#010x}"),
        }
    }

    /// `un_80301964`, SnapLoad0: snapshot `un_804D6E0C` of slot A read, the list spun to its
    /// end, and when it went well and the buffer holds an image, the image decoded into
    /// Init's and shown as a sprite placed by its center at (320, 240) on a new GObj drawing
    /// at link 18, the last one's GObj freed.
    #[doc(alias = "un_80301964")]
    #[doc(alias = "un_80301A64")]
    #[doc(alias = "un_80301AD4")]
    fn yoshiki_snap_load(&mut self) {
        let index = self.game.hud.debug.word(SNAP_LOAD) as i32;
        let r = self.snap_read(0, index);
        if r != result::BUSY || self.card_spin(r) != result::READY {
            return;
        }
        let image = self.game.hud.debug.yoshiki.image;
        if self.snap_decode(image) == 0 {
            return;
        }
        if let Some(g) = self.game.hud.debug.yoshiki.sprite {
            crate::hsd::gobj::free(self, g);
        }
        let g = self.game.world.create(0xE, 0xF, 0);
        self.game
            .world
            .setup_gx_link(g, CodeRef(sobj::RENDER), 0x12, 0);
        self.game.hud.debug.yoshiki.desc_image = image;
        let (s, head) = sobj::create(
            &mut self.game.objs.sobjs,
            self.data,
            g,
            None,
            sobj::Image::Buffer {
                addr: image,
                width: IMAGE_WIDTH,
                height: IMAGE_HEIGHT,
                format: IMAGE_FORMAT,
            },
            (0, 0),
            0x80,
            None,
        );
        self.set_sobj_head(g, head);
        let o = &mut self.game.objs.sobjs[s.0];
        o.x = 320.0;
        o.y = 240.0;
        o.flags |= 2;
        self.game.hud.debug.yoshiki.sprite = Some(g);
    }
}
