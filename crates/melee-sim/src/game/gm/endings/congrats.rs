//! Congratulations (`gm_1A9B.c`, with `lb_01F8.c`'s still): after a single-player mode's
//! ending, one picture of the character for the mode (`GmRegend*.thp`, a single THP frame
//! decoded once into the three planes of a sprite drawn as the movies' are), its jingle by the
//! run's difficulty, and A or Start fading the screen to black before the scene ends; then the
//! game over modes' exit (`gm_801BEE9C`): the run's unlocks, then the next mode or a
//! challenger.

use melee_data::archive::ArchiveId;

use super::super::super::handles::CodeRef;
use super::super::super::state::{Obj, Running};
use crate::hsd::gobj::GObjId;
use crate::hsd::sobj::{self, GxTexObj, OBJ_SOBJ, SObjId};

/// `gm_Scene_Congrats_OnEnter`, `gm_Scene_Congrats_OnFrame`.
pub const ON_ENTER: CodeRef = CodeRef(0x801A_9B30);
pub const ON_FRAME: CodeRef = CodeRef(0x801A_9D0C);
/// `lbMthp8001F928`, the still's sprite's render callback.
pub const RENDER: u32 = 0x8001_F928;

/// `gmRegend_SimpleThpFiles`, `gmRegend_AdventureThpFiles`, `gmRegend_AllstarThpFiles`'
/// characters (Zelda and Sheik share one).
const NAMES: [&str; 26] = [
    "Captain",
    "Donkey",
    "Fox",
    "Gamewatch",
    "Kirby",
    "Koopa",
    "Link",
    "Luigi",
    "Mario",
    "Marth",
    "Mewtwo",
    "Ness",
    "Peach",
    "Pikachu",
    "Poponana",
    "Purin",
    "Samus",
    "Yoshi",
    "Zeldaseak",
    "Zeldaseak",
    "Falco",
    "Clink",
    "Drmario",
    "Roy",
    "Pichu",
    "Ganon",
];

/// The still's size (`lbMthp8001FAA0`'s `0x230` by `0x1A0`).
const WIDTH: u16 = 0x230;
const HEIGHT: u16 = 0x1A0;

/// `HSD_PAD_A | HSD_PAD_START`.
const PAD_A_START: u64 = 0x1100;

/// `lbl_804335B8` (`lb_01F8.c`): the still's planes, size, file and sprite.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Still {
    /// `x0`, `x24`, `x48`: the planes' texture objects, as the render callback last loaded
    /// them as maps 0 to 2 (none before it first draws).
    pub tex: Option<[GxTexObj; 3]>,
    /// `x20`, `x44`, `x68`: the planes' blocks (luma, then the two chroma planes).
    pub planes: [u32; 3],
    /// `x6C`, `x6E`: its size.
    pub width: u16,
    pub height: u16,
    /// `x90`: the sprite, and where its pool put it (the pointer stays as later scenes make
    /// the pool anew).
    pub sobj: Option<SObjId>,
    pub sobj_addr: u32,
    /// `unk94`, `unk98`: the file's block and its size, and its bytes' archive.
    pub file: u32,
    pub size: u32,
    pub archive: Option<ArchiveId>,
}

/// What the still's render callback draws: its planes from the file's single frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct StillFrame {
    pub file: ArchiveId,
    pub width: u16,
    pub height: u16,
}

/// Congratulations' statics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Congrats {
    /// `gmRegend_InputDelayTimer`, `gmRegend_ExitTimer`.
    pub input_wait: u8,
    pub exit_wait: u8,
    pub still: Still,
}

/// The game over mode congratulations' picture stands for: the debug one's stand-in, or its
/// own.
fn picture_mode(r: &Running) -> u8 {
    let m = super::super::super::mn::common::game_mode(r);
    if m == super::GM_DEBUG_GOVER {
        r.game.endings.gover.mode()
    } else {
        m
    }
}

/// The picture's file for character `ckind` in game over mode `mode`.
fn picture_file(mode: u8, ckind: u8) -> String {
    let kind = match mode {
        super::GM_CLASSIC_GOVER => "Simple",
        super::GM_ADVENTURE_GOVER => "Adventure",
        _ => "Allstar",
    };
    format!("GmRegend{kind}{}.thp", NAMES[usize::from(ckind)])
}

/// `THPDec_8032F8D4`: the image a JPEG frame holds (its width and height from the frame
/// header, and its sampling: 4 for 4:2:0, 2 for 4:2:2, 1 for 4:4:4, else 0), read up to the
/// scan, once a JFIF header has come; none if the frame isn't one (no start of image, not
/// three components, an APP0 that isn't JFIF, unequal sampling), as the original returns 0.
#[doc(alias = "THPDec_8032F8D4")]
pub fn read_image_desc(data: &[u8]) -> Option<crate::thp::ImageDesc> {
    let b = |i: usize| data.get(i).copied().unwrap_or(0);
    let mut out = crate::thp::ImageDesc::default();
    let mut at = 0;
    if b(0) != 0xFF || b(1) != 0xD8 {
        return None;
    }
    at += 2;
    let mut valid = false;
    loop {
        if b(at) != 0xFF {
            return None;
        }
        at += 1;
        while b(at) == 0xFF {
            at += 1;
        }
        let marker = b(at);
        at += 1;
        if marker == 0xC0 {
            out.height = (u16::from(b(at + 3)) << 8) | u16::from(b(at + 4));
            out.width = (u16::from(b(at + 5)) << 8) | u16::from(b(at + 6));
            let count = b(at + 7);
            at += 8;
            if count != 3 {
                return None;
            }
            let (mut h, mut v) = ([0u8; 3], [0u8; 3]);
            for i in 0..3 {
                let f = b(at + 1);
                h[i] = f >> 4;
                v[i] = f & 0xF;
                at += 3;
            }
            let div = |a: u8, c: u8| {
                if c == 0 {
                    0
                } else {
                    i32::from(a) / i32::from(c)
                }
            };
            if div(h[0], h[1]) == 2 && div(h[0], h[2]) == 2 {
                if div(v[0], v[1]) == 2 && div(v[0], v[2]) == 2 {
                    out.format = 4;
                } else if v[0] == v[1] && v[0] == v[2] {
                    out.format = 2;
                }
            } else if h[0] == h[1] && h[0] == h[2] {
                if v[0] == v[1] && v[0] == v[2] {
                    out.format = 1;
                }
            } else {
                return None;
            }
        } else if marker == 0xE0 {
            let length = (u16::from(b(at)) << 8) | u16::from(b(at + 1));
            at += 2;
            for &t in b"JFIF\0" {
                if b(at) != t {
                    return None;
                }
                at += 1;
            }
            valid = true;
            at += usize::from(length).wrapping_sub(7);
        } else if marker == 0xDA {
            break;
        } else if (0xC0..=0xFE).contains(&marker) {
            let length = (u16::from(b(at)) << 8) | u16::from(b(at + 1));
            at += 2 + usize::from(length).wrapping_sub(2);
        }
        if out.format != 0 && valid {
            break;
        }
    }
    Some(out)
}

impl Running<'_> {
    /// The still, `lbMthp8001FAA0`: the file read into the main heap (`lbFile_80016760`), the
    /// three planes' blocks, the decoder's image from the file's header (`THPDec_8032F8D4`,
    /// in a block of its own), its work area, the frame decoded into the planes
    /// (`THPVideoDecode`, `THPDec_803313D0`), and the image's and the work area's blocks
    /// freed.
    #[doc(alias = "lbMthp8001FAA0")]
    #[doc(alias = "lbFile_80016760")]
    fn congrats_still(&mut self, name: &str, width: u16, height: u16) {
        let (entrynum, len) = self.game.ft.data.files.entry(name);
        let archive = self.game.ft.data.files.archive(name);
        let file = crate::hsd::heap::with(|h| h.raw_file_alloc(len, archive)).unwrap_or(0);
        if self.game.audio.clock.is_some() {
            self.game.audio.load_file(self.data, entrynum, file, len);
        }
        let (w, h) = (u32::from(width), u32::from(height));
        let y = crate::hsd::heap::mem_alloc_unclaimed(w * h);
        let u = crate::hsd::heap::mem_alloc_unclaimed(w * h / 4);
        let v = crate::hsd::heap::mem_alloc_unclaimed(w * h / 4);
        let ctx = crate::hsd::heap::mem_alloc_as(0xC, "THPDec_8032FD40_Data");
        let bytes = self.data.get(archive).data();
        let desc = read_image_desc(bytes).unwrap_or_default();
        crate::hsd::heap::with(|hp| {
            let mut c = [0u8; 0xC];
            c[0..4].copy_from_slice(&desc.val0.to_be_bytes());
            c[4..6].copy_from_slice(&desc.width.to_be_bytes());
            c[6..8].copy_from_slice(&desc.height.to_be_bytes());
            c[8] = desc.format;
            hp.image.put(ctx, &c);
        });
        let work_len = crate::thp::work_size(&desc, height) as u32;
        let work = crate::hsd::heap::mem_alloc_unclaimed(work_len);
        crate::hsd::heap::with(|hp| {
            if !hp.image.on {
                return;
            }
            hp.image
                .claim(work, work + work_len, "the THP decoder's work area");
            let mut frame = vec![0u8; 4];
            frame.extend_from_slice(bytes);
            if let Ok(p) = crate::thp::decode_frame(&frame, width, height) {
                hp.image.put(y, &p.y);
                hp.image.put(u, &p.u);
                hp.image.put(v, &p.v);
            }
        });
        crate::hsd::heap::mem_free(ctx);
        crate::hsd::heap::mem_free(work);
        let s = &mut self.game.endings.congrats.still;
        s.width = width;
        s.height = height;
        s.planes = [y, u, v];
        s.file = file;
        s.size = len;
        s.archive = Some(archive);
    }

    /// The still's sprite on `gobj`, `lbMthp8001F890`: from the descriptor of its size in
    /// RGBA8 with no image (the planes replace it), drawn as a movie's.
    #[doc(alias = "lbMthp8001F890")]
    fn congrats_still_sprite(&mut self, gobj: GObjId) -> SObjId {
        let s = self.game.endings.congrats.still;
        let (o, head) = sobj::create(
            &mut self.game.objs.sobjs,
            self.data,
            gobj,
            None,
            sobj::Image::Movie {
                width: s.width,
                height: s.height,
            },
            (0, 0),
            0x80,
            None,
        );
        self.set_sobj_head(gobj, head);
        self.game.objs.sobjs[o.0].flags |= 0x10;
        self.game.endings.congrats.still.sobj = Some(o);
        self.game.endings.congrats.still.sobj_addr = self.game.objs.sobjs.addr(o.0);
        o
    }

    /// The still's render callback, `lbMthp8001F928`: the three planes as I8 texture maps 0
    /// to 2 (the luma full size, the chroma half), then the sprite.
    #[doc(alias = "lbMthp8001F928")]
    pub fn congrats_still_render(&mut self, gobj: GObjId) {
        let s = self.game.endings.congrats.still;
        let size = [
            (s.width, s.height),
            (s.width >> 1, s.height >> 1),
            (s.width >> 1, s.height >> 1),
        ];
        let tex = std::array::from_fn(|i| {
            near_lod(sobj::gx_init_tex_obj(
                s.planes[i],
                size[i].0,
                size[i].1,
                1,
                0,
                0,
                0,
            ))
            .loaded(i)
        });
        self.game.endings.congrats.still.tex = Some(tex);
        if let Some(file) = s.archive {
            self.game
                .objs
                .display
                .draws
                .push(crate::hsd::display::Draw::StillPlanes(StillFrame {
                    file,
                    width: s.width,
                    height: s.height,
                }));
        }
        self.render_sobjs(gobj);
    }

    /// `gm_Scene_Congrats_OnEnter`: the timers, a sprite camera over the screen at render
    /// priority 8 over link 11, the scene's sound banks, the background flash, the still of
    /// the character for the mode on a sprite GObj at the screen's middle, and its jingle (the
    /// hardest difficulty's on level 4).
    #[doc(alias = "gm_Scene_Congrats_OnEnter")]
    #[doc(alias = "gm_8017DFF4")]
    #[doc(alias = "fn_8017DEC8")]
    pub fn congrats_on_enter(&mut self) {
        let c = &mut self.game.endings.congrats;
        c.input_wait = 0x1E;
        c.exit_wait = 0;
        let cam = self.game.world.create(0x13, 0x14, 0);
        self.sprite_camera(cam, 0x280, 0x1E0, 8, false);
        self.game.world.gobj_mut(cam).gxlink_prios = 0x800;
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x12);
        a.lb.want_banks(2, 0x10);
        a.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
        self.bg_flash_create(10);
        let gobj = self.game.world.create(0xE, 0xF, 0);
        self.game.world.set_obj(gobj, OBJ_SOBJ, Obj::SObj(None));
        self.game.world.setup_gx_link(gobj, CodeRef(RENDER), 0xB, 0);
        let ckind = self.game.endings.gover.ckind();
        let name = picture_file(picture_mode(self), ckind);
        self.congrats_still(&name, WIDTH, HEIGHT);
        let s = self.congrats_still_sprite(gobj);
        let o = &mut self.game.objs.sobjs[s.0];
        o.x = 320.0;
        o.y = 240.0;
        o.flags |= 2;
        let o1 = &self.game.one_player;
        let level = match super::super::super::mn::common::game_mode(self) {
            super::GM_CLASSIC_GOVER => o1.classic.base.player.cpu_level,
            super::GM_ADVENTURE_GOVER => o1.adventure.base.player.cpu_level,
            _ => o1.allstar.base.player.cpu_level,
        };
        let sfx = if level == 4 { 0x9C45 } else { 0x9C41 };
        self.sfx_play(sfx, 0x7F, 0x40);
    }

    /// `gm_Scene_Congrats_OnFrame`: after 30 ticks, A or Start on the human's port fades the
    /// screen to black over 60 ticks and stops the sounds; at the fade's end the scene ends
    /// with its exit value (6 in the debug game over mode, else 1).
    #[doc(alias = "gm_Scene_Congrats_OnFrame")]
    pub fn congrats_on_frame(&mut self) {
        let c = &mut self.game.endings.congrats;
        if c.input_wait != 0 {
            c.input_wait -= 1;
        } else if c.exit_wait != 0 {
            c.exit_wait -= 1;
            if c.exit_wait == 0 {
                let v = if super::super::super::mn::common::game_mode(self) == super::GM_DEBUG_GOVER
                {
                    6
                } else {
                    1
                };
                self.regend_exit(v);
            }
        } else {
            let port = self.game.endings.gover.port();
            if self.game.buttons_triggered(port) & PAD_A_START != 0 {
                self.game.bgflash.fade_to_black(0x3C);
                self.game.endings.congrats.exit_wait = 0x3C;
                self.sfx_stop_all();
                self.lb_audio_call(1);
            }
        }
    }

    /// `gm_801A6630`: `v` into the scene's exit data's first byte, and the scene's end.
    #[doc(alias = "gm_801A6630")]
    #[doc(alias = "gm_GetCurrentSceneExitData")]
    fn regend_exit(&mut self, v: u8) {
        if super::super::super::mn::common::game_mode(self)
            == super::super::super::vi::GM_DEBUG_CUTSCENE
        {
            self.game.endings.congrats_exit[0] = v;
        } else {
            self.game.endings.gover_exit[0] = v;
        }
        self.game.end_scene();
    }

    /// The files congratulations reads, for the frontend: its still's.
    pub fn congrats_scene_files(&self) -> super::super::super::vi::SceneFiles {
        let mut f = super::super::super::vi::SceneFiles::default();
        let ckind = self.game.endings.gover.ckind();
        f.raws.push(picture_file(picture_mode(self), ckind));
        f
    }
}

/// `ChKind_None`, and the characters a mode's clear unlocks.
const CHKIND_NONE: u8 = 0x21;
const CKIND_GAMEWATCH: u8 = 3;
const CKIND_LUIGI: u8 = 7;
const CKIND_MARS: u8 = 9;
const CKIND_PURIN: u8 = 0xF;
const CKIND_ZELDA: u8 = 0x12;
const CKIND_SEAK: u8 = 0x13;
const CKIND_CLINK: u8 = 0x15;
const CKIND_DRMARIO: u8 = 0x16;
const CKIND_EMBLEM: u8 = 0x17;
const CKIND_MARIO: u8 = 8;

/// The game over modes' run (`fn_8017DEC8`): 0 Adventure's, 1 Classic's, 2 All-Star's.
fn run_data<'a>(r: &'a Running, n: u8) -> Option<&'a super::super::one_player::Unk1PData> {
    let o = &r.game.one_player;
    match n {
        0 => Some(&o.adventure.base),
        1 => Some(&o.classic.base),
        2 => Some(&o.allstar.base),
        _ => None,
    }
}

impl Running<'_> {
    /// `fn_80173098`: the character run `n`'s clear unlocks: Roy for Marth and Dr. Mario for
    /// Mario without a continue, Young Link once ten characters have cleared a mode, Mr. Game
    /// & Watch once every other character has cleared one mode, Luigi for an Adventure
    /// run that met him, Jigglypuff; each only while locked.
    #[doc(alias = "fn_80173098")]
    #[doc(alias = "gmMainLib_8015D344")]
    fn clear_challenger(&mut self, n: u8) -> u8 {
        self.game.sync_unlocks();
        let run = *run_data(self, n).expect("a single-player run");
        let locked = |r: &Self, ck: u8| !r.game.gm.unlocks.ckind_unlocked(ck);
        if run.xc[1] == 0 {
            if run.player.ckind as u8 == CKIND_MARS && locked(self, CKIND_EMBLEM) {
                return CKIND_EMBLEM;
            }
            if run.player.ckind as u8 == CKIND_MARIO && locked(self, CKIND_DRMARIO) {
                return CKIND_DRMARIO;
            }
        }
        let r = &self.game.game_data.thing.save_data.unk_8;
        let count = (0..0x19)
            .filter(|&i| (r.x10 | r.x14 | r.x18) & (1 << i) != 0)
            .count();
        if count >= 10 && locked(self, CKIND_CLINK) {
            return CKIND_CLINK;
        }
        if self.all_cleared_one_mode() && locked(self, CKIND_GAMEWATCH) {
            return CKIND_GAMEWATCH;
        }
        if n == 0 {
            let a = &self.game.one_player.adventure;
            if locked(self, CKIND_LUIGI) && a.x74[0] != 0 && a.x74[1] != 0 {
                return CKIND_LUIGI;
            }
        }
        if locked(self, CKIND_PURIN) {
            return CKIND_PURIN;
        }
        CHKIND_NONE
    }

    /// `gm_80173224`: the character a clear of run `n` unlocks (with `clear`), else Marth's
    /// by the characters played (`gm_80172E74`).
    #[doc(alias = "gm_80173224")]
    fn clear_unlock(&mut self, n: u8, clear: bool) -> u8 {
        let ckind = if clear {
            self.clear_challenger(n)
        } else {
            CHKIND_NONE
        };
        if ckind == CHKIND_NONE {
            self.challenger_by_characters()
        } else {
            ckind
        }
    }

    /// `fn_80173834`: character `ckind`'s trophy for mode `major` given anew (with `again`,
    /// its record mark cleared first), and with Zelda's or Sheik's the other's.
    #[doc(alias = "fn_80173834")]
    fn give_mode_trophy(&mut self, ckind: u8, major: u8, again: bool) {
        let give = |r: &mut Self, ck: u8| {
            let t = super::super::unlocks::mode_trophy(ck, major);
            if again {
                let n = t as u32;
                r.game.game_data.thing.save_data.x1B58[(n / 32) as usize] &= !(1 << (n % 32));
            }
            r.give_record_trophy(t);
        };
        give(self, ckind);
        if ckind == CKIND_ZELDA {
            give(self, CKIND_SEAK);
        }
        if ckind == CKIND_SEAK {
            give(self, CKIND_ZELDA);
        }
    }

    /// `gm_8017390C`: the trophies run `n`'s clear earns (with `clear`): Adventure's (0xE7
    /// for Luigi's cue, its character's, 0x51 and 0x53 without a continue for the cue and at
    /// least level 3, 0xE3 under 0x101D0 frames), Classic's (its character's, 0x52 without a
    /// continue at level 3, 0x11A under 0x4650 frames), All-Star's (its character's, 0xBE,
    /// 0xCC without a continue, 0x86 at level 3).
    #[doc(alias = "gm_8017390C")]
    fn clear_trophies(&mut self, n: u8, clear: bool) {
        match n {
            0 => {
                let a = self.game.one_player.adventure;
                if a.x74[2] != 0 {
                    self.give_record_trophy(0xE7);
                }
                if clear {
                    let b = a.base;
                    self.give_mode_trophy(b.player.ckind as u8, 4, true);
                    if b.xc[1] == 0 && a.x74[3] != 0 {
                        self.give_record_trophy(0x51);
                    }
                    if b.xc[1] == 0 && b.player.cpu_level >= 3 {
                        self.give_record_trophy(0x53);
                    }
                    if b.x20 < 0x101D0 {
                        self.give_record_trophy(0xE3);
                    }
                }
            }
            1 if clear => {
                let b = self.game.one_player.classic.base;
                self.give_mode_trophy(b.player.ckind as u8, 3, true);
                if b.xc[1] == 0 && b.player.cpu_level >= 3 {
                    self.give_record_trophy(0x52);
                }
                if b.x20 < 0x4650 {
                    self.give_record_trophy(0x11A);
                }
            }
            2 if clear => {
                let b = self.game.one_player.allstar.base;
                self.give_mode_trophy(b.player.ckind as u8, 5, true);
                self.give_record_trophy(0xBE);
                if b.xc[1] == 0 {
                    self.give_record_trophy(0xCC);
                }
                if b.player.cpu_level >= 3 {
                    self.give_record_trophy(0x86);
                }
            }
            _ => {}
        }
    }

    /// The game over modes' congratulations' exit, `gm_801BEE9C`: the character run's clear
    /// unlocks, Flat Zone for Mr. Game & Watch, the run's trophies, the save's trophies and
    /// special messages (`0x40`); then a challenger for the unlocked character, or the
    /// challenger approach for a notice, else the mode the exit data names.
    #[doc(alias = "gm_801BEE9C")]
    pub fn gover_congrats_exit(&mut self) {
        let next = self.game.endings.gover_exit[0];
        let g = self.game.endings.gover;
        let ckind = self.clear_unlock(g.slot(), true);
        if g.ckind() == CKIND_GAMEWATCH && !self.stage_unlocked(0x1B) {
            self.unlock_stage(0x1B);
        }
        self.clear_trophies(g.slot(), true);
        self.trophy_unlocks();
        self.special_unlocks(0x40);
        if ckind == CHKIND_NONE {
            if !self.challenger_check(1, g.color()) {
                self.set_pending_game_mode(next);
            }
        } else {
            self.init_challenger(g.ckind(), g.color(), g.port(), g.tag(), ckind, next);
            self.set_pending_game_mode(super::super::machine::GM_CHALLENGER_APPROACH);
        }
        self.set_new_game_mode_pending();
    }
}

/// `GXInitTexObjLOD(tex, GX_NEAR, GX_NEAR, 0, 0, 0, false, false, GX_ANISO_1)` on a texture
/// object: point sampling both ways, no edge LOD, no bias, the LODs 0.
fn near_lod(mut t: GxTexObj) -> GxTexObj {
    t.mode0 = (t.mode0 & !0x003F_FFF0) | 0x100;
    t.mode1 &= !0xFFFF;
    t
}
