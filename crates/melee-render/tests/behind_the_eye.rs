//! A frame with a vertex just behind the eye draws whole: ganon-captain-fd's tick 6279, where
//! a vertex of Final Destination's background lies 0.0004 behind the camera's plane and
//! hundreds of millions of pixels off screen. melee-gpu snapped its screen position in
//! integers, which saturated and moved it in clip space, and its triangle then covered the
//! screen nearer than the whole scene: the stage, the fighters and the timer vanished for the
//! frame, leaving the background color and the HUD that doesn't test depth.
//!
//! Plays the benchmark replay natively and draws its frames from tick 6271 with melee-gpu at
//! 1x, then compares how bright tick 6279's frame is with tick 6278's (the mean of its RGB):
//! the dropout's was 12.6 against 47.4, the whole frame's is 46.7. Skips without `SSBM_DISC`
//! or a hardware GPU.

use std::collections::HashMap;

use melee_data::disc::Disc;
use melee_render::frame::Frames;
use melee_render::gpu::Gpu;
use melee_sim::game::vs_match::{DiscFile, Match, Settings};
use melee_slippi::{Device, Replay};

const REPLAY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/bench/ganon-captain-fd.slp"
);

/// The tick (the replay's frames played, from 1) whose frame dropped out.
const TICK: u32 = 6279;

/// Frames drawn before it, whose copies (the shadows) it samples.
const WARM_UP: u32 = 8;

#[test]
fn tick_6279_of_ganon_captain_fd_draws_whole() {
    let Some(path) = std::env::var_os("SSBM_DISC") else {
        eprintln!("skipped: SSBM_DISC names no disc");
        return;
    };
    // Not without a GPU, or on a software adapter, which draws gx.wgsl wrong.
    let Some(renderer) = melee_gpu::testing::renderer() else {
        return;
    };
    let disc = Disc::open(&path).expect("the disc");
    melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
    let bytes = std::fs::read(REPLAY).expect("the benchmark replay");
    let replay = Replay::parse(&bytes).expect("a replay");
    let settings = Settings::from_replay(&melee_slippi::start(&replay)).expect("its settings");
    let mut cache: HashMap<String, (i32, Vec<u8>)> = HashMap::new();
    let mut load = |name: &str| {
        let (entrynum, bytes) = cache.entry(name.to_owned()).or_insert_with(|| {
            (
                disc.entrynum(name).expect("a file"),
                disc.read_file(name).expect("a file"),
            )
        });
        DiscFile {
            entrynum: *entrynum,
            bytes: bytes.clone(),
        }
    };
    let mut m = Match::start(&settings, &mut load);
    let mut frames = Frames::new();
    frames.space.load_dol(&disc.dol().expect("main.dol"));
    let mut gpu = Gpu::new(renderer);
    let mut images = Vec::new();
    for tick in 1..=TICK {
        let f = m.step_replay(&mut Device(&replay));
        assert!(!f.ended, "the match ended before tick {TICK}");
        assert!(
            tick < TICK - 1 || !f.draws.is_empty(),
            "tick {tick} draws nothing"
        );
        if !f.draws.is_empty() {
            let fifo = frames.frame(&m.game, &m.data, &f.draws);
            if tick + WARM_UP >= TICK {
                let mut drawn = gpu.run(&frames.space, &m.data, &fifo, None);
                images.push(drawn.pop().expect("the frame"));
            } else {
                gpu.follow(&frames.space, &m.data, &fifo);
            }
        }
        m.recycle(f);
    }
    let [.., before, frame] = &images[..] else {
        panic!("no frames drawn");
    };
    let brightness = |rgba: &[u8]| {
        let sum: u64 = rgba
            .chunks(4)
            .map(|p| p[..3].iter().map(|&c| u64::from(c)).sum::<u64>())
            .sum();
        sum as f64 / (rgba.len() / 4 * 3) as f64
    };
    let (was, is) = (brightness(&before.rgba), brightness(&frame.rgba));
    eprintln!("tick {TICK}'s frame: brightness {is:.2}, the tick before's {was:.2}");
    assert!(
        (is - was).abs() < 5.0,
        "tick {TICK}'s frame is {is:.2} bright on average, the tick before's {was:.2}: the world dropped out"
    );
}
