//! The ruleset interface (`docs/refactor.md`, task 5): every place gameplay differs by ruleset
//! asks this module, through one function per choice, grouped below by the hook point it
//! serves. The rulesets are data: [`Rules`] holds which of the Slippi ruleset's rules a match
//! runs (as a replay's code list or the session's ruleset turns them on) and in which version of
//! Slippi's codes and of UCF, and the versions are tables of choices ([`SLIPPI_3_19`],
//! [`SLIPPI_3_16`], [`SLIPPI_3_14`] and [`VANILLA`]; [`UCF_0_84`], [`UCF_0_8`] and [`UCF_OFF`];
//! a session's [`Ruleset`], [`VANILLA_SESSION`] and [`SLIPPI_SESSION`]) that the functions read.
//! `Rules`' fields are private here, so nothing else can test them: a port that needs a rule
//! calls the function for its choice (`docs/porting.md`, "Rules").
//!
//! `Rules` is part of the game's state and keeps its shape, so the oracle's comparison, the
//! state's hash and saved states don't change; the tables aren't state.

use gekko_fp::FpMode;

use super::player_codes::{EndRules, PlayerCodes, StartRules};
use super::slippi::CodeSet;
use super::state::Game;
use super::ucf::Ucf;

/// Gameplay toggles (`docs/architecture.md`, rulesets): which of the Slippi ruleset's rules a
/// match runs, each on or off, and in which version ([`Rules::from_codes`] reads them from a
/// replay; `game::slippi` lists the codes). The choices each rule makes are this module's
/// functions.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Rules {
    /// The online determinism fixes (`Common` and `Online/Core`), as [`Rules::codes`]'s
    /// version ships them ([`Fixes`]).
    pub slippi: bool,
    /// Which UCF (`game::ucf`, [`UcfCodes`]).
    pub ucf: Ucf,
    /// Neutral spawn points.
    pub neutral_spawn: bool,
    /// Wobbling prevention.
    pub prevent_wobbling: bool,
    /// Off-screen damage from the stage's camera limits.
    pub offscreen_limits: bool,
    /// The Ice Climbers' freeze glitch fix.
    pub freeze_glitch_fix: bool,
    /// Which version of Slippi's codes the rules come from, where versions differ.
    pub codes: CodeSet,
    /// The codes players add to their own setups that the match runs
    /// (`game::player_codes`).
    pub player: PlayerCodes,
    /// The C-stick in the single-player modes works as in VS, smash attacks and no camera
    /// zoom: the Slippi ruleset's own rule (Sarah, 2026-11-14), narrower than Slippi's
    /// C-Stick in Single Player (`0416B480 60000000`, which makes every caller of the
    /// out-of-line `gm_IsCurrently1PMode_inline` answer 0, the quake, the item table's
    /// weight and the bonuses' progress too). Three calls of it answer 0, as the Gecko code
    /// `0406AE04 38600000`, `0406AE9C 38600000`, `0402B0F8 38600000` (`li r3, 0` for each)
    /// makes them: `Fighter_procInput`'s two (a computer player's C-stick and a human's) and
    /// `Camera_8002B0E0`'s (the zoom). Not one of the 27 gameplay codes a `.slp` carries; it
    /// does nothing outside the single-player modes.
    pub c_stick_1p: bool,
}

/// What a code list turns on, as `game::slippi` decodes it: the rules before the version's
/// table applies.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Toggles {
    pub determinism: bool,
    pub ucf: Ucf,
    pub neutral_spawn: bool,
    pub prevent_wobbling: bool,
    pub offscreen_limits: bool,
    pub freeze_glitch_fix: bool,
}

impl Rules {
    /// Vanilla: no rule on.
    pub const VANILLA: Rules = Rules {
        slippi: false,
        ucf: Ucf::Off,
        neutral_spawn: false,
        prevent_wobbling: false,
        offscreen_limits: false,
        freeze_glitch_fix: false,
        codes: CodeSet::V3_19,
        player: PlayerCodes::NONE,
        c_stick_1p: false,
    };

    /// The Slippi ruleset as Slippi's online play runs it: 3.19's 27 gameplay codes with UCF
    /// 0.84.
    pub const SLIPPI_ONLINE: Rules = Rules {
        slippi: true,
        ucf: Ucf::V0_84,
        neutral_spawn: true,
        prevent_wobbling: true,
        offscreen_limits: true,
        freeze_glitch_fix: true,
        codes: CodeSet::V3_19,
        player: PlayerCodes::NONE,
        c_stick_1p: false,
    };

    /// [`Rules::SLIPPI_ONLINE`].
    pub fn slippi_online() -> Rules {
        Rules::SLIPPI_ONLINE
    }

    /// The rules a code list's toggles make in version `codes`.
    pub(crate) fn from_toggles(t: Toggles, codes: CodeSet) -> Rules {
        Rules {
            slippi: t.determinism,
            ucf: t.ucf,
            neutral_spawn: t.neutral_spawn,
            prevent_wobbling: t.prevent_wobbling,
            offscreen_limits: t.offscreen_limits,
            freeze_glitch_fix: t.freeze_glitch_fix,
            codes,
            ..Rules::VANILLA
        }
    }

    /// These rules with the players' codes `player`.
    pub(crate) fn with_player(self, player: PlayerCodes) -> Rules {
        Rules { player, ..self }
    }

    /// These rules with the Slippi ruleset's C-stick rule on or off ([`Rules::c_stick_1p`]).
    pub fn with_c_stick_1p(self, on: bool) -> Rules {
        Rules {
            c_stick_1p: on,
            ..self
        }
    }

    /// These rules with the determinism fixes of version `codes` on, or off with `None`, the
    /// other rules as they are (tests that play one match under each version).
    pub fn with_fixes(self, codes: Option<CodeSet>) -> Rules {
        Rules {
            slippi: codes.is_some(),
            codes: codes.unwrap_or(self.codes),
            ..self
        }
    }

    /// The version of Slippi's codes the rules come from (a replay's, for its playback).
    pub fn code_set(&self) -> CodeSet {
        self.codes
    }

    /// The version of UCF the rules run.
    pub fn ucf(&self) -> Ucf {
        self.ucf
    }

    /// A short name for the rules (the corpus run's report): `vanilla`, the version with the
    /// determinism fixes or `codes` without them, the UCF, and whether players' codes run.
    pub fn short_name(&self) -> String {
        if *self == Rules::VANILLA {
            return "vanilla".into();
        }
        let mut s = if self.slippi {
            match self.codes {
                CodeSet::V3_19 => "3.19",
                CodeSet::V3_16 => "3.16",
                CodeSet::V3_14 => "3.14",
            }
            .to_owned()
        } else {
            "codes".to_owned()
        };
        s.push_str(&format!(" {:?}", self.ucf));
        if self.player != PlayerCodes::NONE {
            s.push_str(" +players' codes");
        }
        s
    }

    /// The determinism fixes' choices: the version's table, or Vanilla's with them off.
    fn fixes(&self) -> &'static Fixes {
        if !self.slippi {
            return &VANILLA;
        }
        match self.codes {
            CodeSet::V3_19 => &SLIPPI_3_19,
            CodeSet::V3_16 => &SLIPPI_3_16,
            CodeSet::V3_14 => &SLIPPI_3_14,
        }
    }

    /// UCF's choices: the version's table.
    fn ucf_codes(&self) -> &'static UcfCodes {
        match self.ucf {
            Ucf::Off => &UCF_OFF,
            Ucf::V0_8 => &UCF_0_8,
            Ucf::V0_84 => &UCF_0_84,
        }
    }
}

// The tables.

/// What one version of Slippi's online determinism fixes (the `Common` and `Online/Core` codes
/// a `.slp` lists, and what Slippi Dolphin runs with them) chooses, where the ruleset differs
/// from Vanilla. `game::slippi`'s `KNOWN` lists the codes by version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fixes {
    /// `Init Player Data` (`80068EEC`): `Fighter_Create`'s block zeroed.
    pub init_player_data: bool,
    /// `Init Stage Data` (`801C154C`): `Ground_GetStageGObj`'s block zeroed.
    pub init_stage_data: bool,
    /// `FreezeDeadUpFallPhysics` (`800D4C1C`, `800D4D68`, `80080E80`).
    pub freeze_dead_up_fall: bool,
    /// Final Destination's background cycle (`8021AAE4`).
    pub fd_background: FdBackground,
    /// `PSCameraIndependentMonitor` (`801D24FC`): Pokémon Stadium's close-up keeps its player
    /// by fixed bounds, not the camera's view.
    pub stadium_monitor_fixed: bool,
    /// `FreezeStadium` and its preloads (3.16's `801D4578` and the rest).
    pub stadium_frozen: bool,
    /// Pokémon Stadium's transformations load synchronously (3.19's `801D4760`, `801D457C`,
    /// `801C65C8`, `800165AC`). 3.16's are frozen and never load; their table says the same
    /// as 3.19's, as the port always has.
    pub stadium_loads_synchronously: bool,
    /// `WhispyBlowDirFix` (`8008653C`): a dead or sleeping fighter counts for neither side.
    pub whispy_blow_dir_fix: bool,
    /// `NanaDeterminism` (`800AC5B8`): what the DI with no knockback reads.
    pub nana_determinism: NanaDeterminism,
    /// Slippi Dolphin's "Store EFB Copies to Texture Only", its default: the GPU's copies
    /// from the EFB don't reach memory (Sarah, 2026-11-07).
    pub efb_copies_texture_only: bool,
    /// melee-hd's own under Slippi's rules: reads whose outcome Slippi's playback could
    /// decide otherwise (the display pass's phase, playback's camera, Brinstar Depths' haze)
    /// are decided both ways and counted into the playback note (design call 9 of
    /// `docs/slippi-ruleset.md`, `live::DisplayPhase`).
    pub playback_note: bool,
    /// melee-hd's own under Slippi's rules: the Super Scope's one-shot charge reads a fixed
    /// stand-in for its GObj's address where the allocator model is off
    /// (`sscope::ADDRESS_STAND_IN`).
    pub sscope_stand_in: bool,
}

/// Final Destination's background cycle (`grLast_8021AAB0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FdBackground {
    /// The scene's timer advances it, drawing from the random seed (Vanilla).
    DrawsFromTheSeed,
    /// The timer is skipped (Slippi 3.16's `04 8021AAE4 60000000`).
    Skipped,
    /// The timer advances it with the random seed kept to itself (3.19's `C2 8021AAE4`).
    KeepsTheSeed,
}

/// What the DI with no knockback reads (`ftCo_800AC5A0`): the low bytes of r5 and r30.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NanaDeterminism {
    /// What the frame left in r5 and the fighter's address in r30 (Vanilla).
    Leftovers,
    /// r30 cleared (3.16's `NanaDeterminism`).
    ClearsR30,
    /// Both cleared (3.19's).
    ClearsR30AndR5,
}

/// Vanilla's choices: no fix.
pub const VANILLA: Fixes = Fixes {
    init_player_data: false,
    init_stage_data: false,
    freeze_dead_up_fall: false,
    fd_background: FdBackground::DrawsFromTheSeed,
    stadium_monitor_fixed: false,
    stadium_frozen: false,
    stadium_loads_synchronously: false,
    whispy_blow_dir_fix: false,
    nana_determinism: NanaDeterminism::Leftovers,
    efb_copies_texture_only: false,
    playback_note: false,
    sscope_stand_in: false,
};

/// Slippi 3.19's.
pub const SLIPPI_3_19: Fixes = Fixes {
    init_player_data: true,
    init_stage_data: true,
    freeze_dead_up_fall: true,
    fd_background: FdBackground::KeepsTheSeed,
    stadium_monitor_fixed: true,
    stadium_frozen: false,
    stadium_loads_synchronously: true,
    whispy_blow_dir_fix: true,
    nana_determinism: NanaDeterminism::ClearsR30AndR5,
    efb_copies_texture_only: true,
    playback_note: true,
    sscope_stand_in: true,
};

/// Slippi 3.16's, which 3.17 keeps word for word and 3.15 already has.
pub const SLIPPI_3_16: Fixes = Fixes {
    fd_background: FdBackground::Skipped,
    stadium_frozen: true,
    nana_determinism: NanaDeterminism::ClearsR30,
    ..SLIPPI_3_19
};

/// Slippi 3.14's: 3.16's without `WhispyBlowDirFix`.
pub const SLIPPI_3_14: Fixes = Fixes {
    whispy_blow_dir_fix: false,
    ..SLIPPI_3_16
};

/// What one version of UCF runs (`game::ucf`): a code's version where 0.8 and 0.84 both have
/// it, or whether 0.84's own runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UcfCodes {
    /// The pad buffer and 1.0 cardinals (`8006B460`).
    pub pad_buffer: bool,
    /// Dashback (`800C9A44`).
    pub dashback: Ucf,
    /// Shield drop (`800998A4`).
    pub shield_drop: Ucf,
    /// Shield Drop Extended (`8009A0B8`).
    pub shield_drop_extended: bool,
    /// The down-smash out of crouch fix (`800D65EC`).
    pub crouch_fix: bool,
    /// Shield SDI (`80093294`).
    pub shield_sdi: bool,
    /// SDI (`8008E54C`).
    pub sdi: bool,
    /// Tumble wiggle (`800908F4`).
    pub tumble: Ucf,
}

/// No UCF.
pub const UCF_OFF: UcfCodes = UcfCodes {
    pad_buffer: false,
    dashback: Ucf::Off,
    shield_drop: Ucf::Off,
    shield_drop_extended: false,
    crouch_fix: false,
    shield_sdi: false,
    sdi: false,
    tumble: Ucf::Off,
};

/// UCF 0.8: dashback, shield drop and tumble.
pub const UCF_0_8: UcfCodes = UcfCodes {
    dashback: Ucf::V0_8,
    shield_drop: Ucf::V0_8,
    tumble: Ucf::V0_8,
    ..UCF_OFF
};

/// UCF 0.84: every code.
pub const UCF_0_84: UcfCodes = UcfCodes {
    pad_buffer: true,
    dashback: Ucf::V0_84,
    shield_drop: Ucf::V0_84,
    shield_drop_extended: true,
    crouch_fix: true,
    shield_sdi: true,
    sdi: true,
    tumble: Ucf::V0_84,
};

/// Which ruleset a session runs from power-on (`docs/slippi-ruleset.md`): Vanilla, NTSC 1.02
/// as the console runs it, or Slippi's, which runs Slippi Dolphin's playback set's general
/// codes (the unlocks and the special and trophy messages off,
/// [`super::gm::UNLOCK_CODES`]), Skip Memcard Prompt, the 27 gameplay codes and the C-stick's
/// smash attacks in the single-player modes ([`Rules::c_stick_1p`]) from power-on, in Slippi
/// Dolphin's floating point, with no memory card. Its choices are [`Session`]'s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub enum Ruleset {
    #[default]
    Vanilla,
    Slippi,
}

/// What a session's ruleset chooses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    /// The floating point the session runs in: the console's, or Slippi Dolphin's JIT's
    /// (menus included, design call 10).
    pub fp_mode: FpMode,
    /// Whether the slots may hold a memory card and the developer setting may be on.
    pub card_and_develop: bool,
    /// The playback set's general codes in the unlock getters, from power-on
    /// (`gm::UNLOCK_CODES`).
    pub general_codes: bool,
    /// Slippi's Skip Memcard Prompt (`C21AF6F4`).
    pub skip_memcard_prompt: bool,
    /// Each VS match playback can rebuild is a live match built as Slippi's playback builds a
    /// replay's (`game::live`).
    pub live_matches: bool,
    /// Slippi's DynamicsFix in every match ([`Game::dynamics_fix`]).
    pub dynamics_fix: bool,
    /// The gameplay rules every match runs, in every scene.
    pub rules: Rules,
}

/// Vanilla's session.
pub const VANILLA_SESSION: Session = Session {
    fp_mode: FpMode::Hardware,
    card_and_develop: true,
    general_codes: false,
    skip_memcard_prompt: false,
    live_matches: false,
    dynamics_fix: false,
    rules: Rules::VANILLA,
};

/// The Slippi ruleset's session: 3.19's gameplay codes with UCF 0.84 and the C-stick's smash
/// attacks in the single-player modes.
pub const SLIPPI_SESSION: Session = Session {
    fp_mode: FpMode::Slippi,
    card_and_develop: false,
    general_codes: true,
    skip_memcard_prompt: true,
    live_matches: true,
    dynamics_fix: true,
    rules: Rules {
        c_stick_1p: true,
        ..Rules::SLIPPI_ONLINE
    },
};

// Hook point: session boot (the card, the unlock getters, the messages, the float mode).

impl Ruleset {
    fn session(self) -> &'static Session {
        match self {
            Ruleset::Vanilla => &VANILLA_SESSION,
            Ruleset::Slippi => &SLIPPI_SESSION,
        }
    }

    /// The floating point the session runs in.
    pub fn fp_mode(self) -> FpMode {
        self.session().fp_mode
    }

    /// The gameplay rules every match of the session runs, in every scene, as Slippi Dolphin
    /// runs its codes.
    pub fn rules(self) -> Rules {
        self.session().rules.clone()
    }

    /// Whether the session may have a memory card and the developer setting.
    pub fn allows_card_and_develop(self) -> bool {
        self.session().card_and_develop
    }

    /// Whether the playback set's general codes run in the unlock getters.
    pub fn runs_general_codes(self) -> bool {
        self.session().general_codes
    }

    /// Whether the memory card scene skips its question (Skip Memcard Prompt).
    pub fn skips_memcard_prompt(self) -> bool {
        self.session().skip_memcard_prompt
    }

    /// Whether VS matches playback can rebuild are live matches.
    pub fn builds_live_matches(self) -> bool {
        self.session().live_matches
    }
}

// Hook point: match setup (spawns, the fighter and stage blocks zeroed).

impl Rules {
    /// Neutral spawn points (`External/NeutralSpawn`, `Running::neutral_spawn`).
    pub fn neutral_spawns(&self) -> bool {
        self.neutral_spawn
    }

    /// `Init Player Data`: a fighter's block zeroed when `Fighter_Create` makes it.
    pub fn inits_player_data(&self) -> bool {
        self.fixes().init_player_data
    }

    /// `Init Stage Data`: a stage object's block zeroed when `Ground_GetStageGObj` makes it.
    pub fn inits_stage_data(&self) -> bool {
        self.fixes().init_stage_data
    }
}

// Hook point: controller input and the action states' input checks (UCF).

impl Rules {
    /// Whether any UCF runs (the recording's UCF toggles).
    pub fn ucf_on(&self) -> bool {
        self.ucf != Ucf::Off
    }

    /// UCF's pad buffer and 1.0 cardinals (`Running::ucf_pad_buffer`).
    pub fn ucf_pad_buffer(&self) -> bool {
        self.ucf_codes().pad_buffer
    }

    /// UCF's dashback, in its version (`Running::ucf_dashback`).
    pub fn ucf_dashback(&self) -> Ucf {
        self.ucf_codes().dashback
    }

    /// UCF's shield drop, in its version.
    pub fn ucf_shield_drop(&self) -> Ucf {
        self.ucf_codes().shield_drop
    }

    /// UCF 0.84's Shield Drop Extended.
    pub fn ucf_shield_drop_extended(&self) -> bool {
        self.ucf_codes().shield_drop_extended
    }

    /// UCF 0.84's down-smash out of crouch fix.
    pub fn ucf_crouch_fix(&self) -> bool {
        self.ucf_codes().crouch_fix
    }

    /// UCF 0.84's shield SDI (`Running::ucf_shield_sdi`).
    pub fn ucf_shield_sdi(&self) -> bool {
        self.ucf_codes().shield_sdi
    }

    /// UCF 0.84's SDI (`Running::ucf_sdi`).
    pub fn ucf_sdi(&self) -> bool {
        self.ucf_codes().sdi
    }

    /// UCF's tumble wiggle, in its version (`Running::ucf_tumble`).
    pub fn ucf_tumble(&self) -> Ucf {
        self.ucf_codes().tumble
    }
}

// Hook point: off-screen damage.

impl Rules {
    /// Off-screen damage from the stage's camera limits (`BrawlOffscreenDamage`,
    /// `Game::offscreen_limits`), in place of the display pass's mark.
    pub fn offscreen_damage_from_limits(&self) -> bool {
        self.offscreen_limits
    }
}

// Hook point: a KO toward the screen.

impl Rules {
    /// `FreezeDeadUpFallPhysics`: the slide's speed kept in the state's variables, and only
    /// the model placed.
    pub fn freezes_dead_up_fall(&self) -> bool {
        self.fixes().freeze_dead_up_fall
    }
}

// Hook point: targeting through fighters, and the reads the display pass's phase or playback's
// camera decide.

impl Rules {
    /// `WhispyBlowDirFix`: a dead or sleeping fighter counts for neither side (`it::owner`).
    pub fn whispy_blow_dir_fix(&self) -> bool {
        self.fixes().whispy_blow_dir_fix
    }

    /// Whether reads Slippi's playback could decide otherwise are decided both ways and
    /// counted into the playback note ([`Fixes::playback_note`]).
    pub fn notes_playback(&self) -> bool {
        self.fixes().playback_note
    }
}

// Hook point: stage determinism.

impl Rules {
    /// Final Destination's background cycle.
    pub fn fd_background(&self) -> FdBackground {
        self.fixes().fd_background
    }

    /// `PSCameraIndependentMonitor`: Pokémon Stadium's close-up keeps its player by fixed
    /// bounds.
    pub fn stadium_monitor_fixed(&self) -> bool {
        self.fixes().stadium_monitor_fixed
    }

    /// Pokémon Stadium frozen (`gr::stadium::frozen`).
    pub fn stadium_frozen(&self) -> bool {
        self.fixes().stadium_frozen
    }

    /// Pokémon Stadium's transformations load synchronously.
    pub fn stadium_loads_synchronously(&self) -> bool {
        self.fixes().stadium_loads_synchronously
    }
}

// Hook point: grabs and hits (wobbling).

impl Rules {
    /// Wobbling prevention (`External/PreventWobbling`).
    pub fn prevents_wobbling(&self) -> bool {
        self.prevent_wobbling
    }
}

// Hook point: the Ice Climbers' freeze glitch.

impl Rules {
    /// The freeze glitch fix (`External/FreezeGlitchFix`).
    pub fn fixes_freeze_glitch(&self) -> bool {
        self.freeze_glitch_fix
    }
}

// Hook point: the computer player's leftover registers.

impl Rules {
    /// What the DI with no knockback reads (`NanaDeterminism`).
    pub fn nana_determinism(&self) -> NanaDeterminism {
        self.fixes().nana_determinism
    }
}

// Hook point: addresses gameplay reads.

impl Rules {
    /// The Super Scope's stand-in address where the allocator model is off.
    pub fn sscope_stand_in(&self) -> bool {
        self.fixes().sscope_stand_in
    }
}

// Hook point: the single-player modes' C-stick.

impl Rules {
    /// The Slippi ruleset's C-stick rule ([`Rules::c_stick_1p`]).
    pub fn c_stick_1p(&self) -> bool {
        self.c_stick_1p
    }
}

// Hook point: recording and playback.

impl Game {
    /// Whether the GPU's copies from the EFB reach memory. A console's do. Slippi's Dolphin
    /// keeps them on the GPU ("Store EFB Copies to Texture Only", its default), so memory keeps
    /// what it held before them: melee-hd follows it under the Slippi ruleset and in a
    /// replay's playback (Sarah, 2026-11-07), the console elsewhere (docs/engine.md, "The GPU's
    /// pixels").
    pub fn copies_reach_memory(&self) -> bool {
        !(self.rules.fixes().efb_copies_texture_only || self.runs_playback_codes())
    }

    /// Whether Slippi's DynamicsFix runs (`ftCo_8009DD94+0x2FC`,
    /// [`super::state::Running::setup_dirty_matrices`]): a `Common` code of both of Slippi
    /// Dolphin's code sets, unconditional, so in every match Slippi Dolphin runs, its
    /// playback's and its live play's alike, though no `.slp` lists it. So in playback, and
    /// under the Slippi ruleset in every match, live or kept in the menus' session (Sudden
    /// Death, the single-player modes, the attract match); never under Vanilla (Sarah,
    /// 2026-11-16: "if slippi differs between live play and playback, so too should we in the
    /// same way"). The oracle can turn it off (`oracle_hooks::set_dynamics_fix_off`), to see
    /// what else its comparison finds.
    pub fn dynamics_fix(&self) -> bool {
        #[cfg(feature = "oracle-hooks")]
        if super::oracle_hooks::dynamics_fix_off() {
            return false;
        }
        self.runs_playback_codes() || self.ruleset.session().dynamics_fix
    }

    /// Whether Slippi's playback set runs in this match (a replay's, or a live match built as
    /// playback builds one): its `Common` codes (the frame count, `AllocSceneBuffer`,
    /// `CostumeBoundCheck`), its GObjs, and the recorder's processes as its codes'.
    pub fn runs_playback_codes(&self) -> bool {
        self.playback.is_some()
    }

    /// Whether playback's sound codes act (`Playback::sound_codes`): a replay's match from its
    /// scene on; never a live match.
    pub fn runs_sound_codes(&self) -> bool {
        self.playback.as_ref().is_some_and(|p| p.sound_codes())
    }
}

// Hook point: players' own codes (`game::player_codes`).

impl Rules {
    /// X/Z swap (`C2 8034E2AC`).
    pub fn swaps_xz(&self) -> bool {
        self.player.swap_xz
    }

    /// Invisible techs (`C2 80080A30`).
    pub fn hides_techs(&self) -> bool {
        self.player.invisible_techs
    }

    /// Fast-fall spark (`C2 8007D5A0`).
    pub fn fall_fast_spark(&self) -> bool {
        self.player.fall_fast_spark
    }

    /// Quiet announcer (`04 800249A4`).
    pub fn quiet_announcer(&self) -> bool {
        self.player.quiet_announcer
    }

    /// The camera code (`C2 801C0A50`).
    pub fn player_camera(&self) -> bool {
        self.player.camera
    }

    /// What `C2 8016E750` writes into the match's start data.
    pub fn start_rules(&self) -> Option<StartRules> {
        self.player.start_rules
    }

    /// What `C2 8016E91C` writes at the end of the scene's entry.
    pub fn end_rules(&self) -> Option<EndRules> {
        self.player.end_rules
    }

    /// SDI colors (`C2 8008E588`).
    pub fn sdi_colors(&self) -> bool {
        self.player.sdi_colors
    }

    /// SDI colors' reset (`C2 8008E734`).
    pub fn sdi_colors_reset(&self) -> bool {
        self.player.sdi_colors_reset
    }

    /// Dash grab color (`C2 8006A138`).
    pub fn dash_grab_color(&self) -> bool {
        self.player.dash_grab_color
    }

    /// Fountain of Dreams' reflection at 640x480.
    pub fn izumi_reflection_size(&self) -> bool {
        self.player.izumi_reflection_size
    }

    /// Fountain of Dreams' reflection keeping the fighters' render callbacks.
    pub fn izumi_reflection_callbacks(&self) -> bool {
        self.player.izumi_reflection_callbacks
    }

    /// Marth's sword trail colors (`C2 80136510`), accepted only without Marth.
    pub fn marth_trail(&self) -> bool {
        self.player.marth_trail
    }

    /// Pokémon Stadium's monitor weights.
    pub fn stadium_weights(&self) -> bool {
        self.player.stadium_weights
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tables give each version's choices as the branches they replaced decided them
    /// (`slippi && codes == V3_19` and the like), so a table's entry flipped fails here as well
    /// as in the matches.
    #[test]
    fn the_tables_choose_as_the_versions_do() {
        let fixes = |codes| Rules::VANILLA.with_fixes(codes);
        for (codes, slippi) in [
            (None, false),
            (Some(CodeSet::V3_19), true),
            (Some(CodeSet::V3_16), true),
            (Some(CodeSet::V3_14), true),
        ] {
            let r = fixes(codes);
            let v3_19 = codes == Some(CodeSet::V3_19);
            assert_eq!(r.inits_player_data(), slippi);
            assert_eq!(r.inits_stage_data(), slippi);
            assert_eq!(r.freezes_dead_up_fall(), slippi);
            assert_eq!(r.stadium_monitor_fixed(), slippi);
            assert_eq!(r.stadium_loads_synchronously(), slippi);
            assert_eq!(r.notes_playback(), slippi);
            assert_eq!(r.sscope_stand_in(), slippi);
            assert_eq!(r.stadium_frozen(), slippi && !v3_19);
            assert_eq!(
                r.whispy_blow_dir_fix(),
                slippi && codes != Some(CodeSet::V3_14)
            );
            assert_eq!(
                r.fd_background(),
                match codes {
                    None => FdBackground::DrawsFromTheSeed,
                    Some(CodeSet::V3_19) => FdBackground::KeepsTheSeed,
                    Some(_) => FdBackground::Skipped,
                }
            );
            assert_eq!(
                r.nana_determinism(),
                match codes {
                    None => NanaDeterminism::Leftovers,
                    Some(CodeSet::V3_19) => NanaDeterminism::ClearsR30AndR5,
                    Some(_) => NanaDeterminism::ClearsR30,
                }
            );
        }
        for ucf in [Ucf::Off, Ucf::V0_8, Ucf::V0_84] {
            let r = Rules {
                ucf,
                ..Rules::VANILLA
            };
            let v0_84 = ucf == Ucf::V0_84;
            assert_eq!(r.ucf_on(), ucf != Ucf::Off);
            assert_eq!(r.ucf_dashback(), ucf);
            assert_eq!(r.ucf_shield_drop(), ucf);
            assert_eq!(r.ucf_tumble(), ucf);
            assert_eq!(r.ucf_pad_buffer(), v0_84);
            assert_eq!(r.ucf_shield_drop_extended(), v0_84);
            assert_eq!(r.ucf_crouch_fix(), v0_84);
            assert_eq!(r.ucf_shield_sdi(), v0_84);
            assert_eq!(r.ucf_sdi(), v0_84);
        }
    }

    #[test]
    fn the_sessions_choose_as_the_rulesets_do() {
        assert_eq!(Ruleset::Vanilla.rules(), Rules::default());
        assert_eq!(
            Ruleset::Slippi.rules(),
            Rules::slippi_online().with_c_stick_1p(true)
        );
        for ruleset in [Ruleset::Vanilla, Ruleset::Slippi] {
            let slippi = ruleset == Ruleset::Slippi;
            assert_eq!(ruleset.fp_mode() == FpMode::Slippi, slippi);
            assert_eq!(ruleset.allows_card_and_develop(), !slippi);
            assert_eq!(ruleset.runs_general_codes(), slippi);
            assert_eq!(ruleset.skips_memcard_prompt(), slippi);
            assert_eq!(ruleset.builds_live_matches(), slippi);
            assert_eq!(ruleset.session().dynamics_fix, slippi);
        }
    }

    /// Nothing outside this module compares a session's ruleset by name: it asks the
    /// ruleset's functions (the session's format reads and writes the name).
    #[test]
    fn rulesets_are_asked_through_the_module() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        fn sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    sources(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
        sources(&src, &mut files);
        let mut found = Vec::new();
        for p in files {
            let rel = p
                .strip_prefix(&src)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if rel == "game/ruleset.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&p).unwrap();
            let code = text.split("\n#[cfg(test)]").next().unwrap();
            for (i, l) in code.lines().enumerate() {
                let l = l.trim();
                if l.starts_with("//") {
                    continue;
                }
                if ["== Ruleset::", "!= Ruleset::", "== super::state::Ruleset::"]
                    .iter()
                    .any(|t| l.contains(t))
                    || l.contains("== super::super::state::Ruleset::")
                {
                    found.push(format!("{rel}:{}: {l}", i + 1));
                }
            }
        }
        assert!(
            found.is_empty(),
            "a ruleset compared by name outside game::ruleset:\n{}",
            found.join("\n")
        );
    }
}
