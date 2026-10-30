//! The match's develop tools (`melee/db/*`): at the debug ROM's level (`DbLevel` 3 and up,
//! a developer setting, [`crate::game::session::Develop`]) a match's start sets them up
//! (`db_Setup`: `DbCo.dat`'s names, then each tool's debug text screen and state) and its
//! scene's pre-process runs them each frame (`db_RunEveryFrame`): each port's D-pad
//! combinations toggle the computer players' and the animations' readouts, the HUD and the
//! stage's visibility, the debug cameras, the sound's mix, the item menu and its spawns, the
//! bonuses' readout, the effect pools' limits and five-times speed. Below that level both
//! return at once, as on a retail disc.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::camera;
use super::hud::devtext::DevText;
use super::state::{Game, Running};
use crate::fpu::{mem, reg};
use crate::hsd::cobj;
use crate::hsd::gobj::GObjId;
use crate::input::pad::{
    BUTTON_A, BUTTON_B, BUTTON_DOWN, BUTTON_LEFT, BUTTON_RIGHT, BUTTON_START, BUTTON_UP, BUTTON_X,
    BUTTON_Y, TRIGGER_L, TRIGGER_R, TRIGGER_Z,
};
use crate::msl::Arg;
use crate::vec::Vec3;

/// `DbLKind_DebugRom`, `DbLKind_Develop`.
const DEBUG_ROM: i32 = super::gm::DB_DEBUG_ROM;
const DEVELOP: i32 = 4;

/// `HSD_GOBJ_PLINK_FIGHTER`, `_ITEM`, `_GROUND`.
const P_LINK_FIGHTER: u8 = 8;
const P_LINK_ITEM: u8 = 9;
const P_LINK_GROUND: u8 = 5;

/// `Ft_Kind_MasterH`, `Ft_Kind_CrezyH`.
const KIND_MASTER_HAND: i32 = 0x1B;
const KIND_CRAZY_HAND: i32 = 0x1C;

/// `It_Kind_M_Ball`, `It_Kind_L_Gun_Ray` (`It_Common_End`), `It_Kind_Kuriboh`,
/// `It_Kind_Octarock_Stone`, `It_Kind_Unk4` (a coin), `It_Kind_Old_Kuri`,
/// `It_Kind_Arwing_Laser`.
const IT_M_BALL: i32 = 0x22;
const IT_COMMON_END: i32 = 0x23;
const IT_KURIBOH: i32 = 0x2B;
const IT_OCTAROCK_STONE: i32 = 0x2F;
const IT_COIN: i32 = 0x9E;
const IT_OLD_KURI: i32 = 0xD0;
const IT_ARWING_LASER: i32 = 0xEA;

/// A port's buttons as the tools read them (`db_ButtonStates`): held, held the frame
/// before, pressed and released this frame, and the pad library's repeat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Buttons {
    pub current: u32,
    pub prev: u32,
    pub pressed: u32,
    pub released: u32,
    pub repeat: u32,
}

/// The tools' statics (`dbinit.c` to `dbscreenshot.c`). A screen is a debug text pool index
/// ([`super::hud::devtext::DevTexts`]); its glyphs live in the screen.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Db {
    /// `db_ButtonStates`.
    pub buttons: [Buttons; 4],
    /// `db_bonus_names`, `db_motionstate_names`, `db_submotion_names`: the name lists in
    /// `DbCo.dat`'s `dbLoadCommonData`.
    pub bonus_names: Option<DataRef>,
    pub motionstate_names: Option<DataRef>,
    pub submotion_names: Option<DataRef>,
    /// `db_CpuHandicapInfo.text`, `db_ShowCpuHandicapInfo`.
    pub cpu_text: Option<u8>,
    pub cpu_shown: bool,
    /// `db_AnimationInfo.text` and `db_804D6B48`: the fighters' collision bubbles (1 to 3),
    /// their other displays' bit (`MiscFighterVisualsStatus`, six bits) and the readout.
    pub anim_text: Option<u8>,
    pub fighter_bubbles: u8,
    pub fighter_visuals: u8,
    pub anim_shown: bool,
    /// `db_ItemAndPokemonMenuText`.
    pub item_text: Option<u8>,
    /// `dbsound.c`'s: the mix's toggles (`db_SoundToggles`), the readout's step
    /// (`db_ShowSoundInfo`), the voices' peaks and their timers (`db_SoundRelated_6B68` to
    /// `6B74`), its screen and `db_804D6B7C` (the debug text's GObj).
    pub sound_toggles: i32,
    pub sound_shown: i32,
    pub sound_peaks: [i32; 4],
    pub sound_text: Option<u8>,
    pub sound_gobj: Option<GObjId>,
    /// `db.MiscStageVisualsStatus`.
    pub stage_visuals: u32,
    /// `db_CameraInfoDisplay`, `db_CameraInfoDisplayTimer`, `db_ShowCameraInfo`.
    pub camera_text: Option<u8>,
    pub camera_timer: u8,
    pub camera_shown: u8,
    /// `db_804D6B98`: the port whose bonuses show (0xFF for none) and whether their screens
    /// take the second colours; `db_804D6B9C`, the screens' heap block, and its two screens.
    pub bonus_port: u8,
    pub bonus_colors: bool,
    pub bonus_block: u32,
    pub bonus_texts: [Option<u8>; 2],
    /// `db_804D6BA0`: the effects' and the particles' pool limits on.
    pub alloc_limits: [bool; 2],
    /// `db_5xSpeedStatus`.
    pub speed_5x: bool,
    /// `db_MiscVisualEffectsStatus`: the HUD's and the stage's display step (0 to 3).
    pub misc_effects: i32,
    /// `lbaudio_ax.c`'s `debug_enabled` (`lbAudioAx_80025098`), which nothing reads.
    pub audio_debug: bool,
}

/// `db_TextColors`: the bonus screens' background and text colours.
const BONUS_COLORS: [([u8; 4], [u8; 4]); 2] = [
    ([0x00, 0x00, 0x00, 0x00], [0xFF, 0x00, 0x00, 0xFF]),
    ([0x80, 0x40, 0xFF, 0x80], [0x00, 0x00, 0x00, 0xFF]),
];

/// `db_SoundToggleOrder`.
const SOUND_TOGGLE_ORDER: [i32; 4] = [3, 2, 0, 1];

/// `db_ItemNames`, `db_BarrelEnemies`, `db_AdventureEnemies` (the last 13 null), and
/// `db_PokemonNames`.
const ITEM_NAMES: [&[u8]; 35] = [
    b"Capsule ",
    b"Box     ",
    b"Taru    ",
    b"Egg     ",
    b"Kusudama",
    b"TaruCann",
    b"BombHei ",
    b"Dosei   ",
    b"Heart   ",
    b"Tomato  ",
    b"Star    ",
    b"Bat     ",
    b"Sword   ",
    b"Parasol ",
    b"G Shell ",
    b"R Shell ",
    b"L Gun   ",
    b"Freeze  ",
    b"Foods   ",
    b"MSBomb  ",
    b"Flipper ",
    b"S Scope ",
    b"StarRod ",
    b"LipStick",
    b"Harisen ",
    b"F Flower",
    b"Kinoko  ",
    b"DKinoko ",
    b"Hammer  ",
    b"WStar   ",
    b"ScBall  ",
    b"RabbitC ",
    b"MetalB  ",
    b"Spycloak",
    b"M Ball  ",
];
const BARREL_ENEMIES: [&[u8]; 4] = [b"Kuriboh ", b"Leadead ", b"Octarock", b"Ottosei "];
const ADVENTURE_ENEMIES: [&[u8]; 13] = [
    b"old-Kuri",
    b"Mato    ",
    b"Heiho   ",
    b"Nokonoko",
    b"Patapata",
    b"likelike",
    b"old-lead",
    b"old-octa",
    b"old-otto",
    b"whitebea",
    b"klap    ",
    b"zgshell ",
    b"zrshell ",
];
const POKEMON_NAMES: [&[u8]; 31] = [
    b"Random",
    b"Tosakinto",
    b"Chicorita",
    b"Kabigon",
    b"Kamex",
    b"Matadogas",
    b"Lizardon",
    b"Fire",
    b"Thunder",
    b"Freezer",
    b"Sonans",
    b"Hassam",
    b"Unknown",
    b"Entei",
    b"Raikou",
    b"Suikun",
    b"Kireihana",
    b"Marumine",
    b"Lugia",
    b"Houou",
    b"Metamon",
    b"Pippi",
    b"Togepy",
    b"Mew",
    b"Cerebi",
    b"Hitodeman",
    b"Lucky",
    b"Porygon2",
    b"Hinoarashi",
    b"Maril",
    b"Fushigibana",
];

/// The bonus screens' block (`sizeof(*db_804D6B9C) * 2`: a screen pointer and 1500 bytes of
/// glyphs, twice).
const BONUS_BLOCK: u32 = 2 * 0x5E0;

impl Game {
    /// `db_ButtonsDown`.
    #[doc(alias = "db_ButtonsDown")]
    pub fn db_down(&self, port: i32) -> u32 {
        self.db.buttons[port as usize].current
    }

    /// `db_ButtonsPressed`.
    #[doc(alias = "db_ButtonsPressed")]
    pub fn db_pressed(&self, port: i32) -> u32 {
        self.db.buttons[port as usize].pressed
    }

    /// `db_ButtonsRepeat`.
    #[doc(alias = "db_ButtonsRepeat")]
    pub fn db_repeat(&self, port: i32) -> u32 {
        self.db.buttons[port as usize].repeat
    }

    /// Whether `port` holds `held` and pressed `pressed` this frame, as the tools test.
    fn db_combo(&self, port: i32, held: u16, pressed: u16) -> bool {
        self.db_down(port) & u32::from(held) != 0 && self.db_pressed(port) & u32::from(pressed) != 0
    }

    fn text(&mut self, t: Option<u8>) -> &mut DevText {
        &mut self.hud.devtext.pool[usize::from(t.expect("a debug text screen"))]
    }

    /// A screen made for a tool: shown, its cursor hidden, its colours and scale.
    fn db_screen(
        &mut self,
        id: i8,
        (x, y, w, h): (i32, i32, i32, i32),
        bg: [u8; 4],
        fg: [u8; 4],
        scale: (f32, f32),
    ) -> Option<u8> {
        let t = self.hud.devtext.create(id, x, y, w, h)?;
        self.hud.devtext.show(t);
        let s = &mut self.hud.devtext.pool[usize::from(t)];
        s.hide_cursor();
        s.set_bg_color(bg);
        s.set_text_color(fg);
        s.set_scale(scale.0, scale.1);
        Some(t)
    }

    /// The fighters' GObjs, in their process link's order.
    fn fighter_gobjs(&self) -> Vec<GObjId> {
        self.link_gobjs(P_LINK_FIGHTER)
    }

    fn link_gobjs(&self, link: u8) -> Vec<GObjId> {
        let mut out = Vec::new();
        let mut cur = self.world.first_in(link);
        while let Some(g) = cur {
            out.push(g);
            cur = self.world.gobj(g).next;
        }
        out
    }

    /// `ftLib_IsMasterHandPresent`, `ftLib_IsCrazyHandPresent`.
    #[doc(alias = "ftLib_IsMasterHandPresent")]
    #[doc(alias = "ftLib_IsCrazyHandPresent")]
    fn hand_present(&self, kind: i32) -> bool {
        self.fighter_gobjs()
            .into_iter()
            .any(|g| self.fighters[self.fighter_id(g).0].kind == kind)
    }
}

impl Running<'_> {
    /// A name of a `DbCo.dat` list, as `%s` prints it (empty for a null entry).
    fn db_name(&self, list: Option<DataRef>, i: i32) -> Vec<u8> {
        let list = list.expect("DbCo.dat's names");
        let Some(s) = self.data.ptr(list.at(4 * i as u32)) else {
            return Vec::new();
        };
        let bytes = &self.data.get(s.archive).data()[s.offset as usize..];
        bytes.iter().copied().take_while(|&c| c != 0).collect()
    }

    /// The match's develop tools set up, `db_Setup`: at the debug ROM's level the ports'
    /// buttons cleared, `DbCo.dat`'s names, and each tool.
    #[doc(alias = "db_Setup")]
    pub fn db_setup(&mut self) {
        if self.game.gm.db_level < DEBUG_ROM {
            return;
        }
        self.game.db.buttons = Default::default();
        // `lbArchive_LoadSymbols`: from the disc into the scene's heap, never the cache.
        let (a, _, _) = self.load_archive_blocks("DbCo.dat");
        let root = self
            .data
            .get(a)
            .public("dbLoadCommonData")
            .map(|offset| DataRef { archive: a, offset })
            .expect("DbCo.dat's dbLoadCommonData");
        let db = &mut self.game.db;
        db.bonus_names = self.data.ptr(root);
        db.motionstate_names = self.data.ptr(root.at(4));
        db.submotion_names = self.data.ptr(root.at(8));
        self.db_setup_cpu();
        self.db_setup_anim();
        self.db_setup_item_menu();
        self.db_setup_sound();
        // `fn_SetupMiscStageVisuals`.
        self.game.db.stage_visuals = 0;
        // `fn_SetupCameraInfo`.
        let db = &mut self.game.db;
        db.camera_text = None;
        db.camera_timer = 0;
        db.camera_shown = 0;
        self.db_setup_bonus();
        // `fn_SetupObjAllocLimiter`, `fn_Setup5xSpeed`.
        self.db_setup_alloc_limiter();
        self.game.db.speed_5x = false;
    }

    /// The tools each frame, `db_RunEveryFrame` (the match's pre-process): each port's
    /// buttons (two ports' only with a hand boss present), the D-pad's diagonals cleared,
    /// then each tool's check for every port.
    #[doc(alias = "db_RunEveryFrame")]
    pub fn db_run_every_frame(&mut self) {
        if self.game.gm.db_level < DEBUG_ROM {
            return;
        }
        let ports = if self.game.hand_present(KIND_MASTER_HAND)
            || self.game.hand_present(KIND_CRAZY_HAND)
        {
            2
        } else {
            4
        };
        let (up, down) = (u32::from(BUTTON_UP), u32::from(BUTTON_DOWN));
        let (left, right) = (u32::from(BUTTON_LEFT), u32::from(BUTTON_RIGHT));
        let clear = |mut b: u32| {
            if b & up != 0 {
                if b & left != 0 {
                    b &= !(up | left);
                }
                if b & right != 0 {
                    b &= !(up | right);
                }
            }
            if b & down != 0 {
                if b & left != 0 {
                    b &= !(down | left);
                }
                if b & right != 0 {
                    b &= !(down | right);
                }
            }
            b
        };
        for i in 0..ports {
            let p = &self.game.pads.master[i];
            let (button, repeat) = (clear(p.button), clear(p.repeat));
            let s = &mut self.game.db.buttons[i];
            s.prev = s.current;
            s.current = button;
            s.pressed = s.current & (s.prev ^ s.current);
            s.released = s.prev & (s.prev ^ s.current);
            s.repeat = repeat;
        }
        for i in 0..4 {
            self.db_check_misc_effects(i);
        }
        for i in 0..4 {
            let b = self.game.db.buttons[i as usize];
            let p = &self.game.pads.master[i as usize];
            let (cx, cy) = (reg(p.nml_sub_stick_x), reg(p.nml_sub_stick_y));
            self.db_check_camera(i, b.current, b.pressed, cx, cy);
        }
        for i in 0..4 {
            self.db_check_sound(i);
        }
        for i in 0..4 {
            self.db_alloc_limiter(i);
        }
        for i in 0..4 {
            self.db_check_item_menu(i);
        }
        for i in 0..4 {
            self.db_check_cpu(i);
        }
        self.db_update_cpu();
        for i in 0..4 {
            self.db_check_anim(i);
        }
        self.db_update_anim();
        for i in 0..4 {
            self.db_check_stage_effects(i);
        }
        for i in 0..4 {
            self.db_check_bonus(i);
        }
        for i in 0..4 {
            // `fn_Check5xSpeed`.
            if self.game.db_combo(i, BUTTON_A, BUTTON_RIGHT) {
                self.db_toggle_5x();
            }
        }
    }

    /// `fn_Toggle5xSpeed`: five times the speed, or back to the match's.
    #[doc(alias = "fn_Toggle5xSpeed")]
    #[doc(alias = "fn_Check5xSpeed")]
    #[doc(alias = "fn_Setup5xSpeed")]
    #[doc(alias = "gm_ResetGameSpeed")]
    fn db_toggle_5x(&mut self) {
        self.game.db.speed_5x ^= true;
        if self.game.db.speed_5x {
            self.game.set_game_speed(5.0);
        } else {
            self.game.reset_game_speed();
        }
    }

    // The computer players' readout (`dbcpu.c`).

    /// `fn_SetupCpuHandicapInfo`: its screen (id 6), hidden behind nothing shown.
    #[doc(alias = "fn_SetupCpuHandicapInfo")]
    fn db_setup_cpu(&mut self) {
        let _ = self.game.hud.devtext.get_gobj();
        self.game.db.cpu_shown = false;
        self.game.db.cpu_text =
            self.game
                .db_screen(6, (20, 20, 60, 7), [0, 0, 0, 0], [0xFF; 4], (9.0, 12.0));
    }

    /// `fn_UpdateCpuHandicapInfo`: while shown, each slot's state, level, type, handicap and
    /// ratios.
    #[doc(alias = "fn_UpdateCpuHandicapInfo")]
    fn db_update_cpu(&mut self) {
        if !self.game.db.cpu_shown {
            return;
        }
        let t = self.game.db.cpu_text;
        let lines: Vec<[Arg; 7]> = (0..6)
            .map(|slot| {
                let p = self.game.player(slot);
                [
                    Arg::Int(p.player_state),
                    Arg::Int(i32::from(p.cpu_level)),
                    Arg::Int(i32::from(p.cpu_type)),
                    Arg::Int(i32::from(p.handicap)),
                    Arg::Float(reg(p.unk50)),
                    Arg::Float(reg(p.attack_ratio)),
                    Arg::Float(reg(p.defense_ratio)),
                ]
            })
            .collect();
        let s = self.game.text(t);
        s.erase();
        s.set_cursor_xy(0, 0);
        s.printf(b"A B  C D E    F    G", &[]);
        for l in &lines {
            s.printf(b"\n%d %d %2d %d %2.2f %2.2f %2.2f", l);
        }
    }

    /// `fn_CheckCpuHandicapInfo`: B and D-pad down show or hide it.
    #[doc(alias = "fn_CheckCpuHandicapInfo")]
    fn db_check_cpu(&mut self, port: i32) {
        if !self.game.db_combo(port, BUTTON_B, BUTTON_DOWN) {
            return;
        }
        let shown = !self.game.db.cpu_shown;
        self.game.db.cpu_shown = shown;
        let t = self.game.db.cpu_text;
        let s = self.game.text(t);
        if shown {
            s.show_background();
            s.show_text();
        } else {
            s.hide_background();
            s.hide_text();
        }
    }

    // The animations' readout and the fighters' displays (`dbanim.c`).

    /// `fn_SetupAnimationInfo`: its screen (id 7), the fighters' bubbles at 1.
    #[doc(alias = "fn_SetupAnimationInfo")]
    fn db_setup_anim(&mut self) {
        let _ = self.game.hud.devtext.get_gobj();
        let db = &mut self.game.db;
        db.fighter_bubbles = 1;
        db.anim_shown = false;
        db.fighter_visuals = 0;
        self.game.db.anim_text =
            self.game
                .db_screen(7, (20, 20, 60, 12), [0, 0, 0, 0], [0xFF; 4], (9.0, 12.0));
    }

    /// `fn_ToggleMiscFighterVisuals`: the fighters' next display (one bit, shifted along the
    /// six), and the items' stomp, pickup and coin ranges with its second, fourth and sixth.
    #[doc(alias = "fn_ToggleMiscFighterVisuals")]
    fn db_toggle_fighter_visuals(&mut self) {
        let v = &mut self.game.db.fighter_visuals;
        *v = if *v != 0 { (*v << 1) & 0x3F } else { 1 };
        let v = self.game.db.fighter_visuals;
        for g in self.game.fighter_gobjs() {
            let f = self.game.fighter_id(g);
            let b = &mut self.game.fighters[f.0].x21FC_flag[0];
            *b = (*b & 3) | ((v << 2) & !3);
        }
        if v & 2 != 0 {
            self.db_set_item_ranges(0, true);
        } else {
            self.db_set_item_ranges(0, false);
        }
        if v & 8 != 0 {
            self.db_set_item_ranges(1, true);
        } else {
            self.db_set_item_ranges(1, false);
        }
        if v & 0x20 != 0 {
            self.db_set_item_ranges(2, true);
        } else {
            self.db_set_item_ranges(2, false);
        }
    }

    /// `fn_UpdateAnimationInfo`: while shown, each fighter's player, state, animation and
    /// frame, and its interrupt flags.
    #[doc(alias = "fn_UpdateAnimationInfo")]
    fn db_update_anim(&mut self) {
        if !self.game.db.anim_shown {
            return;
        }
        let t = self.game.db.anim_text;
        let (states, subs) = (self.game.db.motionstate_names, self.game.db.submotion_names);
        struct Line {
            player: i32,
            motion: i32,
            anim: i32,
            frame: f64,
            flags: u8,
            motion_name: Vec<u8>,
            anim_name: Vec<u8>,
        }
        let lines: Vec<Line> = self
            .game
            .fighter_gobjs()
            .into_iter()
            .map(|g| {
                let fp = &self.game.fighters[self.game.fighter_id(g).0];
                let (motion, anim) = (fp.motion_id, fp.anim_id);
                Line {
                    player: i32::from(fp.player_idx),
                    motion,
                    anim,
                    frame: reg(fp.cur_anim_frame),
                    flags: fp.x221C_u16_y,
                    motion_name: if motion < 0x155 {
                        self.db_name(states, motion)
                    } else {
                        Vec::new()
                    },
                    anim_name: if anim != -1 && anim < 0x127 {
                        self.db_name(subs, anim)
                    } else {
                        Vec::new()
                    },
                }
            })
            .collect();
        let s = self.game.text(t);
        s.erase();
        s.set_cursor_xy(0, 0);
        for (i, l) in lines.iter().enumerate() {
            if i != 0 {
                s.printf(b"\n", &[]);
            }
            if l.motion < 0x155 {
                s.printf(b"%d %s", &[Arg::Int(l.player), Arg::Str(&l.motion_name)]);
            } else {
                s.printf(b"%d %d", &[Arg::Int(l.player), Arg::Int(l.motion)]);
            }
            s.set_cursor_x(23);
            if l.anim != -1 {
                if l.anim < 0x127 {
                    s.printf(b"%s", &[Arg::Str(&l.anim_name)]);
                } else {
                    s.printf(b"%d", &[Arg::Int(l.anim)]);
                }
            }
            s.set_cursor_x(44);
            s.printf(b"%03.2f", &[Arg::Float(l.frame)]);
            if l.flags != 0 {
                s.set_cursor_x(52);
                let f = i32::from(l.flags);
                for (bit, c) in [(1, &b"L"[..]), (2, b"R"), (4, b"T")] {
                    if l.flags & bit != 0 {
                        s.printf(c, &[Arg::Int(f)]);
                    } else {
                        s.printf(b" ", &[]);
                    }
                }
            }
        }
    }

    /// `fn_CheckAnimationInfo`: with R, D-pad up the fighters' next bubbles, right the
    /// port's fighter's and its items', left the next display; Y and down the readout; with
    /// Y, left a Super Mushroom (with A a Poison one) and right its end.
    #[doc(alias = "fn_CheckAnimationInfo")]
    fn db_check_anim(&mut self, port: i32) {
        if self.game.db_down(port) & u32::from(TRIGGER_R) != 0 {
            if self.game.db_pressed(port) & u32::from(BUTTON_UP) != 0 {
                let b = &mut self.game.db.fighter_bubbles;
                *b += 1;
                if *b > 3 {
                    *b = 1;
                }
                let b = self.game.db.fighter_bubbles;
                for g in self.game.fighter_gobjs() {
                    let f = self.game.fighter_id(g);
                    self.game.fighters[f.0].x21FC_flag[0] = b;
                }
            }
            if self.game.db_pressed(port) & u32::from(BUTTON_RIGHT) != 0
                && self.game.player_pkind(port) != 3
            {
                let g = self.game.player_entity(port).expect("the port's fighter");
                let f = self.game.fighter_id(g);
                let b = &mut self.game.fighters[f.0].x21FC_flag[0];
                let x = (*b & 3) + 1;
                *b = if x > 3 { 1 } else { x };
                // `fn_80225E6C`: the fighter's items take its setting.
                for it in self.game.link_gobjs(P_LINK_ITEM) {
                    if super::it::item(self.game, it).owner == Some(g) {
                        super::it::item::debug_owner_bubbles(self.game, it, Some(g));
                    }
                }
            }
            if self.game.db_pressed(port) & u32::from(BUTTON_LEFT) != 0 {
                self.db_toggle_fighter_visuals();
            }
        }
        if self.game.db_combo(port, BUTTON_Y, BUTTON_DOWN) {
            let shown = !self.game.db.anim_shown;
            self.game.db.anim_shown = shown;
            let t = self.game.db.anim_text;
            let s = self.game.text(t);
            if shown {
                s.show_background();
                s.show_text();
            } else {
                s.hide_background();
                s.hide_text();
            }
        }
        if self.game.db_down(port) & u32::from(BUTTON_Y) != 0 {
            let a = self.game.db_down(port) & u32::from(BUTTON_A) != 0;
            if self.game.db_pressed(port) & u32::from(BUTTON_LEFT) != 0
                && self.game.player_pkind(port) != 3
            {
                let g = self.game.player_entity(port).expect("the port's fighter");
                if a {
                    self.apply_poison_mushroom(g);
                } else {
                    self.apply_super_mushroom(g);
                }
            }
            if self.game.db_pressed(port) & u32::from(BUTTON_RIGHT) != 0
                && self.game.player_pkind(port) != 3
            {
                let g = self.game.player_entity(port).expect("the port's fighter");
                if a {
                    self.poison_mushroom_end(g);
                } else {
                    self.super_mushroom_end(g);
                }
            }
        }
    }

    // The HUD and the stage (`dbeffect.c`, `dbcamera.c`'s stage effects).

    /// `fn_CheckMiscVisualEffects`: X and D-pad down step the display (0 to 3): the HUD
    /// shown at 0, hidden after.
    #[doc(alias = "fn_CheckMiscVisualEffects")]
    fn db_check_misc_effects(&mut self, port: i32) {
        if !self.game.db_combo(port, BUTTON_X, BUTTON_DOWN) {
            return;
        }
        let s = &mut self.game.db.misc_effects;
        *s += 1;
        if *s > 3 {
            *s = 0;
        }
        if self.game.db.misc_effects == 0 {
            self.game.show_hud();
        } else if !self.game.is_hud_hidden() {
            self.game.hide_hud();
        }
    }

    /// `fn_802270C4`: the fighters' shadows on or off.
    #[doc(alias = "fn_802270C4")]
    fn db_fighter_shadows(&mut self, on: bool) {
        for g in self.game.fighter_gobjs() {
            let f = self.game.fighter_id(g);
            if self.game.fighters[f.0].x20A4.shadow.is_some() {
                self.game.fighters[f.0].x20A4.x0_b5 = !on;
                self.shadow_update(g);
            }
        }
    }

    /// `fn_8022713C`: every stage object's flag `x10_flags.b7`.
    #[doc(alias = "fn_8022713C")]
    fn db_ground_flag(&mut self, on: bool) {
        for g in self.game.link_gobjs(P_LINK_GROUND) {
            // The link's other GObjs hold no stage part (`ground != NULL`).
            if let Some(super::state::UserData::Ground(gr)) = self.game.world.gobjs[g.0].user_data {
                self.game.grounds[gr.0].x10_flags.b7 = on;
            }
        }
    }

    /// `fn_CheckMiscStageEffects`: R and D-pad down step the stage's debug displays (seven
    /// steps: shadows, the camera's flags, the stage objects' flag); X and down the stage
    /// shown again in the display's first two steps (with the fog's background colour),
    /// hidden in the others.
    #[doc(alias = "fn_CheckMiscStageEffects")]
    #[doc(alias = "fn_SetupMiscStageVisuals")]
    fn db_check_stage_effects(&mut self, port: i32) {
        if self.game.db_combo(port, TRIGGER_R, BUTTON_DOWN) {
            self.game.db.stage_visuals = self.game.db.stage_visuals.wrapping_add(1);
            // Each step: shadows, then the camera's shadows, collisions and zones, the
            // stage's flag, the camera's two flags and its special points.
            let step = match self.game.db.stage_visuals {
                1 => (false, false, false, false, false, false, false, false),
                2 => (true, true, false, true, false, false, false, true),
                3 => (false, false, false, true, true, false, false, true),
                4 => (false, false, true, true, true, false, false, true),
                5 => (false, false, true, true, true, true, false, true),
                6 => (false, false, true, true, true, false, true, true),
                _ => (true, false, false, false, false, false, false, false),
            };
            self.db_fighter_shadows(step.0);
            let c = &mut self.game.camera.cam;
            camera::set_shadows(c, step.1);
            camera::set_draw_collisions(c, step.2);
            camera::set_draw_zones(c, step.3);
            self.db_ground_flag(step.4);
            let c = &mut self.game.camera.cam;
            camera::set_x39a_b0(c, step.5);
            camera::set_x39a_b1(c, step.6);
            camera::set_draw_special_points(c, step.7);
            if !(1..=6).contains(&self.game.db.stage_visuals) {
                self.game.db.stage_visuals = 0;
            }
        }
        if self.game.db_combo(port, BUTTON_X, BUTTON_DOWN) {
            match self.game.db.misc_effects {
                0 | 1 => {
                    super::gr::ground::apply_background_color(self.game);
                    camera::set_stage_visible(&mut self.game.camera.cam, 1);
                }
                _ => camera::set_stage_visible(&mut self.game.camera.cam, 0),
            }
        }
    }

    // The debug cameras' tools (`dbcamera.c`).

    /// `fn_CheckCameraInfo_helper`: the camera's readout's screen (id 12), hidden, made once
    /// the debug text has its GObj.
    fn db_camera_screen(&mut self) {
        if self.game.db.camera_text.is_some() || self.game.hud.devtext.get_gobj().is_none() {
            return;
        }
        self.game.db.camera_text = self.game.db_screen(
            12,
            (420, 360, 32, 3),
            [0, 0, 0, 0x80],
            [0xFF; 4],
            (12.0, 16.0),
        );
        if let Some(t) = self.game.db.camera_text {
            let s = &mut self.game.hud.devtext.pool[usize::from(t)];
            s.hide_background();
            s.hide_text();
        }
    }

    /// `fn_CheckCameraInfo`: outside training and Camera Mode, the C stick past 0.6 or D-pad
    /// up alone takes the free debug camera (D-pad up again the follow camera, then the
    /// match's); the debug camera moves by the port's C stick; B toggles its readout in the
    /// stage displays' third and fourth steps; X and D-pad down the background white or
    /// black in the display's last two steps.
    #[doc(alias = "fn_CheckCameraInfo")]
    #[doc(alias = "fn_SetupCameraInfo")]
    fn db_check_camera(&mut self, port: i32, down: u32, pressed: u32, cx: f64, cy: f64) {
        let abs = |v: f64| if v < 0.0 { fp::fneg(v) } else { v };
        let threshold = reg(0.6);
        if !super::gm::is_training(self.game)
            && self.game.game_mode != super::gm::camera_mode::GM_CAMERA_MODE
        {
            let c = &self.game.camera.cam;
            if !camera::is_debug_free(c)
                && !camera::is_debug_follow(c)
                && (abs(cx) > threshold || abs(cy) > threshold)
            {
                self.db_camera_screen();
                camera::debug_free(self.game, self.data);
            } else if pressed & u32::from(BUTTON_UP) != 0 {
                self.db_camera_screen();
                let mods = u32::from(BUTTON_Y | BUTTON_X | TRIGGER_L | TRIGGER_R);
                if down & mods == 0 {
                    if camera::is_debug_free(&self.game.camera.cam) {
                        camera::debug_follow(self.game, self.data, port);
                    } else if camera::is_debug_follow(&self.game.camera.cam) {
                        camera::leave_debug(&mut self.game.camera);
                    } else {
                        camera::debug_free(self.game, self.data);
                    }
                }
            }
        }
        if camera::is_debug_follow(&self.game.camera.cam) {
            self.db_follow_camera_input(port);
        } else if camera::is_debug_free(&self.game.camera.cam) {
            self.db_free_camera_input(port);
        }
        if camera::is_debug_free(&self.game.camera.cam)
            && self.game.db.stage_visuals.wrapping_sub(3) <= 1
        {
            if pressed & u32::from(BUTTON_B) != 0 {
                self.game.db.camera_shown = u8::from(self.game.db.camera_shown == 0);
            }
        } else {
            self.game.db.camera_shown = 0;
        }
        self.db_update_camera_info();
        if self.game.db_combo(port, BUTTON_X, BUTTON_DOWN) {
            match self.game.db.misc_effects {
                2 => camera::set_background_color(&mut self.game.camera.cam, 0xFF, 0xFF, 0xFF),
                3 => camera::set_background_color(&mut self.game.camera.cam, 0, 0, 0),
                _ => {}
            }
        }
    }

    /// `fn_80227188`: while shown and its timer runs, the camera's eye, interest, field of
    /// view and pitch (each coordinate -1 past 99999); hidden as the timer ends.
    #[doc(alias = "fn_80227188")]
    fn db_update_camera_info(&mut self) {
        let Some(t) = self.game.db.camera_text else {
            return;
        };
        let gobj = camera::gobj(&self.game.camera.cam);
        if gobj.is_none() || self.game.db.camera_shown == 0 {
            let s = &mut self.game.hud.devtext.pool[usize::from(t)];
            s.hide_background();
            s.hide_text();
            return;
        }
        if self.game.db.camera_timer > 1 {
            let c = camera::camera_cobj(self.game);
            let eye = cobj::eye_position(&mut self.game.objs, self.data, c).expect("the eye");
            let int = cobj::interest(&mut self.game.objs, self.data, c).expect("the interest");
            let fov = reg(cobj::fov(&self.game.objs.cobjs, Some(c)));
            let dy = fp::fsubs(reg(int.y), reg(eye.y));
            let dz = fp::fneg(fp::fsubs(reg(int.z), reg(eye.z)));
            let ang = fp::fmuls(
                57.295_780_181_884_766,
                reg(crate::math::atan2(mem(dy), mem(dz))),
            );
            let coord = |v: f32| {
                let r = reg(v);
                let a = if r < 0.0 { fp::fneg(r) } else { r };
                if a > 99999.0 { -1 } else { fp::fctiwz(r) }
            };
            let (ex, ey, ez) = (coord(eye.x), coord(eye.y), coord(eye.z));
            let (ix, iy, iz) = (coord(int.x), coord(int.y), coord(int.z));
            let s = &mut self.game.hud.devtext.pool[usize::from(t)];
            s.erase();
            s.set_cursor_xy(0, 0);
            s.printf(b"EYE %d,%d,%d", &[Arg::Int(ex), Arg::Int(ey), Arg::Int(ez)]);
            s.set_cursor_xy(0, 1);
            s.printf(b"INT %d,%d,%d", &[Arg::Int(ix), Arg::Int(iy), Arg::Int(iz)]);
            s.set_cursor_xy(0, 2);
            s.printf(
                b"FOV %d  ANG %d",
                &[Arg::Int(fp::fctiwz(fov)), Arg::Int(fp::fctiwz(ang))],
            );
            s.show_background();
            s.show_text();
        } else if self.game.db.camera_timer == 1 {
            let s = &mut self.game.hud.devtext.pool[usize::from(t)];
            s.hide_background();
            s.hide_text();
        } else {
            return;
        }
        self.game.db.camera_timer = self.game.db.camera_timer.wrapping_sub(1);
    }

    /// The port's C stick as the debug cameras read it: each axis 0 below 0.2.
    fn db_camera_stick(&self, port: i32) -> (f64, f64, u32) {
        let p = &self.game.pads.master[(port & 0xFF) as usize];
        let threshold = |v: f32| {
            let r = reg(v);
            let a = if r < 0.0 { fp::fneg(r) } else { r };
            if a < reg(0.2) { 0.0 } else { r }
        };
        (
            threshold(p.nml_sub_stick_x),
            threshold(p.nml_sub_stick_y),
            p.button,
        )
    }

    /// `fn_802277E8`: the follow camera's input: with D-pad left its distance, with right
    /// its offset, else its orbit, around the followed fighter's camera point.
    #[doc(alias = "fn_802277E8")]
    #[doc(alias = "Camera_8003019C")]
    fn db_follow_camera_input(&mut self, port: i32) {
        let (cx, cy, buttons) = self.db_camera_stick(port);
        let b = camera::debug_followed(self.game);
        let at = self.game.camera.boxes[b].bone_pos;
        if buttons & u32::from(BUTTON_LEFT) != 0 {
            self.db_follow_dolly(at, cy);
        } else if buttons & u32::from(BUTTON_RIGHT) != 0 {
            self.db_follow_pan(at, fp::fneg(cx), fp::fneg(cy));
        } else {
            self.db_follow_orbit(at, cx, cy);
        }
    }

    /// `fn_80227904`: the free camera's input: with D-pad left its distance, with right its
    /// position, else its orbit; its readout's timer at 60.
    #[doc(alias = "fn_80227904")]
    fn db_free_camera_input(&mut self, port: i32) {
        let (cx, cy, buttons) = self.db_camera_stick(port);
        if buttons & u32::from(BUTTON_LEFT) != 0 {
            self.db_free_dolly(cy);
        } else if buttons & u32::from(BUTTON_RIGHT) != 0 {
            self.db_free_pan(fp::fneg(cx), fp::fneg(cy));
        } else if cx != 0.0 || cy != 0.0 {
            // `fn_80227B64`.
            let d = &self.game.camera.debug;
            let (eye, int) = (d.free_eye_pos, d.free_int_pos);
            let eye = self.db_orbit(int, eye, cx, cy);
            self.game.camera.debug.free_eye_pos = eye;
        }
        self.game.db.camera_timer = 60;
    }

    /// The camera's CObj and its eye's direction (`HSD_CObjGetEyeVector`).
    fn db_eye_vector(&mut self) -> (crate::hsd::cobj::CObjId, Vec3) {
        let c = camera::camera_cobj(self.game);
        let v = cobj::eye_vector(&mut self.game.objs, self.data, Some(c)).expect("the eye");
        (c, v)
    }

    /// `HSD_CObjGetEyeDistance`.
    fn db_eye_distance(&mut self, c: crate::hsd::cobj::CObjId) -> f32 {
        cobj::eye_distance(&mut self.game.objs, self.data, Some(c))
    }

    /// `fn_802279E8`: the eye (`pos`) orbited around `interest` by the stick, two degrees a
    /// frame each way, its pitch kept from 1 to 179 degrees; `pos` unchanged with the stick
    /// centred.
    #[doc(alias = "fn_802279E8")]
    #[doc(alias = "fn_80227B64")]
    fn db_orbit(&mut self, interest: Vec3, pos: Vec3, cx: f64, cy: f64) -> Vec3 {
        if cx == 0.0 && cy == 0.0 {
            return pos;
        }
        let up = Vec3::new(0.0, 1.0, 0.0);
        let (c, fwd) = self.db_eye_vector();
        let pitch = fp::fmuls(
            57.295_780_181_884_766,
            reg(crate::math::acos(fwd.sdk_dot(up))),
        );
        let mut new_pitch = fp::fmuls(2.0, cy);
        if fp::fadds(pitch, new_pitch) > 179.0 {
            new_pitch = fp::fsubs(179.0, pitch);
        }
        if fp::fadds(pitch, new_pitch) < 1.0 {
            new_pitch = fp::fsubs(1.0, pitch);
        }
        let axis = up.sdk_cross(fwd).sdk_normalize();
        let m = crate::mtx::Mtx34::rotation_about(
            axis,
            mem(fp::fmuls(0.017_453_292_384_743_69, new_pitch)),
        );
        let fwd = m.transform(fwd);
        let dist = self.db_eye_distance(c);
        let m = crate::mtx::Mtx34::rotation_about(
            up,
            mem(fp::fmuls(0.017_453_292_384_743_69, fp::fmuls(2.0, cx))),
        );
        let fwd = m.transform(fwd).sdk_scale(dist);
        interest.sdk_sub(fwd)
    }

    /// The followed camera's eye and interest from its offsets around `at` (the struct's
    /// names swapped, as `Camera_8002FEEC` sets them).
    fn db_follow_place(&mut self, at: Vec3) {
        let d = &mut self.game.camera.debug;
        d.follow_eye_pos = d.follow_int_offset + at;
        d.follow_int_pos = d.follow_eye_offset + at;
    }

    /// `fn_80227BA8` for the follow camera: its eye offset orbited around its interest
    /// offset, then placed.
    #[doc(alias = "fn_80227BA8")]
    fn db_follow_orbit(&mut self, at: Vec3, cx: f64, cy: f64) {
        let d = &self.game.camera.debug;
        let (eye, int) = (d.follow_eye_offset, d.follow_int_offset);
        let eye = self.db_orbit(int, eye, cx, cy);
        self.game.camera.debug.follow_eye_offset = eye;
        self.db_follow_place(at);
    }

    /// `1 - 0.05 * stick` of the eye's distance along its direction.
    fn db_dolly(&mut self, stick: f64) -> Vec3 {
        let (c, fwd) = self.db_eye_vector();
        let dist = self.db_eye_distance(c);
        let s = fp::fmuls(reg(dist), fp::fnmsubs(0.050_000_000_745_058_06, stick, 1.0));
        fwd.sdk_scale(mem(s))
    }

    /// `fn_80227D38` for the follow camera: its eye offset nearer or farther, then placed.
    #[doc(alias = "fn_80227D38")]
    fn db_follow_dolly(&mut self, at: Vec3, cy: f64) {
        if cy != 0.0 {
            let v = self.db_dolly(cy);
            let d = &mut self.game.camera.debug;
            d.follow_eye_offset = d.follow_int_offset.sdk_sub(v);
        }
        self.db_follow_place(at);
    }

    /// `fn_80227CAC`: the free camera's eye nearer or farther.
    #[doc(alias = "fn_80227CAC")]
    fn db_free_dolly(&mut self, cy: f64) {
        if cy != 0.0 {
            let v = self.db_dolly(cy);
            let d = &mut self.game.camera.debug;
            d.free_eye_pos = d.free_int_pos.sdk_sub(v);
        }
    }

    /// The pan's scale: 3% of the view's height at the eye's distance.
    fn db_pan_scale(&mut self) -> (crate::hsd::cobj::CObjId, f64) {
        let c = camera::camera_cobj(self.game);
        let dist = self.db_eye_distance(c);
        let fov = reg(cobj::fov(&self.game.objs.cobjs, Some(c)));
        let half = fp::fdivs(fp::fmuls(0.017_453_292_384_743_69, fov), 2.0);
        let tan = reg(crate::math::tan(mem(half)));
        let s = fp::fmuls(
            0.029_999_999_329_447_746,
            fp::fmuls(2.0, fp::fmuls(reg(dist), tan)),
        );
        (c, s)
    }

    /// The pan's moves along the camera's left and up vectors, for `x` and `y`.
    fn db_pan_moves(&mut self, x: f64, y: f64) -> (Option<Vec3>, Option<Vec3>) {
        let (c, s) = self.db_pan_scale();
        let left = (x != 0.0).then(|| {
            let v = cobj::left_vector(&mut self.game.objs, self.data, Some(c)).expect("left");
            v.sdk_scale(mem(fp::fmuls(s, x)))
        });
        let up = (y != 0.0).then(|| {
            let v = cobj::up_vector(&mut self.game.objs, self.data, Some(c)).expect("up");
            v.sdk_scale(mem(fp::fmuls(fp::fneg(s), y)))
        });
        (left, up)
    }

    /// `fn_80228124` for the follow camera: both offsets moved, then placed.
    #[doc(alias = "fn_80228124")]
    fn db_follow_pan(&mut self, at: Vec3, x: f64, y: f64) {
        let (left, up) = self.db_pan_moves(x, y);
        let d = &mut self.game.camera.debug;
        for v in [left, up].into_iter().flatten() {
            d.follow_int_offset = d.follow_int_offset.sdk_add(v);
            d.follow_eye_offset = d.follow_eye_offset.sdk_add(v);
        }
        self.db_follow_place(at);
    }

    /// `fn_80227FE0`: the free camera's eye and interest moved together.
    #[doc(alias = "fn_80227FE0")]
    fn db_free_pan(&mut self, x: f64, y: f64) {
        if x == 0.0 && y == 0.0 {
            return;
        }
        let (left, up) = self.db_pan_moves(x, y);
        let d = &mut self.game.camera.debug;
        for v in [left, up].into_iter().flatten() {
            d.free_int_pos = d.free_int_pos.sdk_add(v);
            d.free_eye_pos = d.free_eye_pos.sdk_add(v);
        }
    }

    // The sound's mix and its readout (`dbsound.c`).

    /// `fn_SetupSoundInfo`: its screen (id 9), hidden.
    #[doc(alias = "fn_SetupSoundInfo")]
    fn db_setup_sound(&mut self) {
        let gobj = self.game.hud.devtext.get_gobj();
        let db = &mut self.game.db;
        db.sound_gobj = gobj;
        db.sound_toggles = 0;
        db.sound_shown = 0;
        db.sound_peaks = [0; 4];
        self.game.db.sound_text =
            self.game
                .db_screen(9, (420, 60, 18, 3), [0x80; 4], [0xFF; 4], (12.0, 16.0));
        if let Some(t) = self.game.db.sound_text {
            let s = &mut self.game.hud.devtext.pool[usize::from(t)];
            s.hide_background();
            s.hide_text();
        }
    }

    /// `fn_CheckSoundInfo`: on the first port, X with D-pad left steps the mix (both on,
    /// effects only, neither, music only) and the readout; then the readout
    /// (`fn_UpdateSoundInfo`).
    #[doc(alias = "fn_CheckSoundInfo")]
    fn db_check_sound(&mut self, port: i32) {
        let orig = self.game.db.sound_toggles;
        if port == 0 {
            let (x, left) = (u32::from(BUTTON_X), u32::from(BUTTON_LEFT));
            let step = if self.game.db_pressed(port) & x != 0 {
                self.game.db_down(port) & left != 0
            } else {
                self.game.db_pressed(port) & left != 0 && self.game.db_down(port) & x != 0
            };
            if step {
                let db = &mut self.game.db;
                db.sound_toggles = (db.sound_toggles + 1) % 4;
                db.sound_shown = (db.sound_shown + 1) % 8;
            }
            if self.game.db.sound_toggles != orig {
                let t = SOUND_TOGGLE_ORDER[self.game.db.sound_toggles as usize];
                self.db_sound_mix(t & 1 != 0, t & 2 != 0);
            }
        }
        self.db_update_sound();
    }

    /// `lbAudioAx_80025064`: the music (`bgm`) and the sound effects (`fgm`) on or off.
    #[doc(alias = "lbAudioAx_80025064")]
    fn db_sound_mix(&mut self, bgm: bool, fgm: bool) {
        // `lbl_804D38C4` scales the announcer's channel, `lbl_804D38C8` the effects'.
        let lb = &mut self.game.audio.lb;
        lb.voice_scale = if bgm { 1.0 } else { 0.0 };
        lb.fx_scale = if fgm { 1.0 } else { 0.0 };
    }

    /// `fn_UpdateSoundInfo`: the voices' readout in the last four of its eight steps: the
    /// mix, and the effects' and the active list's voices with their peaks, each held 240
    /// frames.
    #[doc(alias = "fn_UpdateSoundInfo")]
    #[doc(alias = "lbAudioAx_80025098")]
    #[doc(alias = "lbAudioAx_80028B2C")]
    #[doc(alias = "lbAudioAx_80028B4C")]
    #[doc(alias = "AXDriver_8038E5D4")]
    #[doc(alias = "AXDriver_8038E5DC")]
    fn db_update_sound(&mut self) {
        let shown = self.game.db.sound_shown;
        self.game.db.audio_debug = shown > 3;
        let Some(t) = self.game.db.sound_text else {
            let d = &self.game.audio.driver;
            let (playing, linked) = (d.playing, d.linked);
            let p = &mut self.game.db.sound_peaks;
            *p = [playing, 0, linked, 0];
            return;
        };
        if shown <= 3 {
            let s = &mut self.game.hud.devtext.pool[usize::from(t)];
            s.hide_background();
            s.hide_text();
            return;
        }
        let mut x = self.game.audio.driver.playing;
        let p = &mut self.game.db.sound_peaks;
        if p[0] < x {
            p[0] = x;
            p[1] = 0xF0;
        }
        p[1] -= 1;
        if p[1] <= 0 {
            p[0] = x;
            p[1] = 0;
        }
        x = self.game.audio.driver.linked;
        if p[2] < x {
            p[2] = x;
            p[3] = 0xF0;
        }
        p[3] -= 1;
        if p[3] <= 0 {
            p[2] = x;
            p[3] = 0;
        }
        let peaks = *p;
        let mix = SOUND_TOGGLE_ORDER[self.game.db.sound_toggles as usize] & 3;
        let s = &mut self.game.hud.devtext.pool[usize::from(t)];
        s.erase();
        s.set_cursor_xy(1, 0);
        s.printf(
            match mix {
                0 => b"FGM:OFF  BGM:OFF",
                1 => b"FGM:OFF  BGM:ON ",
                2 => b"FGM:ON   BGM:OFF",
                _ => b"FGM:ON   BGM:ON ",
            },
            &[],
        );
        // Both lines print the active list's count first: the original passes `x` after its
        // second read to each (Melee's bugs).
        s.set_cursor_xy(1, 1);
        s.printf(b"PVoice  %3d  %3d\n", &[Arg::Int(x), Arg::Int(peaks[0])]);
        s.set_cursor_xy(1, 2);
        s.printf(b"VVoice  %3d  %3d", &[Arg::Int(x), Arg::Int(peaks[2])]);
        s.show_background();
        s.show_text();
    }

    // The item menu and its spawns (`dbitem.c`).

    /// `fn_SetupItemAndPokemonMenu`: the menu hidden, a Poké Ball and a random Pokémon
    /// picked, every bubble shown, no range.
    #[doc(alias = "fn_SetupItemAndPokemonMenu")]
    fn db_setup_item_menu(&mut self) {
        let m = &mut self.game.it.db_menu;
        m.DisplayStatus = 0;
        m.CurrentlySelectedItem = IT_M_BALL;
        m.LastSelectedItem = m.CurrentlySelectedItem;
        m.CurrentlySelectedPokemon = 0;
        m.LastSelectedPokemon = m.CurrentlySelectedPokemon;
        self.game.it.db_bubbles = 1;
        m.ShowEnemyStompRange = false;
        m.ShowItemPickupRange = false;
    }

    /// The items' range displays on or off (`which`: 0 the enemies' stomp range, 1 the
    /// pickup range, 2 the coins'): `fn_EnableShowEnemyStompRange` and the five others.
    #[doc(alias = "fn_EnableShowEnemyStompRange")]
    #[doc(alias = "fn_DisableShowEnemyStompRange")]
    #[doc(alias = "fn_EnableShowItemPickupRange")]
    #[doc(alias = "fn_DisableShowItemPickupRange")]
    #[doc(alias = "fn_EnableShowCoinPickupRange")]
    #[doc(alias = "fn_DisableShowCoinPickupRange")]
    fn db_set_item_ranges(&mut self, which: u8, on: bool) {
        for g in self.game.link_gobjs(P_LINK_ITEM) {
            let ip = super::it::item_mut(self.game, g);
            let flags = &mut ip.xDAA[0];
            match (which, on) {
                // `xDAA_flag.x0.b3`, for an enemy (`xDD0_flag.x0.b0`) when shown.
                (0, true) => {
                    if ip.xDD0_flag[0] & 0x80 != 0 {
                        ip.xDAA[0] |= 0x10;
                    }
                }
                (0, false) => *flags &= !0x10,
                // `b4`.
                (1, true) => *flags |= 0x08,
                (1, false) => *flags &= !0x08,
                // `b0`, for a coin.
                (2, on) => {
                    if ip.kind == IT_COIN {
                        if on {
                            ip.xDAA[0] |= 0x80;
                        } else {
                            ip.xDAA[0] &= !0x80;
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
        let m = &mut self.game.it.db_menu;
        match which {
            0 => m.ShowEnemyStompRange = on,
            1 => m.ShowItemPickupRange = on,
            _ => m.ShowCoinPickupRange = on,
        }
    }

    /// `fn_ToggleItemCollisionBubbles`: the items' next bubbles (1 to 3), for every item.
    #[doc(alias = "fn_ToggleItemCollisionBubbles")]
    fn db_toggle_item_bubbles(&mut self) {
        let b = &mut self.game.it.db_bubbles;
        *b += 1;
        if *b > 3 {
            *b = 1;
        }
        let b = self.game.it.db_bubbles as u8;
        for g in self.game.link_gobjs(P_LINK_ITEM) {
            let f = &mut super::it::item_mut(self.game, g).xDAA[0];
            *f &= 0xFC;
            *f |= b;
        }
    }

    /// `db_HandleItemPokemonMenuInput`: with L, D-pad up and down (repeating) the item
    /// (the common items, the barrels' enemies, the adventure's), right and left the
    /// Pokémon.
    #[doc(alias = "db_HandleItemPokemonMenuInput")]
    fn db_item_menu_input(&mut self, port: i32) {
        let l = self.game.db_down(port) & u32::from(TRIGGER_L) != 0;
        let rep = self.game.db_repeat(port);
        let m = &mut self.game.it.db_menu;
        let item = &mut m.CurrentlySelectedItem;
        if l && rep & u32::from(BUTTON_UP) != 0 {
            if *item < IT_COMMON_END {
                *item += 1;
                if *item == IT_COMMON_END {
                    *item = IT_KURIBOH;
                }
            } else if *item < IT_OCTAROCK_STONE {
                *item += 1;
                if *item == IT_OCTAROCK_STONE {
                    *item = IT_OLD_KURI;
                }
            } else if *item < IT_ARWING_LASER - 1 {
                *item += 1;
            }
        }
        if l && rep & u32::from(BUTTON_DOWN) != 0 {
            if *item >= IT_OLD_KURI {
                *item -= 1;
                if *item < IT_OLD_KURI {
                    *item = IT_OCTAROCK_STONE - 1;
                }
            } else if *item >= IT_KURIBOH {
                *item -= 1;
                if *item < IT_KURIBOH {
                    *item = IT_COMMON_END - 1;
                }
            } else if *item > 0 {
                *item -= 1;
            }
        }
        let p = &mut m.CurrentlySelectedPokemon;
        if l && rep & u32::from(BUTTON_RIGHT) != 0 && *p < 0x1E {
            *p += 1;
        }
        if l && rep & u32::from(BUTTON_LEFT) != 0 && *p > 0 {
            *p -= 1;
        }
    }

    /// `fn_ShowOrCreateItemAndPokemonMenu`: the menu's screen shown again, or made (id 8).
    #[doc(alias = "fn_ShowOrCreateItemAndPokemonMenu")]
    fn db_show_item_menu(&mut self) {
        let _ = self.game.hud.devtext.get_gobj();
        if self.game.it.db_menu.DisplayStatus == 2 {
            let t = self.game.db.item_text;
            let s = self.game.text(t);
            s.show_background();
            s.show_text();
        } else {
            self.game.db.item_text = self.game.db_screen(
                8,
                (20, 20, 40, 1),
                [0x80, 0x40, 0xFF, 0xFF],
                [0, 0, 0, 0xFF],
                (12.0, 16.0),
            );
        }
        self.game.it.db_menu.DisplayStatus = 1;
    }

    /// `fn_UpdateItemAndPokemonMenu`: the port's input, and on a change the menu shown for
    /// 120 frames with the item's and the Pokémon's names.
    #[doc(alias = "fn_UpdateItemAndPokemonMenu")]
    fn db_update_item_menu(&mut self, port: i32) {
        let m = &mut self.game.it.db_menu;
        m.LastSelectedItem = m.CurrentlySelectedItem;
        m.LastSelectedPokemon = m.CurrentlySelectedPokemon;
        self.db_item_menu_input(port);
        let m = &mut self.game.it.db_menu;
        if m.LastSelectedItem == m.CurrentlySelectedItem
            && m.LastSelectedPokemon == m.CurrentlySelectedPokemon
        {
            return;
        }
        m.Player = port;
        m.DisplayFadeTimer = 0x78;
        if m.DisplayStatus != 1 {
            self.db_show_item_menu();
        }
        let m = &self.game.it.db_menu;
        let i = m.CurrentlySelectedItem;
        let item: &[u8] = if i < IT_COMMON_END {
            ITEM_NAMES[i as usize]
        } else if i < IT_OCTAROCK_STONE {
            BARREL_ENEMIES[(i - IT_KURIBOH) as usize]
        } else if i < IT_ARWING_LASER {
            ADVENTURE_ENEMIES
                .get((i - IT_OLD_KURI) as usize)
                .copied()
                .unwrap_or(b"")
        } else {
            panic!("the debug item menu's item {i:#x} hangs the original (`while (1)`)")
        };
        let pokemon = POKEMON_NAMES[m.CurrentlySelectedPokemon as usize];
        let t = self.game.db.item_text;
        let s = self.game.text(t);
        s.erase();
        s.set_cursor_xy(0, 0);
        s.printf(
            b"Item-> %s  Pokemon-> %s",
            &[Arg::Str(item), Arg::Str(pokemon)],
        );
    }

    /// `db_CheckAndSpawnItem`: D-pad down with nothing else held makes the menu's item 60
    /// above the port's fighter, within the items' limits.
    #[doc(alias = "db_CheckAndSpawnItem")]
    #[doc(alias = "Item_80266F3C")]
    fn db_spawn_item(&mut self, port: i32) {
        let mask = u32::from(
            BUTTON_LEFT
                | BUTTON_RIGHT
                | BUTTON_UP
                | TRIGGER_Z
                | TRIGGER_R
                | TRIGGER_L
                | BUTTON_A
                | BUTTON_B
                | BUTTON_X
                | BUTTON_Y
                | BUTTON_START,
        );
        if self.game.db_down(port) & mask != 0
            || self.game.db_pressed(port) & u32::from(BUTTON_DOWN) == 0
        {
            return;
        }
        let kind = self.game.it.db_menu.CurrentlySelectedItem;
        // `Item_80266F3C`: the common items only in a match that has items.
        if self.game.vs_item_freq() == -1 && kind < IT_COMMON_END {
            return;
        }
        let mut pos = self.game.player_pos(port);
        pos.y = mem(fp::fadds(reg(pos.y), 60.0));
        pos.z = 0.0;
        let facing = super::it::owner::facing_for(self.game, self.data, pos);
        // The fields it doesn't set (`x3E`, `x44_flag`'s other bits, `x45` to `x47`) are
        // stack words no item code reads; `hold_kind`, `x10` and `x48` are the spawn's.
        let mut spawn = crate::game::structs::SpawnItem {
            kind,
            pos,
            prev_pos: pos,
            facing_dir: facing,
            x44_flag: [0x80],
            ..Default::default()
        };
        let common = super::it::item::common_data(self.game);
        let counts = &self.game.it.counts;
        if kind < IT_COMMON_END && counts.x0 as u32 >= common.u32(self.data, 0) {
            // "Item Max Over."
            return;
        }
        if kind < IT_OCTAROCK_STONE && counts.x2C as u32 >= common.u32(self.data, 0x14) {
            // "couldn't get Item struct.(CZako)"
            return;
        }
        if (IT_OLD_KURI..IT_ARWING_LASER).contains(&kind) {
            crate::game::code::unported_call(
                "the debug item spawn of an adventure enemy (it_804A0F60)",
            );
        }
        if kind == IT_M_BALL && super::it::spawn::poke_balls_full(self.game) {
            return;
        }
        if let Some(g) = super::it::item::spawn_air(self, &mut spawn) {
            let b = self.game.it.db_bubbles as u8;
            super::it::item_mut(self.game, g).xDAA[0] |= b;
            super::ef::spawn(self, 0x420, Some(g), &[super::ef::Arg::Vec(spawn.prev_pos)]);
        }
    }

    /// `fn_CheckItemAndPokemonMenu`: the menu's fade for the port that changed it, its
    /// input, the spawn outside a pause, R and D-pad up the items' bubbles, and the develop
    /// level's limits' reset (`fn_80225A54`).
    #[doc(alias = "fn_CheckItemAndPokemonMenu")]
    fn db_check_item_menu(&mut self, port: i32) {
        let m = &mut self.game.it.db_menu;
        if m.DisplayStatus == 1 && m.Player == port {
            if m.DisplayFadeTimer == 0 {
                m.DisplayStatus = 2;
                let t = self.game.db.item_text;
                let s = self.game.text(t);
                s.hide_background();
                s.hide_text();
            } else {
                m.DisplayFadeTimer -= 1;
            }
        }
        self.db_update_item_menu(port);
        if !self.game.db_pause_flag(1) && !self.game.db_pause_flag(0) {
            self.db_spawn_item(port);
        }
        if self.game.db_combo(port, TRIGGER_R, BUTTON_UP) {
            self.db_toggle_item_bubbles();
        }
        self.db_reset_item_peaks(port);
    }

    /// `fn_80225A54`: at the develop level, B and D-pad right set each of the items' kinds'
    /// peak counts back to its live count.
    #[doc(alias = "fn_80225A54")]
    fn db_reset_item_peaks(&mut self, port: i32) {
        if self.game.gm.db_level != DEVELOP || !self.game.db_combo(port, BUTTON_B, BUTTON_RIGHT) {
            return;
        }
        let c = &mut self.game.it.counts;
        c.x4 = c.x0;
        c.xC = c.x8;
        c.x14 = c.x10;
        c.x20 = c.x1C;
        c.x28 = c.x24;
        c.x30 = c.x2C;
        c.x38 = c.x34;
        c.x44 = c.x40;
        c.x4C = c.x48;
        c.x54 = c.x50;
        c.x5C = c.x58;
        c.x64 = c.x60;
    }

    /// `fn_UpdateObjAllocLimiter`: at the develop level, B and D-pad up cap the effects'
    /// pool at its peak (or lift it), A and up the particles' and app-SRTs'.
    // Ports `fn_UpdateObjAllocLimiter`'s level test; aliased once the develop level's limits
    // are ported.
    fn db_alloc_limiter(&mut self, port: i32) {
        if self.game.gm.db_level != DEVELOP {
            return;
        }
        if self.game.db_combo(port, BUTTON_B, BUTTON_UP)
            || self.game.db_combo(port, BUTTON_A, BUTTON_UP)
        {
            crate::game::code::unported_call("the effect pools' limits (fn_UpdateObjAllocLimiter)");
        }
    }

    /// `fn_SetupObjAllocLimiter`: both limits off.
    #[doc(alias = "fn_SetupObjAllocLimiter")]
    fn db_setup_alloc_limiter(&mut self) {
        self.game.db.alloc_limits = [false; 2];
    }

    // The bonuses' readout (`dbbonus.c`).

    /// `fn_SetupBonusInfo`: no port shown (`fn_80228D18`), and the two screens (ids 10 and
    /// 11) in a block of the scene's heap (`fn_80228D38`).
    #[doc(alias = "fn_SetupBonusInfo")]
    #[doc(alias = "fn_80228D18")]
    #[doc(alias = "fn_80228D38")]
    fn db_setup_bonus(&mut self) {
        self.game.db.bonus_port = 0xFF;
        self.game.db.bonus_colors = false;
        let _ = self.game.hud.devtext.get_gobj();
        self.game.db.bonus_block =
            crate::hsd::heap::with(|h| h.mem_alloc(BONUS_BLOCK)).unwrap_or(0);
        let (bg, fg) = BONUS_COLORS[usize::from(self.game.db.bonus_colors)];
        for i in 0..2 {
            self.game.db.bonus_texts[i] = self.game.db_screen(
                i as i8 + 10,
                (i as i32 * 300, 0, 25, 30),
                bg,
                fg,
                (12.0, 16.0),
            );
        }
    }

    /// `fn_80228E54`: the port's bonuses on screen `screen` from `from` on (the first screen
    /// with the ports' scores), going on to the second when it fills.
    #[doc(alias = "fn_80228E54")]
    fn db_print_bonuses(&mut self, port: i32, screen: usize, from: i32) {
        let mut y = 0;
        let t = self.game.db.bonus_texts[screen];
        {
            let s = self.game.text(t);
            s.erase();
            s.set_cursor_xy(0, 0);
        }
        if screen == 0 {
            let scores: Vec<i32> = (0..4).map(|i| self.match_end_score(i)).collect();
            let s = self.game.text(t);
            s.printf(
                b"1P %7d 2P %7d\n",
                &[Arg::Int(scores[0]), Arg::Int(scores[1])],
            );
            s.printf(
                b"3P %7d 4P %7d\n",
                &[Arg::Int(scores[2]), Arg::Int(scores[3])],
            );
            s.printf(
                b"%dP screen %d",
                &[Arg::Int(port + 1), Arg::Int(screen as i32)],
            );
            y = 3;
        }
        self.game.text(t).set_cursor_xy(0, y);
        for bonus in from..0xD7 {
            let n = self.game.bonus_count(port, bonus);
            if n == 0 {
                continue;
            }
            if y >= 0x1E {
                if screen + 1 < 2 {
                    self.db_print_bonuses(port, screen + 1, bonus);
                    return;
                }
                self.game.text(t).printf(b"screen over!!", &[]);
                return;
            }
            let name = self.db_name(self.game.db.bonus_names, bonus);
            let s = self.game.text(t);
            if super::gm::decisions::decision_type(bonus) == 0 {
                s.printf(b"%s", &[Arg::Str(&name)]);
            } else {
                s.printf(b"%s : %d", &[Arg::Str(&name), Arg::Int(n as i32)]);
            }
            y += 1;
            s.set_cursor_xy(0, y);
        }
    }

    /// `fn_8022900C`: the port's bonuses shown, their second colours, then hidden.
    #[doc(alias = "fn_8022900C")]
    fn db_toggle_bonuses(&mut self, port: i32) {
        let db = &mut self.game.db;
        let shown = db.bonus_port == port as u8;
        if shown && db.bonus_colors {
            db.bonus_port = 0xFF;
            db.bonus_colors = false;
            for i in 0..2 {
                let t = self.game.db.bonus_texts[i];
                let s = self.game.text(t);
                s.hide_background();
                s.hide_text();
            }
            return;
        }
        if shown {
            db.bonus_colors = true;
        } else {
            db.bonus_port = port as u8;
        }
        let (bg, fg) = BONUS_COLORS[usize::from(self.game.db.bonus_colors)];
        for i in 0..2 {
            let t = self.game.db.bonus_texts[i];
            let s = self.game.text(t);
            if !shown {
                s.show_background();
                s.show_text();
            }
            s.set_bg_color(bg);
            s.set_text_color(fg);
        }
    }

    /// `fn_CheckBonusInfo`: B and D-pad left step the port's readout; while it shows, its
    /// bonuses tallied (`pl_80039450`) and printed.
    #[doc(alias = "fn_CheckBonusInfo")]
    fn db_check_bonus(&mut self, port: i32) {
        if self.game.db_combo(port, BUTTON_B, BUTTON_LEFT) {
            self.db_toggle_bonuses(port);
        }
        if self.game.db.bonus_port == port as u8 {
            if self.game.player_entity(port).is_some() {
                self.tally_bonuses(port);
            }
            self.db_print_bonuses(port, 0, 0);
        }
    }
}
