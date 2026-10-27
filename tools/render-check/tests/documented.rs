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
    BIG_BLUE, BRINSTAR, BRINSTAR_DEPTHS, CAMERA_NOCARD, CAMERA_SNAPS, CORNERIA, Capture,
    DATA_ALBUM, DATA_ALBUM_DELETE, DATA_BONUS, DATA_COUNT, DATA_DIAGRAM, DATA_INFO, DATA_PRIZE,
    Expect, FLAT_ZONE, FOUR_FD, FOURSIDE, FOX_FOX_FD, FOX_MARTH_BF, FOX_MARTH_FD, FOX_MARTH_PS,
    FOX_SHEIK_DL, FOX_SHEIK_FD, FOX_SHEIK_YS, GALLERY, GREAT_BAY, GREEN_GREENS, Game, Group,
    HYRULE_TEMPLE, ICICLE_MOUNTAIN, IDLE_MOVIE, IDLE_TITLE, ITEMS_POWERUPS_BF, ITEMS_POWERUPS_FD,
    ITEMS_POWERUPS_PS, JUNGLE_JAPES, KONGO_JUNGLE, KONGO_JUNGLE_64, MENU_TOUR, METAL_CLOAK_RC,
    MUSHROOM_KINGDOM, MUSHROOM_KINGDOM_2, MUTE_CITY, NAMES, NAMES_KANA, ONETT, OPTIONS_TOUR,
    PC_IZUMI_SIZE, PEACHS_CASTLE, POKE_FLOATS, RAINBOW_CRUISE, RULES, RULES_ITEMS, RULES_PLUS,
    SELECT_CPUS, SELECT_KONGO, SELECT_RANDOM, SELECT_TEAMS, SHEIK_FOX_FOD, SOUND_JP, SOUND_SONGS,
    SOUND_SOUNDS, SPECIAL_INVISIBLE_YS, TROPHY_COLLECTION, TROPHY_COLLECTION_ROWS, TROPHY_GALLERY,
    TROPHY_LOTTERY, VENOM, YOSHIS_ISLAND, YOSHIS_ISLAND_64, compare, compare_in, reflection,
    scaled,
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
frames!(DATA_COUNT;
    data_count_1300 1300,
    data_count_1700 1700,
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
