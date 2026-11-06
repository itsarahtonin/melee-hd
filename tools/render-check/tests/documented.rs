//! The frames docs/player.md documents, the standard set: each checked against ssbm-rs's
//! reference as the document records it (`melee-render-check compare` on each, its alignment
//! and its two images), and a few drawn at higher scales and in widescreen against their own
//! 1x frame (`scaled`). Ignored by default, since it needs ssbm-rs's references and the disc
//! and takes minutes; run it, with the Fox ditto's mix, as the pre-push frames+audio step:
//!
//! ```sh
//! SSBM_DISC=... SSBM_RS_LOCAL=... SLIPPI_REPLAYS=... \
//!     cargo test --release -p melee-render-check -p melee-audio-check -- --ignored
//! ```
//!
//! More frames of the same matches are in the opt-in sweep (`tests/sweep.rs`). See
//! `common` for where the references come from.

mod common;

use common::{
    BIG_BLUE, BRINSTAR, BRINSTAR_DEPTHS, CAMERA_NOCARD, CAMERA_SNAPS, CELEBI1, CH_MH_1P, CH_PROBE,
    CORNERIA, Capture, DATA_ALBUM, DATA_ALBUM_DELETE, DATA_BONUS, DATA_COUNT, DATA_DIAGRAM,
    DATA_INFO, DATA_PRIZE, DEBUG_COLL, DEBUG_GAL_L, DEBUG_GAL_Z, DEBUG_MENU_ENTER, DEBUG_MENU_SUB,
    DEBUG_PERF_MONITOR, DEBUG_SOUND, DEBUG_VS_BUBBLES, DEBUG_VS_COINS, DEBUG_VS_DISPLAYS,
    DEBUG_VS_STAGE, DEBUG_VS_TOOLS, DEBUG_YOSHIKI_BASIC, DEBUG_YOSHIKI_SNAPS, EV12, EV14, EV22,
    EV26, EV43, EV47, Expect, FLAT_ZONE, FOUR_FD, FOURSIDE, FOX_FOX_FD, FOX_MARTH_BF, FOX_MARTH_FD,
    FOX_MARTH_PS, FOX_SHEIK_DL, FOX_SHEIK_FD, FOX_SHEIK_YS, FZ_CUTSCENE, FZ_RUN, GALLERY, GB_BOY,
    GB_GIRL, GB_GK, GREAT_BAY, GREEN_GREENS, Game, Group, HR_LONG, HR_MISS, HYRULE_TEMPLE,
    ICICLE_MOUNTAIN, IDLE_MOVIE, IDLE_TITLE, IM_BEARS, IM_JUMP, IM_LOOK, ITEMS_POWERUPS_BF,
    ITEMS_POWERUPS_FD, ITEMS_POWERUPS_PS, JUNGLE_JAPES, KONGO_JUNGLE, KONGO_JUNGLE_64, M6_CLASSIC,
    M6_HOMERUN, M6_TARGET, M6_TOUR_CLEARED, M6_TRAINING, M6A_INTRO_AS_IC, M6C_CUTSCENES,
    M6E_GAMEOVER, M6E2_REGEND_ADV, M6E2_REGEND_GW, M6E3_REGEND_MOVIE, M6E3_STAFFROLL_PLAY,
    M6I_ADVENTURE2, M6I_CLASSIC_WIN, M6P_AS_WIN, M6U_EV01_FD, M6U_EV03_TROPHY, M6U_EV37_PICHU,
    M6U_HR_TOTAL, M6U_TR_COMBO, MENU_TOUR, METAL_CLOAK_RC, MH_1P, MH_1P_WILD, MH_3P, MH_PROBE,
    MK_ENEMIES, MK_FALL, MK_ROUTE, MM_10_WIN, MM_15MIN, MUSHROOM_KINGDOM, MUSHROOM_KINGDOM_2,
    MUTE_CITY, MZ_ARENA, MZ_CAPTURE, MZ_EXIT, MZ_KILL, NAMES, NAMES_KANA, O6_INTRO_ALLSTAR, ONETT,
    OPTIONS_TOUR, PC_IZUMI_SIZE, PEACHS_CASTLE, POKE_FLOATS, RACE_LOOK, RACE_PILLAR, RACE_SPIKES,
    RAINBOW_CRUISE, REC_ADVENTURE, REC_ALLSTAR_CLEAR, RESULTS_PILLS, RESULTS_STOCK, RESULTS_TEAMS,
    RULES, RULES_ITEMS, RULES_PLUS, SELECT_CPUS, SELECT_KONGO, SELECT_RANDOM, SELECT_TEAMS,
    SHEIK_FOX_FOD, SOUND_JP, SOUND_SONGS, SOUND_SOUNDS, SPECIAL_INVISIBLE_YS, TIE3_TIME,
    TIE4_STOCK, TOU_BRACKET64, TOU_LIST, TOU_WINNER, TR_CPU, TR_ITEMS, TROPHY_COLLECTION,
    TROPHY_COLLECTION_ROWS, TROPHY_GALLERY, TROPHY_LOTTERY, TT_CLEAR, TT_DK_QUIT, TT_TARGETS,
    UN_HELD, UNLOCK_GW, UNLOCK_TIME, VENOM, YOSHIS_ISLAND, YOSHIS_ISLAND_64, ZE_CLIMB, ZE_LOOK,
    compare, compare_in, reflection, scaled, why,
};

fn identical(game: &Game, frame: u32) {
    compare(game, frame, Group::Standard, Expect::Identical);
}

macro_rules! frames {
    ($game:expr; $($name:ident $frame:literal),* $(,)?) => {
        $(
            #[test]
            #[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
            fn $name() {
                identical(&$game, $frame);
            }
        )*
    };
}

/// Frames whose first logged frame shows a magnifier bubble, whose copy comes from the frame
/// before: against a capture made with ssbm-rs's renderer running.
macro_rules! rendered_frames {
    ($game:expr; $($name:ident $frame:literal),* $(,)?) => {
        $(
            #[test]
            #[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
            fn $name() {
                compare_in(&$game, $frame, Group::Standard, Expect::Identical, Capture::Rendered);
            }
        )*
    };
}

frames!(FOX_FOX_FD;
    fox_fox_fd_140 140,
    fox_fox_fd_300 300,
    fox_fox_fd_600 600,
    fox_fox_fd_1000 1000,
    fox_fox_fd_2600 2600,
    fox_fox_fd_5000 5000,
    fox_fox_fd_8000 8000,
    fox_fox_fd_11000 11000,
    fox_fox_fd_14000 14000,
);
frames!(FOX_MARTH_BF;
    fox_marth_bf_600 600,
    fox_marth_bf_2600 2600,
    fox_marth_bf_6000 6000,
    fox_marth_bf_11000 11000,
);
frames!(FOX_MARTH_FD;
    fox_marth_fd_600 600,
    fox_marth_fd_6000 6000,
);
// The opening movie (its frames as `lbMthp` decodes them, the title's mark over them at 470,
// the logo's flash at 960) and the title screen, from power-on in a recorded session.
frames!(IDLE_MOVIE;
    movie_300 300,
    movie_470 470,
    movie_960 960,
    movie_3000 3000,
    movie_6000 6000,
);
frames!(IDLE_TITLE;
    title_300 300,
    title_700 700,
);
// The main menu (`menu-tour`): the main list as it opens (450) and its cursor moving down
// (650), the camera turned by the C stick (870), the records' list (1200), Special Melee's
// (2450), and the main list again after L, R and Start restart the menu (3700).
frames!(MENU_TOUR;
    menu_450 450,
    menu_650 650,
    menu_870 870,
    menu_1200 1200,
    menu_2450 2450,
    menu_3700 3700,
);
// The Data menu's movies (`gallery`): the Special Movie playing in the menu (1100, 1400) and
// How to Play (1960, 2400).
frames!(GALLERY;
    gallery_1100 1100,
    gallery_1400 1400,
    gallery_1960 1960,
    gallery_2400 2400,
);
// The VS rules (`rules`): the mode set to stock (1010), the stock count at 1 (1830), the
// extra rules hovered (3430); the item switch (`rules-items`): items toggled (1300), the
// frequency hovered (1620); the extra rules (`rules-plus`): the time limit changing (1090),
// the random stage switch with its locked stages (1990); name entry (`names`): a name typed
// (1200), a random name (1450), the erase prompt (1900); its kana (`names-kana`): a glyph's
// pop-up (1300), the katakana keys (1760).
frames!(RULES;
    rules_1010 1010,
    rules_1830 1830,
    rules_3430 3430,
);
frames!(RULES_ITEMS;
    rules_items_1300 1300,
    rules_items_1620 1620,
);
frames!(RULES_PLUS;
    rules_plus_1090 1090,
    rules_plus_1990 1990,
);
frames!(NAMES;
    names_1200 1200,
    names_1450 1450,
    names_1900 1900,
);
frames!(NAMES_KANA;
    names_kana_1300 1300,
    names_kana_1760 1760,
);
// The single-player modes' one-door select (`m6-select`): Classic's records (1200) and its CPU
// level's names scrolling (1470), the Stadium panels (Target Test's 1000, the Home-Run
// Contest's 1050), Training's CPU door (1300), and the clear marks (1250).
frames!(M6_CLASSIC;
    m6_classic_1200 1200,
    m6_classic_1470 1470,
);
frames!(M6_TARGET;
    m6_target_1000 1000,
);
frames!(M6_HOMERUN;
    m6_homerun_1050 1050,
);
frames!(M6_TRAINING;
    m6_training_1300 1300,
);
frames!(M6_TOUR_CLEARED;
    m6_tour_cleared_1250 1250,
);
// The debug mode's game over (`m6-endings/gameover2`): the question with the trophy on its
// stand (1420), the continue's coins paid and the score counting down (1620), and "no"'s end
// (2830).
frames!(M6E_GAMEOVER;
    m6e_gameover_1420 1420,
    m6e_gameover_1620 1620,
    m6e_gameover_2830 2830,
);
// The trophy fall and congratulations (`m6-endings-2/regend-adv`), against captures made with
// ssbm-rs's renderer running: the second cut (the trophy alone on its stand, 2030), the third
// (the trophies laid round, 2300) and congratulations' still (2600).
rendered_frames!(M6E2_REGEND_ADV;
    m6e2_regend_adv_2030 2030,
    m6e2_regend_adv_2300 2300,
    m6e2_regend_adv_2600 2600,
);
// The trophy fall's first cut, against logs started before its first copy (`RenderedSince`):
// Zelda falling with her fading copy and the frame's tinted copies (`regend-adv` 1850), the
// tinted copies fading out after her (1950), and Mr. Game & Watch's All-Star fall
// (`regend-gw` 1500). Every draw is the same but the copy sprite, which samples a copy both
// renderers keep on the GPU.
fn toy_fall(game: &Game, frame: u32, first: u32, same: [usize; 2]) {
    compare_in(
        game,
        frame,
        Group::Standard,
        Expect::SameImages {
            same,
            why: why::TOY_FALL_COPY,
        },
        Capture::RenderedSince(first),
    );
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn m6e2_regend_adv_1850() {
    toy_fall(&M6E2_REGEND_ADV, 1850, 1700, [717, 717]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn m6e2_regend_adv_1950() {
    toy_fall(&M6E2_REGEND_ADV, 1950, 1700, [717, 717]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn m6e2_regend_gw_1500() {
    toy_fall(&M6E2_REGEND_GW, 1500, 1400, [286, 286]);
}
// The staff roll (`m6-endings-3/staffroll-play`), against captures made with ssbm-rs's
// renderer running: the names flying past with shots fired at rest (1400) and the stick to the
// right (1600), the roll paused on B (2200), at Start's six times speed with the background
// dimmed (2550), and the hits' result (3200).
rendered_frames!(M6E3_STAFFROLL_PLAY;
    m6e3_staffroll_play_1400 1400,
    m6e3_staffroll_play_1600 1600,
    m6e3_staffroll_play_2200 2200,
    m6e3_staffroll_play_2550 2550,
    m6e3_staffroll_play_3200 3200,
);
// Classic's game over mode (`m6-endings-3/regend-movie`): the staff roll at Start's speed
// (1700) and the ending movie (2700).
rendered_frames!(M6E3_REGEND_MOVIE;
    m6e3_regend_movie_1700 1700,
    m6e3_regend_movie_2700 2700,
);
// Classic's first stage's intro (`m6-intros/classic-win`): the stage's number and the map
// coming in with Fox and his opponent posed (900), their names written in (950), and the
// screen held before the match (1000).
frames!(M6I_CLASSIC_WIN;
    m6i_classic_win_900 900,
    m6i_classic_win_950 950,
    m6i_classic_win_1000 1000,
);
// Adventure's first stage's intro (`m6-oracle/adventure2`): the map coming in over the stage
// with the camera flying along Mushroom Kingdom's route (900), the title over it (1000), and
// the camera further along with the map held (1150). The map and the title draw without the
// depth test (`fn_80187C9C`).
frames!(M6I_ADVENTURE2;
    m6i_adventure2_900 900,
    m6i_adventure2_1000 1000,
    m6i_adventure2_1150 1150,
);
// A human's Adventure run (`2026-10-06_12-04-48`): Kongo Jungle's intro (7470), and
// Corneria's, the city's parts that map 11's process lays out (27235, 27260), drawn once the
// session has drawn more archives than the address space's 32 MB hold (`mem::Space`); and with
// the fighters' shadows, as All-Star's match's, against references captured with ssbm-rs's
// renderer running, Mushroom Kingdom's route's arena with the joiners' tally (`fn_802FA6C4`,
// its icons' render callback `fn_802F95E8`) (4100) and Peach's Castle against Mario and Peach
// (6000).
frames!(REC_ADVENTURE;
    rec_adventure_7470 7470,
    rec_adventure_27235 27235,
    rec_adventure_27260 27260,
);

fn rec_adventure(frame: u32, same: [usize; 2]) {
    compare_in(
        &REC_ADVENTURE,
        frame,
        Group::Standard,
        Expect::SameImages {
            same,
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn rec_adventure_4100() {
    rec_adventure(4100, [2368, 2364]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn rec_adventure_6000() {
    rec_adventure(6000, [2162, 2162]);
}
// All-Star's intro from the debug mode: Captain Falcon asleep at the screen's place as the
// model comes in (1340), held (1400) and near its end (1460) (`m6-oracle/intro-allstar`), and
// the Ice Climbers, Nana beside and behind Popo (1640, 1700; `m6-allstar/intro-as-ic`).
frames!(O6_INTRO_ALLSTAR;
    o6_intro_allstar_1340 1340,
    o6_intro_allstar_1400 1400,
    o6_intro_allstar_1460 1460,
);
frames!(M6A_INTRO_AS_IC;
    m6a_intro_as_ic_1640 1640,
    m6a_intro_as_ic_1700 1700,
);
// Sarah's All-Star clear (`recordings/allstar-clear`): the first rest area, its hearts and the
// round's trophy on the stand, with the tally of the characters left (1600), and the last
// round's Mr. Game & Watches on Flat Zone (20500), against captures made with ssbm-rs's
// renderer running: identical images, every draw the same but those that sample the fighters'
// shadows' copies; the ending's trophy fall (22300), staff roll (23300) and congratulations
// (24500), identical.
fn rec_allstar_clear(frame: u32, same: [usize; 2]) {
    compare_in(
        &REC_ALLSTAR_CLEAR,
        frame,
        Group::Standard,
        Expect::SameImages {
            same,
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn rec_allstar_clear_1600() {
    rec_allstar_clear(1600, [810, 825]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn rec_allstar_clear_20500() {
    rec_allstar_clear(20500, [1893, 1888]);
}

frames!(REC_ALLSTAR_CLEAR;
    rec_allstar_clear_22300 22300,
    rec_allstar_clear_23300 23300,
    rec_allstar_clear_24500 24500,
);
// All-Star's first match won (`m6-1p-setups/as-win`): the stage clear screen growing over
// the match's blurred copy (6150), its tally with the bonus list (6250), and the screen still
// over the tinted copy (7500). The images are identical; the draws that differ only sample
// the fighters' shadows, as the debug VS coins'; the
// references are captured with ssbm-rs's renderer running, so the copies are drawn.
fn as_win(frame: u32, same: [usize; 2]) {
    compare_in(
        &M6P_AS_WIN,
        frame,
        Group::Standard,
        Expect::SameImages {
            same,
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn m6p_as_win_6150() {
    as_win(6150, [1720, 1720]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn m6p_as_win_6250() {
    as_win(6250, [1719, 1719]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn m6p_as_win_7500() {
    as_win(7500, [1699, 1699]);
}

// The rest area after the clear (`m6-1p-setups/as-win`): its first frames with the HUD's tally
// of the characters left in rows of five (9050), and the walk toward the portal (10300).
#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn m6p_as_win_9050() {
    as_win(9050, [805, 822]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn m6p_as_win_10300() {
    as_win(10300, [804, 821]);
}
// Training's menu over its match on Fountain of Dreams (`m6-training`): the item option on
// the Poké Ball with its name (`tr-items` 2250) and the computer player's behavior set to walk
// (`tr-cpu` 2230). As Celebi's, the first logged frame's water samples the reflection copied
// the frame before, so the references are captured with ssbm-rs's renderer; the images are
// identical, and the draws that differ only sample the fighters' shadows.
fn training(game: &Game, frame: u32, same: [usize; 2]) {
    compare_in(
        game,
        frame,
        Group::Standard,
        Expect::SameImages {
            same,
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn tr_items_2250() {
    training(&TR_ITEMS, 2250, [3774, 3770]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn tr_cpu_2230() {
    training(&TR_CPU, 2230, [4338, 4340]);
}

// Training's match after a computer player left and joined twenty times (`un-held` 11000):
// the fighters made over the freed fighters' blocks, drawn as ssbm-rs's renderer draws them.
#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn un_held_11000() {
    training(&UN_HELD, 11000, [4045, 4045]);
}
// The Options menus (`options-sessions/tour`): Rumble with the name rows (1100), Sound (2300),
// Screen Display with deflicker off (2790), the Options list (2870), Language (3080), Erase
// Data's list (3800, 3880) and its warning (3700).
frames!(OPTIONS_TOUR;
    options_1100 1100,
    options_2300 2300,
    options_2790 2790,
    options_2870 2870,
    options_3080 3080,
    options_3700 3700,
    options_3800 3800,
    options_3880 3880,
);
// The Data menu's screens (`data-sessions`): the special messages as they come in (1200) and
// scrolled (1900); the bonuses (1250, 1900); the other records (1300, 1700); the VS records'
// grid (1250), a cell's detail (1470), the names' grid (2050), the statistics (2750) and the
// rankings (3550); a special message announced (600) and the second (950).
frames!(DATA_INFO;
    data_info_1200 1200,
    data_info_1900 1900,
);
frames!(DATA_BONUS;
    data_bonus_1250 1250,
    data_bonus_1900 1900,
);
// `count`'s 2000 is the main list again after the Records lists, whose light loads light 1
// as the approach's screen does (below).
frames!(DATA_COUNT;
    data_count_1300 1300,
    data_count_1700 1700,
    data_count_2000 2000,
);
frames!(DATA_DIAGRAM;
    data_diagram_1250 1250,
    data_diagram_1470 1470,
    data_diagram_2050 2050,
    data_diagram_2750 2750,
    data_diagram_3550 3550,
);
frames!(DATA_PRIZE;
    data_prize_600 600,
    data_prize_950 950,
);
// The album (`data-sessions/album-browse` on seven snapshots made with HAL's encoder): the
// slots (761), the first page's thumbnails, each snapshot decoded into its image (851), the
// second page (1151), the snapshot's menu (1801) and its view whole (2101); and the delete's
// question (`album-delete` 1041).
frames!(DATA_ALBUM;
    data_album_761 761,
    data_album_851 851,
    data_album_1151 1151,
    data_album_1801 1801,
    data_album_2101 2101,
);
frames!(DATA_ALBUM_DELETE;
    data_album_delete_1041 1041,
);
// The debug mode (`debug-sessions`, at debug level 3): the debug menu's first page
// (menu-enter 700), Global Data Edit over it (menu-sub 950), the sound test's debug menu
// (sound-debug 700), and the debug VS match's readouts (vs-tools): the computer players'
// (1400), the animations' (1650), the stage hidden (2000), the first port's bonuses (2200),
// the second's over its background (2300) and the sound's (2700). Final Destination's floor
// shows the reflection copied the frame before, so the match's references are captured with
// ssbm-rs's renderer running; at 1400 and 1650, where a fighter's shadow falls on the floor,
// its draws differ only in the shadow textures' hashes, copies kept on the GPU, and the images
// are identical.
frames!(DEBUG_MENU_ENTER;
    debug_menu_enter_700 700,
);
frames!(DEBUG_MENU_SUB;
    debug_menu_sub_950 950,
);
frames!(DEBUG_SOUND;
    debug_sound_debug_700 700,
);
rendered_frames!(DEBUG_VS_TOOLS;
    debug_vs_tools_2000 2000,
    debug_vs_tools_2200 2200,
    debug_vs_tools_2300 2300,
    debug_vs_tools_2700 2700,
);
// The develop tools' collision bubbles (vs-bubbles, R and D-pad up): the fighters' hurt
// capsules alone (2000), a shield's bubble over them (2265), the bubbles over the models (2600)
// and a shield over the model (2790), drawn as `lbcollision.c` draws them. At 2790 a fighter's
// shadow falls on the floor: its draws differ only in the shadow textures' hashes, and the
// images are identical.
rendered_frames!(DEBUG_VS_BUBBLES;
    debug_vs_bubbles_2000 2000,
    debug_vs_bubbles_2265 2265,
    debug_vs_bubbles_2600 2600,
);

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn debug_vs_bubbles_2790() {
    compare_in(
        &DEBUG_VS_BUBBLES,
        2790,
        Group::Standard,
        Expect::SameImages {
            same: [1347, 1317],
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}
// The fighters' displays (vs-displays): Ness's absorber (2800), an intangible fighter (2830),
// the ledge points and dynamics chains (3100), the damage line and a green shell's stomp
// range (3500), Fox's reflector (3763, 4331), the computer player's aims over the fighters'
// hits (3870), the pickup ranges (4100) and the thrown hits (4400).
rendered_frames!(DEBUG_VS_DISPLAYS;
    debug_vs_displays_2800 2800,
    debug_vs_displays_2830 2830,
    debug_vs_displays_3100 3100,
    debug_vs_displays_3500 3500,
    debug_vs_displays_3763 3763,
    debug_vs_displays_3870 3870,
    debug_vs_displays_4100 4100,
    debug_vs_displays_4331 4331,
    debug_vs_displays_4400 4400,
);
// The coins' displays (vs-coins): the coins' ranges and the fighters' coin capsules over the
// models. A fighter's shadow falls on the floor: its draws differ only in the shadow textures'
// hashes, and the images are identical.
fn debug_vs_coins(frame: u32, same: [usize; 2]) {
    compare_in(
        &DEBUG_VS_COINS,
        frame,
        Group::Standard,
        Expect::SameImages {
            same,
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn debug_vs_coins_1600() {
    debug_vs_coins(1600, [1342, 1339]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn debug_vs_coins_1700() {
    debug_vs_coins(1700, [1339, 1332]);
}
// The scene loop's performance monitor (perf-monitor, X and D-pad right): in the opening movie
// with its lines (200), in the match (1100, 1300), and on again after a screenshot's wait
// (1700), its times the timeline's. In the match the fighters' shadows fall on the floor: the
// draws differ only in the shadow textures' hashes, and the images are identical.
rendered_frames!(DEBUG_PERF_MONITOR;
    debug_perf_monitor_200 200,
);
fn debug_perf_monitor(frame: u32, same: [usize; 2]) {
    compare_in(
        &DEBUG_PERF_MONITOR,
        frame,
        Group::Standard,
        Expect::SameImages {
            same,
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn debug_perf_monitor_1100() {
    debug_perf_monitor(1100, [1392, 1396]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn debug_perf_monitor_1300() {
    debug_perf_monitor(1300, [1396, 1395]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn debug_perf_monitor_1700() {
    debug_perf_monitor(1700, [1040, 1037]);
}
// The trophy collection's debug view (coll-debug, Z held at the entry): the first trophy with
// its place's text and the gallery's model beside it under the grey light (1000), moved with A
// and the stick (1550), the last trophy after L went round, moved with the D-pad and the camera
// turned (1900), zoomed and the eye raised (2250), and after Start and three more trophies
// (2550).
rendered_frames!(DEBUG_COLL;
    debug_coll_debug_1000 1000,
    debug_coll_debug_1550 1550,
    debug_coll_debug_1900 1900,
    debug_coll_debug_2250 2250,
    debug_coll_debug_2550 2550,
);
// The Yoshiki Test page's snapshot sprites (yoshiki-basic): the save's card's snapshot over the
// menus (2500), left drawn by Init through its new camera (3050), a second one over it (3500),
// and both after the page closed and opened again (3800); its snapshots read, swapped and
// deleted (yoshiki-snaps): the first and second of seven (1900, 2150), the one swapped into the
// third place (3150), and the third after the delete (3800).
rendered_frames!(DEBUG_YOSHIKI_BASIC;
    debug_yoshiki_basic_2500 2500,
    debug_yoshiki_basic_3050 3050,
    debug_yoshiki_basic_3500 3500,
    debug_yoshiki_basic_3800 3800,
);
rendered_frames!(DEBUG_YOSHIKI_SNAPS;
    debug_yoshiki_snaps_1900 1900,
    debug_yoshiki_snaps_2150 2150,
    debug_yoshiki_snaps_3150 3150,
    debug_yoshiki_snaps_3800 3800,
);

// The trophy gallery's Z view (gal-z, Z held at the entry; log frames run about 290 ahead of
// the holds' fields here): the trophy and its parameters' text (1000), moved with A held
// (1220), scaled with B held and the light changed (1470), the close-up with its info board
// and the performance monitor X and the D-pad's right turned on (1670), back in the view with
// the info board left up and port 2's frustum moved (1900), and the view again after the list
// (2230).
rendered_frames!(DEBUG_GAL_Z;
    debug_gal_z_1000 1000,
    debug_gal_z_1220 1220,
    debug_gal_z_1470 1470,
    debug_gal_z_1670 1670,
    debug_gal_z_1900 1900,
    debug_gal_z_2230 2230,
);
// The trophy gallery's L view (gal-l, L held at the entry): the trophy alone, unerased, its
// stands hidden (1000), another trophy after L and R (1265), the close-up with the fog on
// (1610), the fog off (1840), on again in a close-up (1990), and the view again after the
// list, the fog still on (2250).
rendered_frames!(DEBUG_GAL_L;
    debug_gal_l_1000 1000,
    debug_gal_l_1265 1265,
    debug_gal_l_1610 1610,
    debug_gal_l_1840 1840,
    debug_gal_l_1990 1990,
    debug_gal_l_2250 2250,
);
// The stage's displays (vs-stage, R and D-pad down): the blast zones, camera bounds, camera
// boxes and special points (2300), the fighters' ECBs, a hookshot's and the collision lines by
// kind (2600), by terrain (2900) and by ledge and platform (3200), drawn as `mplib.c` draws
// them.
rendered_frames!(DEBUG_VS_STAGE;
    debug_vs_stage_2300 2300,
    debug_vs_stage_2600 2600,
    debug_vs_stage_2900 2900,
    debug_vs_stage_3200 3200,
);
fn debug_vs_tools(frame: u32, same: [usize; 2]) {
    compare_in(
        &DEBUG_VS_TOOLS,
        frame,
        Group::Standard,
        Expect::SameImages {
            same,
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn debug_vs_tools_1400() {
    debug_vs_tools(1400, [1566, 1566]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn debug_vs_tools_1650() {
    debug_vs_tools(1650, [1356, 1382]);
}

// The sound test (`sound-sessions`): a song playing with its name and number (songs 5000), a
// category's icon in the categories (sounds 1450), a category's sounds with the sound's number
// (sounds 4400), and a song's name in Japanese (jp 1620).
frames!(SOUND_SONGS;
    sound_songs_5000 5000,
);
frames!(SOUND_SOUNDS;
    sound_sounds_1450 1450,
    sound_sounds_4400 4400,
);
frames!(SOUND_JP;
    sound_jp_1620 1620,
);
// The challenger approach's screen (`unlock-sessions/time`): coming in (2720) and waiting
// for A with Mewtwo's silhouette (2870). Light 1 is the scene's infinite light's specular
// object, whose half vector no joint drawn under its set sets: the transform unit's memory
// holds the zeros `hsdNew` left, not the half vector of the light an earlier scene had there.
frames!(UNLOCK_TIME;
    unlock_time_2720 2720,
    unlock_time_2870 2870,
);
// The notice approach from the single-player modes (`m6-unlocks`): after Training, the combos'
// trophy's notice (2400, 2600); after the Home-Run Contest, the best distances' total's trophy's
// (1950, 2150); after Event 1 cleared last, Final Destination's (7250, 7450); after Event 3,
// its trophy's (3900, 4000); after Event 37, Pichu's approach (17500, 17580).
frames!(M6U_TR_COMBO;
    m6u_tr_combo_2400 2400,
    m6u_tr_combo_2600 2600,
);
frames!(M6U_HR_TOTAL;
    m6u_hr_total_1950 1950,
    m6u_hr_total_2150 2150,
);
frames!(M6U_EV01_FD;
    m6u_ev01_fd_7250 7250,
    m6u_ev01_fd_7450 7450,
);
frames!(M6U_EV03_TROPHY;
    m6u_ev03_trophy_3900 3900,
    m6u_ev03_trophy_4000 4000,
);
frames!(M6U_EV37_PICHU;
    m6u_ev37_pichu_17500 17500,
    m6u_ev37_pichu_17580 17580,
);
// The approach's screen for Mr. Game & Watch (`unlock-sessions/gw`), the name texture's first
// frame, waiting for A (2890).
frames!(UNLOCK_GW;
    unlock_gw_2890 2890,
);
// Tournament mode (`tournament-sessions`): the setup's options with the bracket of 64 shown
// (1300) and the bracket as it comes in (1600); the entrants' list with the character grid
// (1400) and the name tags (1700), and a match's players' screen (2700); winner-out's
// match-up screen (1900). Their lights load light 1 as the approach's do.
frames!(TOU_BRACKET64;
    tou_bracket64_1300 1300,
    tou_bracket64_1600 1600,
);
frames!(TOU_LIST;
    tou_list_1400 1400,
    tou_list_1700 1700,
    tou_list_2700 2700,
);
frames!(TOU_WINNER;
    tou_winner_1900 1900,
);

// Celebi out of a Poké Ball (`celebi-sessions/celebi1`): sparkling as it lands on the left
// platform (4271) and as it takes off (4301). The first logged frame's water samples the
// reflection copied the frame before, so the references are captured with ssbm-rs's renderer;
// the images are identical, and the water's and platforms' draws differ only in the fighters'
// shadow textures' hashes, copies kept on the GPU.
fn celebi1(frame: u32, same: [usize; 2]) {
    compare_in(
        &CELEBI1,
        frame,
        Group::Standard,
        Expect::SameImages {
            same,
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn celebi1_4271() {
    celebi1(4271, [5519, 5519]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn celebi1_4301() {
    celebi1(4301, [5459, 5453]);
}
// The character and stage selects (`select-sessions`): port 1's door opening as its hand
// takes its coin (701), Mario's costumes changed with X and Y (796, 831), a computer
// player's door opened at random (946), the stage select's random icon hovered (1091) and
// its pick, the stage's name over "Now loading" (1121); three computer players' level sliders
// (1301, 1501); a team battle's doors (2001), a computer player's door with its handicap and
// level sliders (2201), a handicap moved (2401); Kongo Jungle hovered, its name and emblem
// (1181).
frames!(SELECT_RANDOM;
    select_random_701 701,
    select_random_796 796,
    select_random_831 831,
    select_random_946 946,
    select_random_1091 1091,
    select_random_1121 1121,
);
frames!(SELECT_CPUS;
    select_cpus_1301 1301,
    select_cpus_1501 1501,
);
frames!(SELECT_TEAMS;
    select_teams_2001 2001,
    select_teams_2201 2201,
    select_teams_2401 2401,
);
frames!(SELECT_KONGO;
    select_kongo_1181 1181,
);
// The results screen, against captures made with ssbm-rs's renderer running (the portraits
// are copies of the players' cameras' renders): a stock match's panel, rows and portraits as
// Mario poses (2451, 2901), the statistics' first page (3201), scrolled down (3271), the next
// pages (3361, 3401), "Ready for the next battle" once port 1 has pressed Start (3441), the
// character select after with the automatic handicaps the places moved (3521); a team
// battle's rows under its team's name (3301), its last player's statistics beside two
// players ready (3461), the team battle's character select after (3771).
/// A results frame against a capture made with ssbm-rs's renderer running, its draws the
/// same but those sampling an image copied before the log ([`common::why::RESULTS_COPY`]).
fn results_frame(game: &Game, frame: u32, same: usize) {
    compare_in(
        game,
        frame,
        Group::Standard,
        Expect::SameImages {
            same: [same, same],
            why: common::why::RESULTS_COPY,
        },
        Capture::Rendered,
    );
}

macro_rules! results_frames {
    ($game:expr; $($name:ident $frame:literal $same:literal),* $(,)?) => {
        $(
            #[test]
            #[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
            fn $name() {
                results_frame(&$game, $frame, $same);
            }
        )*
    };
}

results_frames!(RESULTS_STOCK;
    results_stock_2451 2451 1404,
    results_stock_2901 2901 1404,
    results_stock_3201 3201 1431,
    results_stock_3271 3271 1472,
    results_stock_3361 3361 1472,
    results_stock_3401 3401 1513,
    results_stock_3441 3441 1359,
);
rendered_frames!(RESULTS_STOCK;
    results_stock_3521 3521,
);
results_frames!(RESULTS_TEAMS;
    results_teams_3301 3301 2959,
    results_teams_3461 3461 2804,
);
rendered_frames!(RESULTS_TEAMS;
    results_teams_3771 3771,
);
// Dr. Mario's second pose on the results screen (`results-sessions/pills`), a pill in each
// hand: as the panel comes in (2701) and on the statistics' page (2901).
results_frames!(RESULTS_PILLS;
    results_pills_2701 2701 1532,
    results_pills_2901 2901 1532,
);
// The character select after a tie under the automatic handicap (`tie-sessions`), captured in
// ssbm-rs's original code, whose stack leftovers move the handicaps: Fox's handicap at 4
// after a tie for second (`tie3-time`, 9381 and 9551); Fox's at 4 and Kirby's at 6 after a
// tie for third (`tie4-stock`, 8741 and 8911).
frames!(TIE3_TIME;
    tie3_time_9381 9381,
    tie3_time_9551 9551,
);
frames!(TIE4_STOCK;
    tie4_stock_8741 8741,
    tie4_stock_8911 8911,
);
// Camera Mode (`special-sessions/camera-snaps`): the card check's slots and the blocks a
// snapshot may take (900), the character select's camera door (1000), the snapshot's offer
// over the match (2400), the match with the menu hidden (2640) and the camera's menu with
// its info boxes (2880); with no card (`camera-nocard`), the offer's message (3500).
frames!(CAMERA_SNAPS;
    camera_snaps_900 900,
    camera_snaps_1000 1000,
);

/// Camera Mode's match on Pokémon Stadium, captured with ssbm-rs's renderer running: the
/// snapshot is of the frame drawn (its blocks, the offer's text, come from the compressed
/// image), and the screens show feeds copied before the log. The images are identical; the
/// screens' draws differ only in their texture's hash, a copy kept on the GPU, `same` events
/// of each frame comparing the same.
macro_rules! camera_frames {
    ($game:expr; $($name:ident $frame:literal $same:literal),* $(,)?) => {
        $(
            #[test]
            #[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
            fn $name() {
                compare_in(
                    &$game,
                    $frame,
                    Group::Standard,
                    Expect::SameImages {
                        same: [$same, $same],
                        why: common::why::SCREEN_COPY,
                    },
                    Capture::Rendered,
                );
            }
        )*
    };
}
camera_frames!(CAMERA_SNAPS;
    camera_snaps_2400 2400 1584,
    camera_snaps_2640 2640 1486,
    camera_snaps_2880 2880 1528,
);
camera_frames!(CAMERA_NOCARD;
    camera_nocard_3500 3500 1862,
);
frames!(FLAT_ZONE;
    flat_zone_600 600,
    flat_zone_2800 2800,
    flat_zone_6000 6000,
    flat_zone_6900 6900,
    flat_zone_9000 9000,
    flat_zone_9300 9300,
);
frames!(POKE_FLOATS;
    poke_floats_900 900,
    poke_floats_6000 6000,
);

// The other stages' oracle matches, each at frames that show its own drawing.
frames!(BIG_BLUE;
    // The race from above: the cars on the track, the Falcon Flyer overhead.
    big_blue_900 900,
    // The cars on the track over the speed-blurred sky.
    big_blue_4500 4500,
    // The track gone: the floating platforms over the sea, the flying saucer.
    big_blue_11400 11400,
    // The pit area's white boost strip and the tower.
    big_blue_21600 21600,
);
rendered_frames!(BIG_BLUE;
    // The cars close up, and a magnifier bubble.
    big_blue_13500 13500,
);
frames!(BRINSTAR;
    // The acid low: the dark cavern and the tilting platforms.
    brinstar_900 900,
    // The acid risen over the stage, lighting it yellow.
    brinstar_2700 2700,
    // The acid down again, a platform on its stalk.
    brinstar_4200 4200,
);
frames!(BRINSTAR_DEPTHS;
    // The rock and its lit pods.
    brinstar_depths_900 900,
    // A fighter's light overlay set up without alpha (from native frame 3740).
    brinstar_depths_3743 3743,
    // Kraid rising and roaring behind the rock.
    brinstar_depths_9900 9900,
    // The rock turned, close up.
    brinstar_depths_13500 13500,
);
frames!(CORNERIA;
    // An Arwing flying past the Great Fox.
    corneria_1200 1200,
    // The city's towers close by, the Great Fox's engine.
    corneria_6300 6300,
);
frames!(FOURSIDE;
    // The helicopter over the rooftops.
    fourside_2700 2700,
    // The flying saucer and the moon.
    fourside_6000 6000,
);
frames!(GREAT_BAY;
    // The turtle's island in the water.
    great_bay_2100 2100,
    // Tingle's balloon over the lab.
    great_bay_6000 6000,
    // The moon close behind.
    great_bay_9900 9900,
);
frames!(GREEN_GREENS;
    // Whispy Woods' face.
    green_greens_900 900,
    // An apple and the star blocks.
    green_greens_5100 5100,
    // A bomb block in the stack.
    green_greens_6900 6900,
    // A tall stack of star blocks.
    green_greens_13500 13500,
);
frames!(HYRULE_TEMPLE;
    // The temple's arches and the sky.
    hyrule_temple_900 900,
    // The whole temple from afar.
    hyrule_temple_3900 3900,
    // The lower ruins under the bridge.
    hyrule_temple_6300 6300,
);
frames!(ICICLE_MOUNTAIN;
    // The start, a pine and the ice blocks.
    icicle_mountain_600 600,
    // A blowing wind band across the climb.
    icicle_mountain_3000 3000,
    // The rope bridges.
    icicle_mountain_9900 9900,
    // A rope bridge and snow-capped ledges.
    icicle_mountain_16200 16200,
);
rendered_frames!(ICICLE_MOUNTAIN;
    // The summit's ledges, and a magnifier bubble.
    icicle_mountain_26100 26100,
);
frames!(JUNGLE_JAPES;
    // The hut and the lanterns.
    jungle_japes_900 900,
);
rendered_frames!(JUNGLE_JAPES;
    // The river below, and a magnifier bubble.
    jungle_japes_2400 2400,
);
frames!(KONGO_JUNGLE;
    // The waterfall behind the raft.
    kongo_jungle_900 900,
    // The tree hut and the platforms.
    kongo_jungle_4200 4200,
);
frames!(KONGO_JUNGLE_64;
    // The tree trunks and the sunset.
    kongo_jungle_64_600 600,
    // The whole stage at sunset.
    kongo_jungle_64_3300 3300,
);
frames!(MUSHROOM_KINGDOM;
    // The pipes, bricks and question blocks.
    mushroom_kingdom_900 900,
    // The lifts and the castle.
    mushroom_kingdom_4800 4800,
);
frames!(MUSHROOM_KINGDOM_2;
    // The waterfall, a Pidgit and a KO's beam of light.
    mushroom_kingdom_2_900 900,
    // Birdo on the cliff.
    mushroom_kingdom_2_8100 8100,
);
frames!(MUTE_CITY;
    // The start gate and its GO sign.
    mute_city_900 900,
    // The orange boost track.
    mute_city_2100 2100,
    // The cars racing past.
    mute_city_17400 17400,
);
rendered_frames!(MUTE_CITY;
    // The pink tiled track, and a magnifier bubble.
    mute_city_2700 2700,
);
frames!(ONETT;
    // Mr. Game & Watch's outline, body and silhouette, and his light overlay set up
    // without alpha (native frames 147 and 148).
    onett_150 150,
    // A car on the street.
    onett_2400 2400,
);
rendered_frames!(ONETT;
    // The car warning sign, and a magnifier bubble.
    onett_9600 9600,
);
frames!(PEACHS_CASTLE;
    // A Bullet Bill over the castle.
    peachs_castle_4200 4200,
    // The switch blocks.
    peachs_castle_7800 7800,
);
frames!(RAINBOW_CRUISE;
    // The ship.
    rainbow_cruise_600 600,
    // The rainbow and the moving carpets.
    rainbow_cruise_3000 3000,
    // The swinging platform.
    rainbow_cruise_4200 4200,
    // The checkered block.
    rainbow_cruise_5400 5400,
    // The path above the clouds.
    rainbow_cruise_6300 6300,
);
frames!(VENOM;
    // A fighter's light overlay set up without alpha (native frames 774 and 775).
    venom_777 777,
    // The Great Fox's wing in the planet's orange light.
    venom_2700 2700,
    // The tunnel's blue light.
    venom_5400 5400,
);
frames!(YOSHIS_ISLAND;
    // The rotating blocks and the pipe.
    yoshis_island_1200 1200,
    // The blocks close up.
    yoshis_island_3000 3000,
);
frames!(YOSHIS_ISLAND_64;
    // The heart and the fruit behind the stage.
    yoshis_island_64_600 600,
    // The whole stage and its clouds.
    yoshis_island_64_3300 3300,
);

// Metal fighters (the Metal Box's shared material, `ft_804D6580`) and refracted ones (the
// Cloaking Device and Invisible Melee: refraction's indirect maps and screen copy).
frames!(ITEMS_POWERUPS_PS;
    // Metal Mr. Game & Watch.
    items_powerups_ps_2100 2100,
);
rendered_frames!(ITEMS_POWERUPS_PS;
    // Metal Mewtwo, and the screen's feed copied before the log.
    items_powerups_ps_7300 7300,
);
frames!(ITEMS_POWERUPS_BF;
    // Metal Pichu.
    items_powerups_bf_5300 5300,
);
rendered_frames!(ITEMS_POWERUPS_BF;
    // Bowser cloaked: refraction's screen copy and indirect map.
    items_powerups_bf_2803 2803,
);
frames!(ITEMS_POWERUPS_FD;
    // Metal Zelda.
    items_powerups_fd_7000 7000,
);
rendered_frames!(METAL_CLOAK_RC;
    // Metal Mr. Game & Watch, and a magnifier bubble.
    metal_cloak_rc_603 603,
    // Metal and cloaked: refracted, as he showed white in the player.
    metal_cloak_rc_703 703,
    // Metal and cloaked, between the cloak's refraction flickers: metal.
    metal_cloak_rc_733 733,
    // Metal and cloaked, refracted, near where Sarah's capture paused.
    metal_cloak_rc_1003 1003,
);
rendered_frames!(SPECIAL_INVISIBLE_YS;
    // Both fighters refracted, the whole match.
    special_invisible_ys_603 603,
);

/// Slippi's name plates, which its playback draws and melee-sim doesn't (docs/player.md's
/// open issues).
const NAME_PLATES: &str = "Slippi's name plates, which melee-sim doesn't draw";

/// Fox vs Sheik's frames: identical but for the name plates, as recorded here.
fn fox_sheik(frame: u32, images: [&'static str; 2]) {
    compare(
        &FOX_SHEIK_FD,
        frame,
        Group::Standard,
        Expect::Known {
            images,
            notes: &[],
            why: NAME_PLATES,
        },
    );
}

#[test]
#[ignore = "needs ssbm-rs's references, the replay and the disc; run with --ignored"]
fn fox_sheik_fd_600() {
    fox_sheik(
        600,
        [
            "image: PSNR 31.65 dB, 0.535% off by 8, 0.366% off by 32: OUT OF TOLERANCE",
            "image: PSNR 32.22 dB, 0.732% off by 8, 0.557% off by 32: OUT OF TOLERANCE",
        ],
    );
}

#[test]
#[ignore = "needs ssbm-rs's references, the replay and the disc; run with --ignored"]
fn fox_sheik_fd_1400() {
    fox_sheik(
        1400,
        [
            "image: PSNR 32.85 dB, 0.816% off by 8, 0.355% off by 32: OUT OF TOLERANCE",
            "image: PSNR 32.79 dB, 0.802% off by 8, 0.329% off by 32: OUT OF TOLERANCE",
        ],
    );
}

#[test]
#[ignore = "needs ssbm-rs's references, the replay and the disc; run with --ignored"]
fn fox_sheik_fd_6000() {
    fox_sheik(
        6000,
        [
            "image: PSNR 31.49 dB, 0.606% off by 8, 0.223% off by 32: OUT OF TOLERANCE",
            "image: PSNR 31.52 dB, 0.610% off by 8, 0.220% off by 32: OUT OF TOLERANCE",
        ],
    );
}

#[test]
#[ignore = "needs ssbm-rs's references, the replay and the disc; run with --ignored"]
fn sheik_fox_fod_600() {
    compare(&SHEIK_FOX_FOD, 600, Group::Standard, Expect::Identical);
}

/// Fountain of Dreams with the reflection size code: the reflection camera, its viewport the
/// whole screen, erases in the white the stage sets, not the game camera's background color.
#[test]
#[ignore = "needs ssbm-rs's references, the replay and the disc; run with --ignored"]
fn pc_izumi_size_600() {
    compare(&PC_IZUMI_SIZE, 600, Group::Standard, Expect::Identical);
}

#[test]
#[ignore = "needs ssbm-rs's references, the replay and the disc; run with --ignored"]
fn pc_izumi_size_6000() {
    compare(&PC_IZUMI_SIZE, 6000, Group::Standard, Expect::Identical);
}

#[test]
#[ignore = "needs ssbm-rs's references, the replay and the disc; run with --ignored"]
fn fox_sheik_ys_600() {
    compare(&FOX_SHEIK_YS, 600, Group::Standard, Expect::Identical);
}

#[test]
#[ignore = "needs ssbm-rs's references, the replay and the disc; run with --ignored"]
fn fox_sheik_dl_600() {
    compare(&FOX_SHEIK_DL, 600, Group::Standard, Expect::Identical);
}

/// Four players: the stock icons' palettes (indexed entries only), Sheik's needle's
/// highlight, and the background's state cache reset pinned.
#[test]
#[ignore = "needs ssbm-rs's references, the replay and the disc; run with --ignored"]
fn four_fd_600() {
    compare(&FOUR_FD, 600, Group::Standard, Expect::Identical);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn fox_marth_ps_600() {
    compare(&FOX_MARTH_PS, 600, Group::Standard, Expect::Identical);
}

/// Pokémon Stadium transformed: the fire stage, with Fox's magnifier bubble from the first
/// logged frame, whose copy comes from the frame before (so the capture ran ssbm-rs's renderer).
#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn fox_marth_ps_6000_fire() {
    compare_in(
        &FOX_MARTH_PS,
        6000,
        Group::Standard,
        Expect::Identical,
        Capture::Rendered,
    );
}

#[test]
#[ignore = "needs the replay and the disc; run with --ignored"]
fn scaled_sheik_fox_fod_600() {
    scaled(&SHEIK_FOX_FOD, 600, Group::Standard);
}

#[test]
#[ignore = "needs the disc; run with --ignored"]
fn scaled_fox_marth_ps_600() {
    scaled(&FOX_MARTH_PS, 600, Group::Standard);
}

#[test]
#[ignore = "needs the disc; run with --ignored"]
fn scaled_fox_marth_bf_600() {
    scaled(&FOX_MARTH_BF, 600, Group::Standard);
}

/// Refraction widescreen: the screen copy's projection widens with the camera.
#[test]
#[ignore = "needs ssbm-rs's local replays and the disc; run with --ignored"]
fn scaled_special_invisible_ys_603() {
    scaled(&SPECIAL_INVISIBLE_YS, 603, Group::Standard);
}

/// Pokémon Stadium's screen showing its feed, a copy of the frame: in 16:9 the feed copies the
/// 73:60 picture's columns, so the screen shows what it shows at 73:60.
#[test]
#[ignore = "needs the disc; run with --ignored"]
fn scaled_fox_marth_ps_9000() {
    scaled(&FOX_MARTH_PS, 9000, Group::Standard);
}

/// Fountain of Dreams' water in 16:9, the reflection in the added sides (the right at 2600,
/// the left at 6000): its coordinates stay inside the reflection's image.
#[test]
#[ignore = "needs the replay and the disc; run with --ignored"]
fn widescreen_reflection_sheik_fox_fod_2600() {
    reflection(&SHEIK_FOX_FOD, 2600, Group::Standard);
}

#[test]
#[ignore = "needs the replay and the disc; run with --ignored"]
fn widescreen_reflection_sheik_fox_fod_6000() {
    reflection(&SHEIK_FOX_FOD, 6000, Group::Standard);
}

// The trophy screens: the gallery's view under its first light (1250), the second light's back
// (1340), the third's background sprite (1400), the close-up (2330), its axes as the camera
// pans (2440) and the list (3300); the
// lottery's machine with a bet (1400), the draw (1700) and the trophy shown (2000); the
// collection's 196 trophies in rings (1300), turned (1700) and zoomed (2500), and in rows
// (1300).
frames!(TROPHY_GALLERY;
    trophy_gallery_1250 1250,
    trophy_gallery_1340 1340,
    trophy_gallery_1400 1400,
    trophy_gallery_2330 2330,
    trophy_gallery_2440 2440,
    trophy_gallery_3300 3300,
);
frames!(TROPHY_LOTTERY;
    trophy_lottery_1400 1400,
    trophy_lottery_1700 1700,
    trophy_lottery_2000 2000,
);
frames!(TROPHY_COLLECTION;
    trophy_collection_1300 1300,
    trophy_collection_1700 1700,
    trophy_collection_2500 2500,
);
frames!(TROPHY_COLLECTION_ROWS;
    trophy_collection_rows_1300 1300,
);
// The Home-Run Contest (milestone 6's task 14): the platform, the bat and the start's label
// (hr-long 1000), Sandbag in flight with the counter running (1240), the field's labels past
// 300 feet (1420), the counter's new record (1540), and no hit's sign (hr-miss 1850).
// Against captures made with ssbm-rs's renderer running (headless, the log's first frame
// samples textures the frames before it copied). Fox's and Sandbag's shadows fall on the
// stage: their draws differ only in the shadow textures' hashes, and the images are
// identical.
fn home_run(game: &Game, frame: u32, same: [usize; 2]) {
    compare_in(
        game,
        frame,
        Group::Standard,
        Expect::SameImages {
            same,
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}

macro_rules! home_run_frames {
    ($game:expr; $($name:ident $frame:literal $same:expr),* $(,)?) => {
        $(
            #[test]
            #[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
            fn $name() {
                home_run(&$game, $frame, $same);
            }
        )*
    };
}

home_run_frames!(HR_LONG;
    hr_long_1000 1000 [1754, 1758],
    hr_long_1240 1240 [1503, 1502],
    hr_long_1420 1420 [1635, 1635],
    hr_long_1540 1540 [1637, 1637],
);
home_run_frames!(HR_MISS;
    hr_miss_1850 1850 [1725, 1725],
);
// Mushroom Kingdom's adventure route (milestone 6's task 16, debug VS at debug level 3, Fox
// alone): the debug item menu's Koopas, a Paratroopa and a green shell falling at the route's
// start (mk-enemies 3000), brick blocks and the route's cliffs (mk-route 2950), a Paratroopa's
// wings and Koopas over the water (mk-route 3721), the water's splash as Fox drops into a pit
// (mk-fall2 3621) and his respawn platform (mk-fall2 3858).
frames!(MK_ENEMIES;
    mk_enemies_3000 3000,
);
frames!(MK_ROUTE;
    mk_route_2950 2950,
    mk_route_3721 3721,
);
frames!(MK_FALL;
    mk_fall_3621 3621,
    mk_fall_3858 3858,
);
// The F-Zero race (milestone 6's task 19, debug VS at debug level 3, Link running): the hazard
// sign as the first car nears (fz-run 2201), the cars driving past (3401) and one hitting Link
// (4201); the F-Zero cutscene's racers on the race's track (fz-cutscene 3131 and 3191).
frames!(FZ_RUN;
    fz_run_2201 2201,
    fz_run_3401 3401,
    fz_run_4201 4201,
);
frames!(FZ_CUTSCENE;
    fz_cutscene_3131 3131,
    fz_cutscene_3191 3191,
);

// The cutscenes (milestone 6's task 4, the debug cutscene mode at debug level 3), two frames
// each: Luigi at Peach's Castle (1150, 1300), Brinstar's warning (1500, 1650) and the planet's
// explosion (1800, 2000) and Giga Bowser defeated, his trophy fallen on its stand (4700, 4800),
// identical.
frames!(M6C_CUTSCENES;
    m6c_luigi_1150 1150,
    m6c_luigi_1300 1300,
    m6c_brinstar_1500 1500,
    m6c_brinstar_1650 1650,
    m6c_explosion_1800 1800,
    m6c_explosion_2000 2000,
    m6c_defeat_4700 4700,
    m6c_defeat_4800 4800,
);

/// A cutscene frame with the fighters' shadows (and, on Final Destination, its floor's
/// reflection), against a capture made with ssbm-rs's renderer running: the three Kirbys and
/// the giant Kirby on Green Greens (2200, 2250; 2320, 2380), metal Mario and Luigi on
/// Battlefield (3500, 3700), Bowser's trophy falling and the character posing on Final
/// Destination (3850, 4000), Bowser's trophy before Giga Bowser's transformation (4200, 4300)
/// and Giga Bowser come out of it (4500).
fn m6c_shadowed(frame: u32, same: [usize; 2]) {
    compare_in(
        &M6C_CUTSCENES,
        frame,
        Group::Standard,
        Expect::SameImages {
            same,
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}

// The hands (milestone 6's tasks 3 and 12, debug VS at debug level 3, Link against them on
// Final Destination): Master Hand alone (mh-1p 3420), Crazy Hand alone pointing over Link
// (ch-probe 3180), and both hands (ch-mh-1p): Crazy Hand's entrance under the cutscene camera
// (3150, which a mutation of its interest fails), Link held (4100), a hand flat over the
// stage in a tag attack's stretch (4800), and a hand's sparks over the stage's boss backdrop
// with Link in a magnifier bubble (11080). Against captures with ssbm-rs's renderer running
// (each first frame samples copies of the frame before): identical images, every draw the
// same but the two that sample the fighters' shadows' copy.
#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn mh_1p_3420() {
    compare_in(
        &MH_1P,
        3420,
        Group::Standard,
        Expect::SameImages {
            same: [1118, 1126],
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}

/// Master Hand's attacks (milestone 6's task 3, debug VS at debug level 3, Link against him on
/// Final Destination), a frame each where it draws something its own, against captures with
/// ssbm-rs's renderer running, as `mh_1p_3420`: in `mh-1p` the sweep (3150), the poke (3600),
/// the finger gun's bullets (4045), the ram's dust from behind (4300), the walk's shots (4720),
/// the crush's clouds (5000), the finger beams (5500), the jet's shadow as he flies over (7050),
/// the flat slap's dust (7550) and the slap with Link in a magnifier bubble (11350); the drill's
/// sparks (`mh-1p-wild` 4450); the punch's explosions (`mh-probe` 3750), his squeeze of Link
/// (5450) and the slam (5600); and the throw (`mh-3p` 3540).
fn master_hand(game: &Game, frame: u32, same: [usize; 2]) {
    compare_in(
        game,
        frame,
        Group::Standard,
        Expect::SameImages {
            same,
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}

macro_rules! master_hand {
    ($($name:ident $game:ident $frame:literal $same:expr),* $(,)?) => {
        $(
            #[test]
            #[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
            fn $name() {
                master_hand(&$game, $frame, $same);
            }
        )*
    };
}

master_hand!(
    mh_1p_sweep_3150 MH_1P 3150 [1090, 1092],
    mh_1p_poke_3600 MH_1P 3600 [1109, 1111],
    mh_1p_gun_4045 MH_1P 4045 [1086, 1086],
    mh_1p_ram_4300 MH_1P 4300 [1480, 1484],
    mh_1p_shots_4720 MH_1P 4720 [1147, 1147],
    mh_1p_crush_5000 MH_1P 5000 [1191, 1193],
    mh_1p_beams_5500 MH_1P 5500 [1137, 1137],
    mh_1p_jet_7050 MH_1P 7050 [2243, 2228],
    mh_1p_flat_slap_7550 MH_1P 7550 [1199, 1199],
    mh_1p_slap_11350 MH_1P 11350 [1106, 541],
    mh_1p_wild_drill_4450 MH_1P_WILD 4450 [1247, 1228],
    mh_probe_punch_3750 MH_PROBE 3750 [1231, 1235],
    mh_probe_squeeze_5450 MH_PROBE 5450 [564, 564],
    mh_probe_slam_5600 MH_PROBE 5600 [1122, 1124],
    mh_3p_throw_3540 MH_3P 3540 [2128, 2129],
);

/// Crazy Hand's attacks (milestone 6's task 12), as Master Hand's: alone over Link
/// (`ch-probe`), the poke's dust (2990), a bomb dropped (3600) and the crush's sparks (4450);
/// with Master Hand (`ch-mh-1p`), an explosion by him (3450), his beam (3880), Link held
/// (4030), a bomb (5440), his crawl (6150), the sparks after a tag attack (8700) and an
/// explosion with Link in a magnifier bubble (9600).
macro_rules! crazy_hand {
    ($($name:ident $game:ident $frame:literal $same:expr),* $(,)?) => {
        $(
            #[test]
            #[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
            fn $name() {
                master_hand(&$game, $frame, $same);
            }
        )*
    };
}

crazy_hand!(
    ch_probe_poke_2990 CH_PROBE 2990 [1133, 1123],
    ch_probe_bomb_3600 CH_PROBE 3600 [1071, 1071],
    ch_probe_crush_4450 CH_PROBE 4450 [1543, 1543],
    ch_mh_1p_explosion_3450 CH_MH_1P 3450 [1699, 1702],
    ch_mh_1p_beam_3880 CH_MH_1P 3880 [732, 731],
    ch_mh_1p_held_4030 CH_MH_1P 4030 [1322, 1324],
    ch_mh_1p_bomb_5440 CH_MH_1P 5440 [2339, 2362],
    ch_mh_1p_crawl_6150 CH_MH_1P 6150 [1380, 1380],
    ch_mh_1p_sparks_8700 CH_MH_1P 8700 [1851, 1848],
    ch_mh_1p_explosion_9600 CH_MH_1P 9600 [1242, 1309],
);

macro_rules! m6c_shadowed {
    ($($name:ident $frame:literal $same:expr),* $(,)?) => {
        $(
            #[test]
            #[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
            fn $name() {
                m6c_shadowed($frame, $same);
            }
        )*
    };
}

m6c_shadowed!(
    m6c_kirbys_2200 2200 [2286, 2286],
    m6c_kirbys_2250 2250 [2395, 2398],
    m6c_giant_kirby_2320 2320 [1952, 1952],
    m6c_giant_kirby_2380 2380 [1952, 1952],
    m6c_metal_3500 3500 [1310, 1310],
    m6c_metal_3700 3700 [1324, 1328],
    m6c_trophy_3850 3850 [1193, 1195],
    m6c_trophy_4000 4000 [1168, 1167],
    m6c_giga_4200 4200 [1125, 1125],
    m6c_giga_4300 4300 [1149, 1157],
    m6c_giga_4500 4500 [2035, 2049],
);

/// Star Fox's cutscene (2450, 2900): the screen's erase takes the background color whose alpha
/// `Camera_GetBackgroundColor` never writes, which the erase doesn't write to the screen.
fn m6c_starfox(frame: u32, same: [usize; 2]) {
    compare(
        &M6C_CUTSCENES,
        frame,
        Group::Standard,
        Expect::SameImages {
            same,
            why: why::ERASE_ALPHA,
        },
    );
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn m6c_starfox_2450() {
    m6c_starfox(2450, [1078, 1080]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn m6c_starfox_2900() {
    m6c_starfox(2900, [1102, 1102]);
}

// The race (milestone 6's task 20, debug VS at debug level 3): Link and Mario at the start
// under the course's lights (race-look 2060) and Mario's KO as the blast zones follow Link
// (race-look 2460); Fox in the first spike pit (race-spikes 4600) and against a pillar
// (race-pillar 4510).
frames!(RACE_LOOK;
    race_look_2060 2060,
    race_look_2460 2460,
);
frames!(RACE_SPIKES;
    race_spikes_4600 4600,
);
frames!(RACE_PILLAR;
    race_pillar_4510 4510,
);

// Brinstar's escape (milestone 6's task 18, debug VS at debug level 3): Link and Mario at the
// shaft's foot under the stage's lights (ze-look 2300), Fox's Firefox past the lower point light
// (ze-climb 2900), the shaft's upper part shaking once the camera's timer has run out (ze-climb
// 3500), the way out with Mario back from a KO below the camera (ze-climb 3800) and the match's
// end on it (ze-climb 4000).
frames!(ZE_LOOK;
    ze_look_2300 2300,
);
frames!(ZE_CLIMB;
    ze_climb_2900 2900,
    ze_climb_3500 3500,
    ze_climb_3800 3800,
    ze_climb_4000 4000,
);
// The Underground Maze (milestone 6's task 17, debug VS at debug level 3): Fox's Firefox up the
// maze under the fighters' lights it moves with him, a ReDead on a ledge (mz-exit 2400) and the
// match's end at the way out (mz-exit 2650); an arena opened, its portal swaying in
// (mz-arena 2870); a Like Like chewing Fox inside it (mz-capture 3000); a beaten Like Like
// flung toward the screen (mz-kill 3480).
frames!(MZ_EXIT;
    mz_exit_2400 2400,
    mz_exit_2650 2650,
);
frames!(MZ_ARENA;
    mz_arena_2870 2870,
);
frames!(MZ_CAPTURE;
    mz_capture_3000 3000,
);
frames!(MZ_KILL;
    mz_kill_3480 3480,
);
// Giga Bowser and the Wire Frames (`m6-giga-bowser`), against captures made with ssbm-rs's
// renderer running (Final Destination's floor samples the reflection copied the frame before):
// identical images, the floor's two draws that sample the fighters' shadows differing by their
// textures' hashes alone, as the debug VS match's.
fn giga_bowser(game: &Game, frame: u32, same: [usize; 2]) {
    compare_in(
        game,
        frame,
        Group::Standard,
        Expect::SameImages {
            same,
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn ch_probe_3180() {
    compare_in(
        &CH_PROBE,
        3180,
        Group::Standard,
        Expect::SameImages {
            same: [1070, 1076],
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn gb_gk_5060() {
    giga_bowser(&GB_GK, 5060, [1269, 1269]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn gb_gk_5230() {
    giga_bowser(&GB_GK, 5230, [1218, 1218]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn gb_gk_5330() {
    giga_bowser(&GB_GK, 5330, [1253, 1249]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn gb_boy_3545() {
    giga_bowser(&GB_BOY, 3545, [895, 888]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn gb_girl_3557() {
    giga_bowser(&GB_GIRL, 3557, [904, 906]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn ch_mh_1p_3150() {
    compare_in(
        &CH_MH_1P,
        3150,
        Group::Standard,
        Expect::SameImages {
            same: [1400, 1400],
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}
#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn ch_mh_1p_4100() {
    compare_in(
        &CH_MH_1P,
        4100,
        Group::Standard,
        Expect::SameImages {
            same: [1356, 1359],
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}
#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn ch_mh_1p_4800() {
    compare_in(
        &CH_MH_1P,
        4800,
        Group::Standard,
        Expect::SameImages {
            same: [1315, 1346],
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}
#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn ch_mh_1p_11080() {
    compare_in(
        &CH_MH_1P,
        11080,
        Group::Standard,
        Expect::SameImages {
            same: [1101, 1101],
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}

// The adventure's Icicle Mountain (milestone 6's task 9, debug VS at debug level 3): Link and
// Mario on the climb's first fields (im-look 2300) and a Polar Bear walking down a field with the
// spawner's Topis (im-look 4300); the debug item menu's Polar Bears around Fox (im-jump 2950) and
// knocked about by his smashes (im-bears 3500).
frames!(IM_LOOK;
    im_look_2300 2300,
    im_look_4300 4300,
);
frames!(IM_JUMP;
    im_jump_2950 2950,
);
frames!(IM_BEARS;
    im_bears_3500 3500,
);
// Target Test (milestone 6's task 6): Mario's target stage in the debug VS tour (o6-targets
// 1500) and the Ice Climbers' with its blocks (14900); Donkey Kong's from the mode, its targets'
// counter and timer (tt-dk-quit 1300, and 1420 after the jabs). At 1500 a fighter's shadow
// falls on the floor (against a capture made with ssbm-rs's renderer running): its draws differ
// only in the shadow textures' hashes, and the images are identical.
frames!(TT_TARGETS;
    tt_targets_14900 14900,
);
// Samus's stage cleared from the mode (tt-clear): mid-run with six targets left in the
// counter (2000) and the clear's "Complete!" over the stage (2990), against captures made with
// ssbm-rs's renderer running (headless, each log's first frame samples a copy made before the
// log): the images identical, the draws that sample Samus's shadow different only in the
// shadow textures' hashes; and the select again with the new record as the high score (3100).
frames!(TT_CLEAR;
    tt_clear_3100 3100,
);

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn tt_clear_2000() {
    compare_in(
        &TT_CLEAR,
        2000,
        Group::Standard,
        Expect::SameImages {
            same: [1063, 1063],
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn tt_clear_2990() {
    compare_in(
        &TT_CLEAR,
        2990,
        Group::Standard,
        Expect::SameImages {
            same: [1059, 1059],
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn tt_targets_1500() {
    compare_in(
        &TT_TARGETS,
        1500,
        Group::Standard,
        Expect::SameImages {
            same: [1238, 1238],
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}
// Multi-Man Melee (milestone 6's task 15), from the main menu: the list on 10-Man (mm-10-win 780),
// its select (950), the match with the counter and the Wire Frames joining (1800), the clear's
// "Complete!" (2760) and the select again with the time as the record (3000); 15-Minute
// Melee's timer counting down and its counter's KOs (mm-15min 1800).
// The match's frames against captures made with ssbm-rs's renderer running (ref-*/rendered):
// the fighters' shadows sample copies kept on the GPU.
frames!(MM_10_WIN;
    mm_10_win_780 780,
    mm_10_win_950 950,
    mm_10_win_3000 3000,
);

/// A Multi-Man match's frame against a capture with ssbm-rs's renderer running: identical
/// images, the draws that sample the fighters' shadows different only in their textures'
/// hashes.
fn mm_rendered(game: &common::Game, frame: u32, same: [usize; 2]) {
    compare_in(
        game,
        frame,
        Group::Standard,
        Expect::SameImages {
            same,
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn mm_10_win_1800() {
    mm_rendered(&MM_10_WIN, 1800, [1693, 1720]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn mm_10_win_2760() {
    mm_rendered(&MM_10_WIN, 2760, [964, 964]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn mm_15min_1800() {
    mm_rendered(&MM_15MIN, 1800, [1434, 1460]);
}
// Event Match (milestone 6's task 21): the Event list's first page and Event 12's one-door
// select (ev12 720, 1090), the list's last page on Event 43 (ev43 1050), and two matches:
// Event 43 on Big Blue (ev43 1750) and Trophy Tussle 3 on the third trophy stage (ev47
// 2100). In the matches the fighters' shadows sample textures copied the frame before the
// log, so their references are captured with ssbm-rs's renderer running: the images are
// identical, and the draws that differ only sample the shadows.
frames!(EV12;
    ev12_720 720,
    ev12_1090 1090,
);
frames!(EV43;
    ev43_1050 1050,
);

fn event_match(game: &Game, frame: u32, same: [usize; 2]) {
    compare_in(
        game,
        frame,
        Group::Standard,
        Expect::SameImages {
            same,
            why: why::SHADOW_COPY,
        },
        Capture::Rendered,
    );
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn ev43_1750() {
    event_match(&EV43, 1750, [2784, 2784]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn ev47_2100() {
    event_match(&EV47, 2100, [1747, 1744]);
}

// Event Match's third round: Trophy Tussles 1 and 2 on the first and second trophy stages
// (m6-event-3's ev14 2000 and ev26 2100), captured with ssbm-rs's renderer running.
#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn e3_ev14_2000() {
    event_match(&EV14, 2000, [2384, 2384]);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn e3_ev26_2100() {
    event_match(&EV26, 2100, [2440, 2415]);
}

// Event Match's fourth round: Event 22's crowd and its counter in the 1P HUD (m6-event-3's
// ev22 2000), captured with ssbm-rs's renderer running.
#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn e4_ev22_2000() {
    event_match(&EV22, 2000, [1775, 1775]);
}

frames!(TT_DK_QUIT;
    tt_dk_quit_1300 1300,
    tt_dk_quit_1420 1420,
);
