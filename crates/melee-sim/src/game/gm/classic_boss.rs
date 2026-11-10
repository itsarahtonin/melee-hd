//! Classic's boss stage (`gm_17C0.c`): its start hook (`fn_8017C71C`), the controller that
//! runs Master Hand's entry, Crazy Hand's arrival and the hands' defeat (`fn_8017C1A4`), and
//! Crazy Hand's slot (`fn_8017C0C8`); the arrival's condition (`fn_8017EDDC`, `gm_17EB.c`).

use super::super::ft::masterhand::KIND_CRAZY_HAND;
use super::super::handles::CodeRef;
use super::super::state::Running;
use super::GM_CLASSIC;

/// `fn_8017C1A4`, the controller's process.
pub const PROC: u32 = 0x8017_C1A4;

/// `Gm_PKind_Cpu`, `CKind_CrezyH`.
const PKIND_CPU: u8 = 1;
const CKIND_CREZYH: i8 = 0x1E;
/// Master Hand's and Crazy Hand's slots, and the player's.
const SLOT_PLAYER: i32 = 0;
const SLOT_CRAZY: i32 = 1;
const SLOT_MASTER: i32 = 2;

/// `lbl_804706C0`: the boss stage's controller state.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct BossStage {
    /// `x0`: the controller's state (0 Master Hand's entry, 1 Crazy Hand's arrival, 2 the
    /// fight's start, 3 Master Hand alone, 4 both hands, 5 one left, 6 one beaten, 7 both
    /// beaten).
    pub state: i32,
    /// `x4`: the slot the defeat's camera looks at.
    pub focus: i32,
    /// `x8`: the state's frame.
    pub timer: i32,
    /// `xC`: Master Hand's stamina for the run's level (`ftBossLib_8015C530`).
    pub hp: i32,
    /// `x10`, `x14`: the player's and Master Hand's `x1988` kept through Crazy Hand's
    /// arrival.
    pub player_x1988: i32,
    pub master_x1988: i32,
}

impl Running<'_> {
    /// The boss stage's start hook, `fn_8017C71C`: the controller's process (priority 21),
    /// its state reset, Master Hand's stamina for the run's level, and Crazy Hand's costume
    /// loaded for his arrival.
    #[doc(alias = "fn_8017C71C")]
    pub fn classic_boss_start(&mut self) {
        let w = &mut self.game.world;
        let g = w.create(15, 17, 0);
        w.setup_proc(g, CodeRef(PROC), 21);
        let b = &mut self.game.one_player.boss;
        b.state = 0;
        b.focus = 1;
        b.timer = 0;
        let level = super::super::it::item::one_player_cpu_level(self);
        let hp = self.boss_level_word(level);
        self.game.one_player.boss.hp = hp;
        self.game.set_player_stamina(SLOT_MASTER, hp);
        self.load_fighter_costume(KIND_CRAZY_HAND, 0);
    }

    /// Crazy Hand joins in slot 1, `fn_8017C0C8`: a computer player with Master Hand's
    /// level, kind and ratios, one stock, on team 4, with Master Hand's stamina.
    // Ports `fn_8017C0C8`; aliased once a check reaches it (Crazy Hand's arrival).
    fn classic_crazy_hand_joins(&mut self) {
        let mut p = super::vs_mode::player_defaults();
        p.slot_type = PKIND_CPU;
        p.stocks = 1;
        p.cpu_kind = 4;
        p.cpu_level = self.game.player_cpu_level(SLOT_MASTER) as u8;
        p.cpu_kind = self.game.player_cpu_type(SLOT_MASTER) as u8;
        p.attack_ratio = self.game.player_attack_ratio(SLOT_MASTER);
        p.defense_ratio = self.game.player_unk50(SLOT_MASTER);
        p.color = 0;
        p.ckind = CKIND_CREZYH;
        p.team = 4;
        p.xC_b7 = true;
        p.hp = self.game.one_player.boss.hp as u16;
        p.xD_b2 = true;
        p.xD_b0 = true;
        p.spawn_dir = 1;
        self.vs_add_player(SLOT_CRAZY, &p);
    }

    /// The boss stage's controller, `fn_8017C1A4`: Master Hand's entry with the HUD and
    /// input off; Crazy Hand's arrival once Master Hand's stamina is down to half
    /// (`fn_8017EDDC`), the match slowed while he enters; a hand's defeat (the camera on it,
    /// half speed, the crowd); and both hands' (the background's flash, quakes and rumble
    /// every 30 frames, the match's end).
    #[doc(alias = "fn_8017C1A4")]
    pub fn classic_boss_proc(&mut self) {
        let w164 = self.boss_mh_word(0x164);
        let w168 = self.boss_mh_word(0x168);
        let w16c = self.boss_mh_word(0x16C);
        let w174 = self.boss_mh_word(0x174);
        let hp = |r: &Self, slot| r.game.player_remaining_hp(slot);
        let b = self.game.one_player.boss;
        match b.state {
            0 => {
                if b.timer == 0 {
                    self.game.vs.start.disable_pausing = false;
                    self.game.vs.state.hud_enabled = 0;
                    self.sfx_play(0x81650, 127, 64);
                    self.game.ft_disable_all_input();
                    self.game.one_player.boss.timer = 1;
                } else if !self.boss_mh_entering() {
                    self.game.one_player.boss.state = 2;
                }
            }
            1 => {
                if b.timer == 0 {
                    self.lb_audio_music_stop();
                    self.boss_set_rate(0x1D9);
                    self.game.set_player_more_flags_bit4(SLOT_PLAYER, 1);
                    self.game.set_player_more_flags_bit3(SLOT_PLAYER, 1);
                    let e = self.player_gobj(SLOT_PLAYER);
                    let x = self.game.ft_80087c58(e);
                    self.game.one_player.boss.player_x1988 = x;
                    self.game.ft_80087c64(e, 2);
                    let e = self.player_gobj(SLOT_MASTER);
                    let x = self.game.ft_80087c58(e);
                    self.game.one_player.boss.master_x1988 = x;
                    self.game.ft_80087c64(e, 2);
                    self.game.one_player.boss.timer = 1;
                } else if b.timer == 1 {
                    self.sfx_play(0x81651, 127, 64);
                    self.classic_crazy_hand_joins();
                    self.game.one_player.boss.timer = 2;
                } else if !self.boss_ch_in_183() {
                    self.start_stage_music(0);
                    self.boss_reset_item_rate();
                    self.game.set_player_more_flags_bit4(SLOT_PLAYER, 0);
                    self.game.set_player_more_flags_bit3(SLOT_PLAYER, 0);
                    let e = self.player_gobj(SLOT_PLAYER);
                    self.game.ft_80087c64(e, b.player_x1988);
                    let e = self.player_gobj(SLOT_MASTER);
                    self.game.ft_80087c64(e, b.master_x1988);
                    self.boss_camera_standard();
                    self.game.one_player.boss.state = 4;
                }
            }
            2 => {
                self.game.vs.state.hud_enabled = 1;
                self.start_stage_music(0);
                self.game.ft_enable_all_input();
                self.boss_camera_standard();
                self.game.one_player.boss.state = 3;
            }
            3 => {
                let mh = hp(self, SLOT_MASTER);
                let b = &mut self.game.one_player.boss;
                if mh <= 0 {
                    b.state = 7;
                    b.timer = 0;
                } else if gekko_fp::frsp(f64::from(mh))
                    <= gekko_fp::fmuls(0.5, gekko_fp::frsp(f64::from(b.hp)))
                    && self.classic_crazy_hand_allowed()
                {
                    let b = &mut self.game.one_player.boss;
                    b.state = 1;
                    b.timer = 0;
                }
            }
            4 => {
                let (mh, ch) = (hp(self, SLOT_MASTER), hp(self, SLOT_CRAZY));
                let b = &mut self.game.one_player.boss;
                if mh <= 0 && ch <= 0 {
                    b.state = 7;
                    b.timer = 0;
                } else if mh <= 0 || ch <= 0 {
                    b.state = 6;
                    b.timer = 0;
                }
            }
            5 => {
                if hp(self, SLOT_MASTER) <= 0 && hp(self, SLOT_CRAZY) <= 0 {
                    let b = &mut self.game.one_player.boss;
                    b.state = 7;
                    b.timer = 0;
                }
            }
            6 => {
                let focus = if hp(self, SLOT_MASTER) <= 0 { 2 } else { 1 };
                self.game.one_player.boss.focus = focus;
                let end = w164.wrapping_add(w168);
                let t = b.timer;
                if t == 0 {
                    self.boss_camera_on_player(focus);
                    self.game.set_db_pause_flag(6);
                } else if t == w164 {
                    self.sfx_play_cheer(0x4E200);
                    self.boss_camera_standard();
                    self.game.clear_db_pause_flag(6);
                    self.game.set_game_speed(0.5);
                } else if t >= end {
                    self.game.set_game_speed(1.0);
                    self.game.one_player.boss.state = 5;
                }
                let both = hp(self, SLOT_MASTER) <= 0 && hp(self, SLOT_CRAZY) <= 0;
                let b = &mut self.game.one_player.boss;
                if both {
                    b.state = 7;
                    b.timer = 0;
                } else if b.timer <= end {
                    b.timer = b.timer.wrapping_add(1);
                }
            }
            7 => {
                let flash = w164.wrapping_add(w16c);
                let end = w174.wrapping_add(flash);
                let t = b.timer;
                if t == 0 {
                    self.game.vs.state.hud_enabled = 0;
                    let focus = if b.focus == 2 { 1 } else { 2 };
                    self.game.one_player.boss.focus = focus;
                    self.boss_camera_on_player(focus);
                    self.game.set_db_pause_flag(6);
                    self.player_disable_input(SLOT_PLAYER);
                    self.player_80036844(SLOT_PLAYER, 1);
                    self.game.vs.start.timer_enabled = false;
                } else if t == w164 {
                    self.sfx_play_cheer(0x4E200);
                    self.boss_camera_standard();
                    self.game.clear_db_pause_flag(6);
                    self.game.set_game_speed(0.5);
                    self.game.bgflash.fade_to_white(w16c);
                } else if t == flash {
                    self.game.bgflash.fade_white_to_black(w174);
                } else if t >= end {
                    // `lbAudioAx_80028B90`: the master volume back up.
                    self.game.audio.lb.master = 0x7F;
                    self.game.set_game_speed(1.0);
                    self.game.vs_set_end_kind(8);
                    self.game.vs_terminate();
                    return;
                }
                if t >= w164 {
                    // `lbAudioAx_80028B6C`: the master volume a step down.
                    let lb = &mut self.game.audio.lb;
                    lb.master = (lb.master - 1).max(0);
                }
                if t <= flash && t.wrapping_rem(30) == 0 {
                    let tag = self.game.player_nametag(SLOT_PLAYER);
                    let port = self.game.player_port(SLOT_PLAYER);
                    self.tou_rumble(port as usize, tag, 2, 30);
                    super::super::camera::request_quake(
                        self,
                        super::super::camera::QUAKE_MEDIUM,
                        None,
                    );
                }
                let b = &mut self.game.one_player.boss;
                if b.timer <= end {
                    b.timer = b.timer.wrapping_add(1);
                }
            }
            _ => {}
        }
    }

    /// A slot's fighter (`Player_GetEntity`), which the boss stage's slots always have.
    fn player_gobj(&self, slot: i32) -> crate::hsd::gobj::GObjId {
        self.game
            .player_entity(slot)
            .expect("the boss stage's slot has a fighter")
    }

    /// Whether Crazy Hand may join, `fn_8017EDDC`: in Classic, with no continue taken, on
    /// Normal or harder, in under 21,000 frames of play.
    #[doc(alias = "fn_8017EDDC")]
    fn classic_crazy_hand_allowed(&self) -> bool {
        if self.game.game_mode != GM_CLASSIC {
            return false;
        }
        let run = &self.game.one_player.classic.base;
        run.xc[1] == 0 && run.player.cpu_level >= 2 && run.x20 < 0x5208
    }
}
