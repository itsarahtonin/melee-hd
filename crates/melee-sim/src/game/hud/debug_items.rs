//! The debug menus' entries' callbacks (`if_2FFC.c`, `if_3004.c`): submenus, the debug
//! mode's states and other modes they go to, and the settings they copy. Each is
//! `soundtest_callback`: told what happened ([`super::debug_menu::CB_BACK`] and the rest),
//! it answers 0 for the menu's own callback to run after it.

use super::debug_menu::{CB_BACK, CB_SELECT, CB_START};
use crate::game::gm::machine::{GM_BOOT, GM_MENU, GM_OPENING_MV, GM_PROGRESSIVE_SCAN, GM_TITLE};
use crate::game::state::Running;

/// `GM_HANYU_CSS`, `GM_HANYU_SSS`, `GM_DEBUG_CUTSCENE`, `GM_CAMERA_VS`.
const GM_HANYU_CSS: u8 = 8;
const GM_HANYU_SSS: u8 = 9;
const GM_DEBUG_CUTSCENE: u8 = 0x19;
const GM_CAMERA_VS: u8 = 0x2A;

/// `fn_802FFE6C`, the submenus' own callback.
pub const SUBMENU_BACK: u32 = 0x802F_FE6C;

impl Running<'_> {
    /// `lbAudioAx_80024030(1)` and `(0)`: the menus' forward and back sounds.
    fn sfx_forward(&mut self) {
        self.lb_audio_call(1);
    }

    fn sfx_back(&mut self) {
        self.lb_audio_call(0);
    }

    /// The scene ends for the debug mode's state `state` (`gm_SetNextGameModeStateId`,
    /// `gm_801A4B60`).
    fn debug_to_state(&mut self, state: u8) {
        self.set_next_game_mode_state_id(state);
        self.game.end_scene();
    }

    /// The scene ends for game mode `mode` (`gm_ChangeGameModeAfterCurrentScene`,
    /// `gm_801A4B60`).
    fn debug_to_mode(&mut self, mode: u8) {
        self.change_game_mode_after_scene(mode);
        self.game.end_scene();
    }

    /// `un_802FFD94`: on A, a submenu with `table` over the current menu, with `cb` its own
    /// callback.
    #[doc(alias = "un_802FFD94")]
    fn debug_submenu(&mut self, arg: i32, table: u32, cb: u32) {
        if arg == CB_SELECT {
            let cur = self
                .game
                .hud
                .devtext
                .menu_current
                .expect("a current menu")
                .slot;
            self.sfx_forward();
            self.menu_open_sub(cur, table, -60, 0);
            let sub = self
                .game
                .hud
                .devtext
                .menu_current
                .expect("the submenu")
                .slot;
            self.menu_set_cb(sub, cb);
        }
    }

    /// `un_802FFCD0(4, values)`: the settings' four words at `off` set to the fifth after
    /// them (the "All" entry's value).
    #[doc(alias = "un_802FFCD0")]
    fn debug_copy_all(&mut self, off: u32) {
        let v = self.game.hud.debug.setting(off + 16);
        for i in 0..4 {
            self.game.hud.debug.set_setting(off + 4 * i, v);
        }
    }

    /// The card data's bytes at `off` (`gmMainLib_8045A6C0.thing`, at its 0x1868, plus `off`),
    /// as memory holds them.
    fn card_data_bytes_at(&self, off: usize, n: usize) -> Vec<u8> {
        let bytes = crate::lb::save::card_data_bytes(&self.game.game_data.thing);
        if off + n > bytes.len() {
            super::super::code::unported_call(&format!(
                "the debug menu's Global Data Edit past the card data (offset {off:#x})"
            ));
        }
        bytes[off..off + n].to_vec()
    }

    /// The Global Data Edit's reads (`un_803003C4`, `un_80300378`, `un_80300338`): the
    /// address aligned to `align`, and the four bytes of the save there into the value.
    fn debug_global_read(&mut self, align: u16) {
        let addr = (self.game.hud.debug.setting(0xF0) >> 16) as u16 & !(align - 1);
        let mut w = self.game.hud.debug.setting(0xF0);
        w = (w & 0xFFFF) | u32::from(addr) << 16;
        self.game.hud.debug.set_setting(0xF0, w);
        let b = self.card_data_bytes_at(usize::from(addr), 4);
        self.game.hud.debug.settings[0xF4..0xF8].copy_from_slice(&b);
    }

    /// The debug menus' entries' callbacks by address, none for another address.
    #[doc(alias = "fn_802FFE0C")]
    #[doc(alias = "un_802FFEA4")]
    #[doc(alias = "un_80300AB8")]
    #[doc(alias = "un_80300FEC")]
    #[doc(alias = "un_80301DCC")]
    #[doc(alias = "un_80300290")]
    #[doc(alias = "un_803003C4")]
    #[doc(alias = "un_80300378")]
    #[doc(alias = "un_80300338")]
    #[doc(alias = "un_80300410")]
    #[doc(alias = "un_80300480")]
    #[doc(alias = "un_803004B4")]
    #[doc(alias = "un_803004E8")]
    #[doc(alias = "un_8030051C")]
    #[doc(alias = "un_80300550")]
    #[doc(alias = "un_80300584")]
    #[doc(alias = "un_803005B8")]
    #[doc(alias = "un_803005EC")]
    #[doc(alias = "un_80300620")]
    #[doc(alias = "un_80300654")]
    #[doc(alias = "un_80300688")]
    #[doc(alias = "un_803006BC")]
    #[doc(alias = "un_803006F0")]
    #[doc(alias = "un_80300724")]
    #[doc(alias = "un_803007C8")]
    #[doc(alias = "un_803007FC")]
    #[doc(alias = "un_80300830")]
    #[doc(alias = "un_80300864")]
    #[doc(alias = "un_80300898")]
    #[doc(alias = "un_803008CC")]
    #[doc(alias = "un_80300900")]
    #[doc(alias = "un_80300934")]
    #[doc(alias = "un_80300758")]
    #[doc(alias = "un_80300790")]
    #[doc(alias = "un_80300968")]
    #[doc(alias = "un_803009A4")]
    #[doc(alias = "un_80300AF4")]
    #[doc(alias = "un_80300B58")]
    #[doc(alias = "un_80301328")]
    #[doc(alias = "un_8030136C")]
    #[doc(alias = "un_803013B0")]
    #[doc(alias = "un_80301454")]
    #[doc(alias = "un_803015BC")]
    #[doc(alias = "un_80301D40")]
    #[doc(alias = "un_803013EC")]
    #[doc(alias = "un_80301420")]
    #[doc(alias = "un_80301490")]
    #[doc(alias = "un_803014CC")]
    #[doc(alias = "un_80301508")]
    #[doc(alias = "un_80301544")]
    #[doc(alias = "un_80301580")]
    #[doc(alias = "un_803015F8")]
    #[doc(alias = "un_80301E08")]
    #[doc(alias = "un_80301C80")]
    #[doc(alias = "un_80301CE0")]
    pub fn debug_item(&mut self, cb: u32, arg: i32) -> Option<i32> {
        let ret = match cb {
            // `fn_802FFE0C`, the DaiRanTou page's own: B closes it, Start starts the match.
            0x802F_FE0C => {
                match arg {
                    CB_BACK => {
                        self.sfx_back();
                        self.menu_close_current();
                    }
                    CB_START => {
                        self.sfx_forward();
                        self.debug_to_state(4);
                    }
                    _ => {}
                }
                0
            }
            // `un_802FFEA4` (EXIT), `un_80300AB8`, `un_80300FEC`, `un_80301DCC`: the match.
            0x802F_FEA4 | 0x8030_0AB8 | 0x8030_0FEC | 0x8030_1DCC => {
                if arg == CB_SELECT {
                    self.sfx_forward();
                    self.debug_to_state(4);
                }
                0
            }
            // `un_80300290`: Global Data Edit.
            0x8030_0290 => {
                self.debug_submenu(arg, 0x803F_A658, SUBMENU_BACK);
                0
            }
            0x8030_03C4 => {
                self.debug_global_read(4);
                0
            }
            0x8030_0378 => {
                self.debug_global_read(2);
                0
            }
            0x8030_0338 => {
                self.debug_global_read(1);
                0
            }
            // `un_80300410`: A writes the value's four bytes into the save.
            0x8030_0410 => {
                if arg == CB_SELECT {
                    self.sfx_forward();
                    let addr = (self.game.hud.debug.setting(0xF0) >> 16) as usize;
                    let at = addr;
                    let mut bytes = crate::lb::save::card_data_bytes(&self.game.game_data.thing);
                    if at + 4 > bytes.len() {
                        super::super::code::unported_call(&format!(
                            "the debug menu's Global Data Edit past the card data (offset {at:#x})"
                        ));
                    }
                    let v: [u8; 4] = self.game.hud.debug.settings[0xF4..0xF8].try_into().unwrap();
                    bytes[at..at + 4].copy_from_slice(&v);
                    self.game.game_data.thing = crate::lb::save::card_data_import(&bytes);
                    self.game.sync_unlocks();
                }
                0
            }
            // The submenus `if_3004.c` opens with `un_802FFD94`.
            0x8030_0480 => {
                self.debug_submenu(arg, 0x803F_B168, 0x802F_FE0C);
                0
            }
            0x8030_04B4 => self.debug_sub(arg, 0x803F_A8E8),
            0x8030_04E8 => self.debug_sub(arg, 0x803F_AA44),
            0x8030_051C => self.debug_sub(arg, 0x803F_B60C),
            0x8030_0550 => self.debug_sub(arg, 0x803F_B728),
            0x8030_0584 => self.debug_sub(arg, 0x803F_B870),
            0x8030_05B8 => self.debug_sub(arg, 0x803F_B98C),
            0x8030_05EC => self.debug_sub(arg, 0x803F_BA9C),
            0x8030_0620 => self.debug_sub(arg, 0x803F_BB98),
            0x8030_0654 => self.debug_sub(arg, 0x803F_BCAC),
            0x8030_0688 => self.debug_sub(arg, 0x803F_BDC0),
            0x8030_06BC => self.debug_sub(arg, 0x803F_BFFC),
            0x8030_06F0 => self.debug_sub(arg, 0x803F_C0FC),
            0x8030_0724 => self.debug_sub(arg, 0x803F_C22C),
            0x8030_07C8 => self.debug_sub(arg, 0x803F_C63C),
            0x8030_07FC => self.debug_sub(arg, 0x803F_CA40),
            0x8030_0830 => self.debug_sub(arg, 0x803F_CC38),
            0x8030_0864 => self.debug_sub(arg, 0x803F_CE4C),
            0x8030_0898 => self.debug_sub(arg, 0x803F_D064),
            0x8030_08CC => self.debug_sub(arg, 0x803F_DAF8),
            0x8030_0900 => self.debug_sub(arg, 0x803F_DB9C),
            0x8030_0934 => self.debug_sub(arg, 0x803F_D310),
            // `un_80300758` (All Char) and `un_80300790` (All PKind): A sets the four players'
            // to the "All" entry's. Their C returns nothing: the machine code returns the
            // argument, or the copy's count (4) after it ([Melee's bugs]).
            0x8030_0758 | 0x8030_0790 => {
                if arg == CB_SELECT {
                    self.debug_copy_all(if cb == 0x8030_0758 { 0x10 } else { 0x24 });
                    4
                } else {
                    arg
                }
            }
            0x8030_0968 | 0x8030_09A4 => {
                if arg == CB_SELECT {
                    self.sfx_forward();
                    self.debug_to_mode(if cb == 0x8030_0968 {
                        GM_HANYU_CSS
                    } else {
                        GM_HANYU_SSS
                    });
                }
                0
            }
            // `un_80300AF4` (Go to Meikyu) and `un_80300B58` (Go to Kinoko): a match on that
            // stage with that character on the first port, and computer players on the
            // others (`startDebugMatch`).
            0x8030_0AF4 | 0x8030_0B58 => {
                if arg == CB_SELECT {
                    let (stage, ckind) = if cb == 0x8030_0AF4 {
                        (0x3F, 0xE)
                    } else {
                        (0x3B, 2)
                    };
                    self.sfx_forward();
                    let d = &mut self.game.hud.debug;
                    d.set_setting(0x8, stage);
                    d.set_setting(0x10, ckind);
                    for i in 1..4 {
                        d.set_setting(0x24 + 4 * i, 3);
                    }
                    self.debug_to_state(4);
                }
                0
            }
            // `un_80301328` and `un_8030136C`: the opening movie mode from its movie or the
            // Special Movie.
            0x8030_1328 | 0x8030_136C => {
                if arg == CB_SELECT {
                    self.sfx_forward();
                    self.game.boot_mode.opening_state = if cb == 0x8030_1328 { 0 } else { 5 };
                    self.debug_to_mode(GM_OPENING_MV);
                }
                0
            }
            0x8030_13B0 => self.debug_mode_item(arg, GM_DEBUG_CUTSCENE),
            0x8030_1454 => self.debug_mode_item(arg, GM_BOOT),
            0x8030_15BC => self.debug_mode_item(arg, GM_PROGRESSIVE_SCAN),
            0x8030_1D40 => self.debug_mode_item(arg, GM_CAMERA_VS),
            0x8030_13EC => {
                self.debug_submenu(arg, 0x803F_D58C, 0x8030_1B48);
                0
            }
            0x8030_1420 => {
                self.debug_submenu(arg, 0x803F_D864, 0x8030_1C80);
                0
            }
            0x8030_1490 => self.debug_state_item(arg, 0xD),
            0x8030_14CC => self.debug_state_item(arg, 7),
            0x8030_1508 => self.debug_state_item(arg, 3),
            0x8030_1544 => self.debug_state_item(arg, 9),
            0x8030_1580 => self.debug_state_item(arg, 0xC),
            0x8030_15F8 => self.debug_state_item(arg, 0xA),
            0x8030_1E08 => self.debug_state_item(arg, 0xE),
            // `un_80301B48` (the intro test's) and `un_80301C80` (the result test's): B
            // closes the submenu, Start goes to the scene.
            0x8030_1B48 | 0x8030_1C80 => {
                match arg {
                    CB_BACK => {
                        self.sfx_back();
                        self.menu_close_current();
                    }
                    CB_START => {
                        self.sfx_forward();
                        self.debug_to_state(if cb == 0x8030_1B48 { 6 } else { 0xB });
                    }
                    _ => {}
                }
                0
            }
            // `un_80301CE0` (All Player): Start goes to the result test, A sets the four
            // players' to the "All" entry's. Its C returns nothing: the machine code returns
            // what the last call left in r3, `gm_80479D58`'s address after Start, 4 after A,
            // else the argument ([Melee's bugs]).
            0x8030_1CE0 => match arg {
                CB_START => {
                    self.sfx_forward();
                    self.debug_to_state(0xB);
                    0x8047_9D58_u32 as i32
                }
                CB_SELECT => {
                    self.debug_copy_all(0x144);
                    4
                }
                _ => arg,
            },
            // The Otoguro test: trophies, coins and records.
            0x8030_09E0 => {
                self.debug_record_read();
                0
            }
            0x8030_0A88 => {
                if self.game.debug_mode.records_kind == 3
                    && self.game.hud.debug.word(0x804D_6DCC) as i32 >= 1
                {
                    self.game.hud.debug.set_word(0x804D_6DCC, 1);
                }
                0
            }
            0x8030_0BBC | 0x8030_0BF8 | 0x8030_0C3C | 0x8030_0C80 | 0x8030_0F98 => {
                if arg == CB_SELECT {
                    self.debug_otoguro_item(cb);
                }
                0
            }
            // `un_80300D78` (Set Melee Count): the totals' values to its default, and their
            // submenu; `un_80300E74` (Set HomeRun Record) and `un_80300F3C` (Set Target
            // Clear): the record's submenu.
            0x8030_0D78 => {
                if arg == CB_SELECT {
                    self.sfx_forward();
                    let d = &mut self.game.hud.debug;
                    let v = d.word(0x804D_6DF0);
                    for a in [0x804D_6DE0, 0x804D_6DE4, 0x804D_6DE8, 0x804D_6DEC] {
                        d.set_word(a, v);
                    }
                    self.debug_submenu(arg, 0x803F_C70C, 0x8030_0CC8);
                }
                0
            }
            0x8030_0E74 | 0x8030_0F3C => {
                if arg == CB_SELECT {
                    self.sfx_forward();
                    let (kind, table, own) = if cb == 0x8030_0E74 {
                        (2, 0x803F_C8C8, 0x8030_0DE0)
                    } else {
                        (3, 0x803F_C828, 0x8030_0ED0)
                    };
                    self.game.debug_mode.records_kind = kind;
                    self.debug_submenu(arg, table, own);
                }
                0
            }
            // The submenus' own callbacks: B closes the submenu, Start writes the save and
            // goes to the main menu. Their C returns nothing; no caller reads a menu's own
            // callback's answer.
            0x8030_0CC8 | 0x8030_0DE0 | 0x8030_0ED0 => {
                match arg {
                    CB_BACK => {
                        self.sfx_back();
                        self.menu_close_current();
                    }
                    CB_START => {
                        self.sfx_forward();
                        self.debug_records_write(cb);
                        self.debug_to_mode(GM_MENU);
                    }
                    _ => {}
                }
                0
            }
            // The single-player modes' and their game overs' entries (milestone 6's), acting
            // only on A, not ported yet.
            0x8030_1028 | 0x8030_1074 | 0x8030_10C0 | 0x8030_1280 | 0x8030_12D4 | 0x8030_1D7C => {
                if arg == CB_SELECT {
                    super::super::code::unported_call(&format!(
                        "the debug menu entry callback {cb:#010x}"
                    ));
                }
                0
            }
            _ => return None,
        };
        Some(ret)
    }

    /// `un_803009E0`, the record's character's callback: its record into the count (the
    /// home-run distance in metres, or the target test's clear flag, written back).
    #[doc(alias = "un_803009E0")]
    #[doc(alias = "un_80300A88")]
    #[doc(alias = "gmMainLib_8015D06C")]
    #[doc(alias = "gmMainLib_8015D48C")]
    #[doc(alias = "gmMainLib_8015D4E8")]
    fn debug_record_read(&mut self) {
        let chara = usize::from(self.game.hud.debug.word(0x804D_6DC8) as u8);
        match self.game.debug_mode.records_kind {
            2 => {
                let v = self.game.game_data.thing.save_data.x1F2C[chara].x7C.x84 as u32;
                let m = crate::fpu::mem(crate::gekko_fp::frsp(f64::from(v / 100)));
                self.game.hud.debug.set_word(0x804D_6DD0, m.to_bits());
            }
            3 => {
                let f = &mut self.game.game_data.thing.save_data.x1F2C[chara].x7C;
                let r = i32::from(f.b0);
                f.b0 = r & 1 != 0;
                self.game.hud.debug.set_word(0x804D_6DCC, r as u32);
            }
            _ => {}
        }
    }

    /// The Otoguro test's entries that act on A: every trophy (`un_80300BBC`, then the
    /// gallery), the Samus mask's and Mario and Yoshi's trophies (`un_80300BF8`,
    /// `un_80300C3C`), the coins (`un_80300C80`) and the KO total (`un_80300F98`), each
    /// then the main menu.
    #[doc(alias = "un_80300BBC")]
    #[doc(alias = "un_80300BF8")]
    #[doc(alias = "un_80300C3C")]
    #[doc(alias = "un_80300C80")]
    #[doc(alias = "un_80300F98")]
    fn debug_otoguro_item(&mut self, cb: u32) {
        let mode = match cb {
            0x8030_0BBC => {
                self.mark_class_two_trophies();
                self.hold_every_trophy();
                crate::game::ty::GM_TOY_GALLERY
            }
            0x8030_0BF8 | 0x8030_0C3C => {
                self.mark_class_two_trophies();
                self.add_trophy(if cb == 0x8030_0BF8 { 0xE6 } else { 0xC9 }, 1);
                GM_MENU
            }
            0x8030_0C80 => {
                let n = self.game.hud.debug.word(0x804D_6DDC);
                self.game.game_data.thing.save_data.x1A48 = n.wrapping_mul(10);
                self.mark_class_two_trophies();
                GM_MENU
            }
            _ => {
                let n = self.game.hud.debug.word(0x804D_6DF4);
                self.sfx_forward();
                self.game.game_data.thing.save_data.x1A5C = n as i32;
                GM_MENU
            }
        };
        self.game.sync_unlocks();
        self.change_game_mode_after_scene(mode);
        self.game.end_scene();
    }

    /// The submenus' Start: the VS totals (`fn_80300CC8`), the home-run record
    /// (`fn_80300DE0`, the count's metres as centimetres) or the target test's clear
    /// (`fn_80300ED0`) into the save.
    #[doc(alias = "fn_80300CC8")]
    #[doc(alias = "fn_80300DE0")]
    #[doc(alias = "fn_80300ED0")]
    fn debug_records_write(&mut self, cb: u32) {
        let d = &self.game.hud.debug;
        let chara = usize::from(d.word(0x804D_6DC8) as u8);
        let w = |a: u32| d.word(a);
        let (t, st, c, b) = (
            w(0x804D_6DE0),
            w(0x804D_6DE4),
            w(0x804D_6DE8),
            w(0x804D_6DEC),
        );
        let count = f32::from_bits(w(0x804D_6DD0));
        let clear = w(0x804D_6DCC);
        let s = &mut self.game.game_data.thing.save_data;
        match cb {
            0x8030_0CC8 => {
                s.time_matches = t;
                s.stock_matches = st;
                s.coin_matches = c;
                s.bonus_matches = b;
            }
            0x8030_0DE0 => {
                use crate::gekko_fp as fp;
                let v = fp::fmuls(100.0, crate::fpu::reg(count));
                s.x1F2C[chara].x7C.x84 = crate::ppc::cvt_fp2unsigned(v) as i32;
            }
            _ => s.x1F2C[chara].x7C.b0 = clear & 1 != 0,
        }
        self.game.sync_unlocks();
    }

    /// A submenu with the submenus' own callback, answering 0.
    fn debug_sub(&mut self, arg: i32, table: u32) -> i32 {
        self.debug_submenu(arg, table, SUBMENU_BACK);
        0
    }

    /// An entry whose A goes to the debug mode's state `state`.
    fn debug_state_item(&mut self, arg: i32, state: u8) -> i32 {
        if arg == CB_SELECT {
            self.sfx_forward();
            self.debug_to_state(state);
        }
        0
    }

    /// An entry whose A goes to game mode `mode`.
    fn debug_mode_item(&mut self, arg: i32, mode: u8) -> i32 {
        if arg == CB_SELECT {
            self.sfx_forward();
            self.debug_to_mode(mode);
        }
        0
    }

    /// `fn_801B0A8C`, the debug menu's second and third pages' own callback: B goes back to
    /// the first page.
    #[doc(alias = "fn_801B0A8C")]
    pub fn debug_menu_back_to_first(&mut self, arg: i32) -> i32 {
        if arg == CB_BACK {
            self.sfx_back();
            self.debug_to_state(0);
        }
        0
    }

    /// `fn_801B09F8`, the debug menu's first page's own callback: B goes back to the title.
    #[doc(alias = "fn_801B09F8")]
    pub fn debug_menu_back_to_title(&mut self, arg: i32) -> i32 {
        if arg == CB_BACK {
            self.sfx_back();
            self.debug_to_mode(GM_TITLE);
        }
        0
    }

    /// `un_80300248`, Publicity: A, with it on, relocks the save.
    #[doc(alias = "un_80300248")]
    pub fn debug_publicity(&mut self, arg: i32) -> i32 {
        if self.game.debug_setting(4) != 0 && arg == CB_SELECT {
            self.sfx_forward();
            self.debug_relock_all();
        }
        0
    }

    /// `un_803001DC` (Versus Mode) and `un_803002FC` (Mode Team Test): the debug mode's
    /// second and third pages.
    #[doc(alias = "un_803001DC")]
    #[doc(alias = "un_803002FC")]
    pub fn debug_page_item(&mut self, arg: i32, state: u8) -> i32 {
        self.debug_state_item(arg, state)
    }

    /// `un_80300218`, Language: the saved language from the entry, whatever happened.
    #[doc(alias = "un_80300218")]
    pub fn debug_language(&mut self) -> i32 {
        let lang = self.game.debug_setting(0) as u8;
        self.game.game_data.set_saved_language(lang);
        self.game.sync_unlocks();
        0
    }
}
