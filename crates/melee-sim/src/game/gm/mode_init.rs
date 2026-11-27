//! Every game mode's `on_init`, which the modes' loop runs once at boot (`gm_801A4510`): each
//! mode's settings in the game's data at their defaults (`gm_InitVsMode` for the VS-like
//! modes, the single-player modes' choices, the events'), and the few kept elsewhere.

use super::super::state::Running;
use super::super::structs::{PlayerInitData, VsModeData};
use super::vs_mode::{player_defaults, start_defaults};
use crate::game::ft::CpuKind;

/// The modes' settings outside the game's data that their `on_init`s set.
#[derive(Clone, Debug, PartialEq, melee_derive::StateEq)]
pub struct ModeStatics {
    /// `gm_80473814.saved_players`: the training mode's players.
    pub training_players: [PlayerInitData; 4],
    /// `gm_80473814.stage_id`: the training mode's stage.
    pub training_stage: i16,
    /// `gmHomeRun_VsModeData`.
    pub homerun: VsModeData,
}

impl Default for ModeStatics {
    fn default() -> Self {
        ModeStatics {
            training_players: core::array::from_fn(|_| PlayerInitData::default()),
            training_stage: 0,
            homerun: VsModeData::default(),
        }
    }
}

/// `GmVsMode`s: the modes table's entries.
const VS_MELEE: usize = 0;
const VS_SUPER_SUDDEN_DEATH: usize = 1;
const VS_INVISIBLE: usize = 2;
const VS_CAMERA: usize = 3;
const VS_FIXED_CAMERA: usize = 4;
const VS_SINGLE_BUTTON: usize = 5;
const VS_TRAINING: usize = 6;
const VS_TINY: usize = 7;
const VS_GIANT: usize = 8;
const VS_STAMINA: usize = 9;
const VS_SLOMO: usize = 10;
const VS_LIGHTNING: usize = 11;
const VS_MULTIMAN: usize = 12;

/// `gm_InitVsMode`: a VS-like mode's start data at its defaults, no winner, loser or stage
/// in order yet.
fn init_vs_mode(vs: &mut VsModeData) {
    vs.start = start_defaults();
    vs.loser = -1;
    vs.ordered_stage_index = -1;
    vs.winner = -1;
}

impl Running<'_> {
    /// The modes' `on_init`s in the modes table's order (`gm_GetAllGameModes`, the
    /// multi-man modes' six entries sharing `gm_Mode_10ManVs_OnInit`).
    #[doc(alias = "gmVsMelee_Mode_OnInit")]
    #[doc(alias = "gmMainLib_8015CDEC")]
    #[doc(alias = "gm_Mode_Classic_OnInit")]
    #[doc(alias = "gm_Mode_Adventure_OnInit")]
    #[doc(alias = "gm_Mode_AllStar_OnInit")]
    #[doc(alias = "gm_Mode_Camera_OnInit")]
    #[doc(alias = "Toy_Mode_OnInit")]
    #[doc(alias = "gm_Mode_TargetTest_OnInit")]
    #[doc(alias = "gm_Mode_SuperSuddenDeath_OnInit")]
    #[doc(alias = "gm_Mode_InvisibleVs_OnInit")]
    #[doc(alias = "gm_Mode_SlowMo_OnInit")]
    #[doc(alias = "gm_Mode_LightningVs_OnInit")]
    #[doc(alias = "gm_Mode_Training_OnInit")]
    #[doc(alias = "gm_Mode_TinyVs_OnInit")]
    #[doc(alias = "gm_Mode_GiantVs_OnInit")]
    #[doc(alias = "gm_Mode_StaminaVs_OnInit")]
    #[doc(alias = "gm_Mode_Homerun_OnInit")]
    #[doc(alias = "gm_Mode_10ManVs_OnInit")]
    #[doc(alias = "gm_Mode_CameraVs_OnInit")]
    #[doc(alias = "gm_Mode_Event_OnInit")]
    #[doc(alias = "gm_Mode_SingleButtonVs_OnInit")]
    pub fn modes_on_init(&mut self) {
        let d = &mut self.game.game_data;
        // VS: its table entry, and the name tags' handicaps (`gmMainLib_8015CDEC`: the four
        // ports' without a name tag).
        init_vs_mode(&mut d.modes.table[VS_MELEE]);
        d.modes.nametags = [5; 4];
        // Classic, Adventure, All-Star: no character chosen, their stocks.
        for (s, stocks) in [
            (&mut d.vs.classic, 3),
            (&mut d.vs.adventure, 3),
            (&mut d.vs.all_star, 1),
        ] {
            s.c_kind = 0x21;
            s.color = 0;
            s.stocks = stocks;
            s.cpu_level = 0;
            s.nametag = 0x78;
            s.round = 0;
        }
        init_vs_mode(&mut d.modes.table[VS_CAMERA]);
        // The trophy gallery (`Toy_Mode_OnInit`): its screens' state cleared, the gallery
        // flagged seen in the save's categories (the current mode is the title's here).
        self.toy_mode_init();
        let d = &mut self.game.game_data;
        // Target Test: no character, no name tag.
        let t = &mut d.vs.event.target_test;
        t.ckind = 0x21;
        t.color = 0;
        t.nametag = 0x78;
        for m in [VS_SUPER_SUDDEN_DEATH, VS_INVISIBLE, VS_SLOMO, VS_LIGHTNING] {
            init_vs_mode(&mut d.modes.table[m]);
        }
        // Training: its players kept for its menus, the others empty.
        init_vs_mode(&mut d.modes.table[VS_TRAINING]);
        let tr = &mut d.modes.table[VS_TRAINING].start.players;
        for i in 0..4 {
            tr[i].color = i as u8;
            tr[i].cpu_kind = CpuKind::Stay.into();
            if i != 0 {
                tr[1].ckind = 0x21;
            }
            self.game.modes.training_players[i] = tr[i].clone();
        }
        let d = &mut self.game.game_data;
        for m in [VS_TINY, VS_GIANT, VS_STAMINA] {
            init_vs_mode(&mut d.modes.table[m]);
        }
        init_vs_mode(&mut self.game.modes.homerun);
        let d = &mut self.game.game_data;
        // The six multi-man modes share one entry, set up six times alike.
        init_vs_mode(&mut d.modes.table[VS_MULTIMAN]);
        init_vs_mode(&mut d.modes.table[VS_FIXED_CAMERA]);
        // Events: none chosen, no name tag.
        let e = &mut d.vs.event;
        e.char_kind = 0;
        e.char_color = 0;
        e.chosen_char = 0x21;
        e.chosen_color = 0;
        e.nametag = 0x78;
        e.event = 0;
        e.port = 0;
        e.team = 0;
        e.random_char_1 = -1;
        e.random_char_2 = -1;
        e.random_char_3 = -1;
        e.timer_counts_up = false;
        e.cleared = false;
        e.round_won = false;
        e.crowd = false;
        e.series = false;
        e.voices_loaded = false;
        e.timed = false;
        e.score = 0;
        e.proc_counter = 0;
        e.unread_14 = 0;
        e.proc_state = 0;
        e.quake_scale = 1.0;
        e.round = 0;
        e.round_stocks = 0;
        e.round_damage = 0;
        e.round_seconds = 0;
        e.round_frames = 0;
        e.series_frames = 0;
        e.kept_char = 0x21;
        e.coins = 0;
        e.play_frames = 0;
        e.load_kind = -1;
        e.crowd_kirby_copies = 0;
        e.stage = 0;
        e.chars = [0x21; 4];
        e.colors = [0; 4];
        init_vs_mode(&mut d.modes.table[VS_SINGLE_BUTTON]);
        let _ = player_defaults;
    }
}
