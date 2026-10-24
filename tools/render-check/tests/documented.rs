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
    BIG_BLUE, BRINSTAR, BRINSTAR_DEPTHS, CORNERIA, Capture, Expect, FLAT_ZONE, FOUR_FD, FOURSIDE,
    FOX_FOX_FD, FOX_MARTH_BF, FOX_MARTH_FD, FOX_MARTH_PS, FOX_SHEIK_DL, FOX_SHEIK_FD, FOX_SHEIK_YS,
    GREAT_BAY, GREEN_GREENS, Game, Group, HYRULE_TEMPLE, ICICLE_MOUNTAIN, IDLE_MOVIE, IDLE_TITLE,
    JUNGLE_JAPES, KONGO_JUNGLE, KONGO_JUNGLE_64, MUSHROOM_KINGDOM, MUSHROOM_KINGDOM_2, MUTE_CITY,
    ONETT, PC_IZUMI_SIZE, PEACHS_CASTLE, POKE_FLOATS, RAINBOW_CRUISE, SHEIK_FOX_FOD, VENOM,
    YOSHIS_ISLAND, YOSHIS_ISLAND_64, compare, compare_in, reflection, scaled,
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
