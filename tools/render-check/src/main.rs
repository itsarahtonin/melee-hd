//! Checks melee-render's frames against ssbm-rs's renderer.
//!
//! ssbm-rs records the GPU command stream of frames of a run as a Dolphin FIFO log
//! (`GX_DFF=FILE GX_DFF_FRAMES=START,COUNT ssbm-run ...`; see docs/player.md). This tool
//! plays such a log through melee-gx, and plays the same replay natively in melee-sim, drawing
//! its display pass through melee-render, then compares the two streams draw by draw (each
//! draw's transformed vertices, raster and TEV state and sampled textures) and the frames as
//! images, within ssbm-rs's render tolerance (PSNR of at least 38 dB, at most 0.25% of the
//! pixels off by more than 32 levels).
//!
//! ```text
//! melee-render-check dump LOG.dff [FRAME]          the log's draws, one line each
//! melee-render-check draw LOG.dff FRAME DRAW       one draw of the log in full
//! melee-render-check play LOG.dff OUT_DIR          the log drawn by melee-gpu
//! melee-render-check align DISC REPLAY.slp LOG.dff FROM TO
//!     which native frame (FROM to TO) the log's first frame is, by its draws' geometry
//! melee-render-check compare DISC REPLAY.slp LOG.dff REF_DIR FRAME [OUT_DIR]
//!     the log's frames (drawn by ssbm-rs, REF_DIR/frame_N.png from gxplay) against the
//!     native frames from FRAME on: draw by draw, and as images within tolerance
//! melee-render-check scaled DISC REPLAY.slp FRAME [OUT_DIR]
//!     native frame FRAME drawn at 2x and 3x, box-downsampled, against the frame at 1x, and
//!     the frame widened to 16:9 against the game's 73:60 over the picture they share
//! melee-render-check snap DISC REPLAY.slp OUT_DIR FRAME...
//!     native frames drawn at 1x, saved as OUT_DIR/frame_N.png
//! melee-render-check dashboard RESULTS_DIR OUT_DIR
//!     a page of the results `compare` and `scaled` recorded with CHECK_RESULTS=RESULTS_DIR:
//!     OUT_DIR/index.html, and the frames compared at full size in OUT_DIR/img/
//! ```

mod native;
mod results;
mod snap;

use std::cell::RefCell;
use std::process::ExitCode;

use melee_gx::dff::File;
use melee_gx::{Draw, Memory, Produced, State};

use snap::{Event, Recorder};

/// Main memory as a FIFO log sets it.
const MEM_SIZE: usize = 0x0180_0000;

/// Plays a FIFO log, recording what each frame draws.
struct LogPlayer {
    ram: RefCell<Vec<u8>>,
    recorder: Recorder,
    /// Draws the log too, with melee-gpu (`play`).
    renderer: Option<RefCell<melee_gpu::Renderer>>,
}

impl Memory for LogPlayer {
    fn read(&self, phys: u32, out: &mut [u8]) {
        let ram = self.ram.borrow();
        let start = (phys & 0x01FF_FFFF) as usize;
        for (i, b) in out.iter_mut().enumerate() {
            *b = ram.get(start + i).copied().unwrap_or(0);
        }
    }

    fn draws(&self) -> bool {
        true
    }

    fn draw(&self, state: &State, draw: &Draw<'_>) {
        self.recorder.draw(state, draw, self);
        if let Some(r) = &self.renderer {
            melee_gx::Sink::draw(&mut *r.borrow_mut(), state, draw, self);
        }
    }

    fn copy(&self, state: &State, value: u32) {
        self.recorder.copy(state, value);
        if let Some(r) = &self.renderer {
            melee_gx::Sink::copy(&mut *r.borrow_mut(), state, value, self);
        }
    }
}

/// Each frame's events of a FIFO log.
fn play_log(path: &str) -> Vec<Vec<Event>> {
    play_log_drawing(path, None).0
}

/// Each frame's events of a FIFO log, and with `out` the frames melee-gpu draws of it, saved
/// there as frame_N.png.
fn play_log_drawing(path: &str, out: Option<&std::path::Path>) -> (Vec<Vec<Event>>, usize) {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let log = File::read(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"));
    let player = LogPlayer {
        ram: RefCell::new(vec![0; MEM_SIZE]),
        recorder: Recorder::default(),
        renderer: out.map(|_| RefCell::new(crate::native::renderer())),
    };
    let mut state = State::load(&log.bp, &log.cp, &log.xf, &log.tmem);
    if let Some(r) = &player.renderer {
        r.borrow_mut().clear_efb(&state);
    }
    let mut saved = 0;
    let mut produced: Vec<Produced> = Vec::new();
    let mut frames = Vec::new();
    for frame in &log.frames {
        let mut at = 0usize;
        let mut updates: Vec<_> = frame.updates.iter().collect();
        updates.sort_by_key(|u| u.position);
        for u in updates {
            let pos = (u.position as usize).min(frame.fifo.len());
            if pos > at {
                state.feed(&player, &frame.fifo[at..pos], &mut produced, false);
                at = pos;
            }
            let mut ram = player.ram.borrow_mut();
            let a = (u.address & 0x01FF_FFFF) as usize;
            let end = (a + u.data.len()).min(MEM_SIZE);
            if a < end {
                ram[a..end].copy_from_slice(&u.data[..end - a]);
            }
        }
        state.feed(&player, &frame.fifo[at..], &mut produced, false);
        produced.clear();
        frames.push(std::mem::take(&mut *player.recorder.events.borrow_mut()));
        if let (Some(r), Some(dir)) = (&player.renderer, out) {
            for f in r.borrow_mut().take_frames() {
                saved += 1;
                let _ = f.save_png(&dir.join(format!("frame_{saved}.png")));
            }
        }
    }
    (frames, saved)
}

fn dump(events: &[Event]) {
    for (i, e) in events.iter().enumerate() {
        match e {
            Event::Draw(d) => println!("{i:5} {}", snap::summary(d)),
            Event::Copy(c) => println!("{i:5} COPY {:08X} {:X?}", c.value, c.regs),
        }
    }
}

fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("dump") if args.len() >= 2 => {
            let frames = play_log(&args[1]);
            let n: usize = args.get(2).map_or(0, |n| n.parse().expect("FRAME"));
            println!("{} frames in the log; frame {n}:", frames.len());
            dump(&frames[n]);
            ExitCode::SUCCESS
        }
        Some("draw") if args.len() >= 4 => {
            let frames = play_log(&args[1]);
            let n: usize = args[2].parse().expect("FRAME");
            let d: usize = args[3].parse().expect("DRAW");
            println!("{:#?}", frames[n][d]);
            ExitCode::SUCCESS
        }
        Some("play") if args.len() >= 3 => {
            let dir = std::path::PathBuf::from(&args[2]);
            std::fs::create_dir_all(&dir).expect("making the output directory");
            let (_, n) = play_log_drawing(&args[1], Some(&dir));
            println!("{n} frames drawn into {}", dir.display());
            ExitCode::SUCCESS
        }
        Some("align") if args.len() >= 6 => {
            let reference = play_log(&args[3]);
            let from: u32 = args[4].parse().expect("FROM");
            let to: u32 = args[5].parse().expect("TO");
            let mut best = (f64::MAX, 0u32, 0i32);
            native::play(&args[1], &args[2], from, to, |n, d| {
                let (dist, matched) = native::distance(&d.events, &reference[0]);
                let score = dist;
                println!(
                    "frame {n} (replay frame {}): {matched} draws matched, {score:.3} apart",
                    d.replay_frame
                );
                if matched > 0 && score < best.0 {
                    best = (score, n, d.replay_frame);
                }
                true
            });
            println!(
                "best: frame {} (replay frame {}), {:.3} apart",
                best.1, best.2, best.0
            );
            ExitCode::SUCCESS
        }
        Some("compare") if args.len() >= 6 => compare(args),
        Some("scaled") if args.len() >= 4 => scaled(args),
        Some("snap") if args.len() >= 5 => {
            let dir = std::path::PathBuf::from(&args[3]);
            std::fs::create_dir_all(&dir).expect("making the output directory");
            let at: Vec<u32> = args[4..]
                .iter()
                .map(|a| a.parse().expect("FRAME"))
                .collect();
            let views = [native::View::REFERENCE];
            for (n, images) in native::draw_views(&args[1], &args[2], &at, &views) {
                let path = dir.join(format!("frame_{n}.png"));
                images[0].save_png(&path).expect("saving a frame");
                println!("{}", path.display());
            }
            ExitCode::SUCCESS
        }
        Some("dashboard") if args.len() >= 3 => {
            let (dir, out) = (
                std::path::Path::new(&args[1]),
                std::path::Path::new(&args[2]),
            );
            if out.extension().is_some_and(|e| e == "html") {
                eprintln!("dashboard takes an output directory: it writes index.html and img/");
                return ExitCode::from(2);
            }
            match results::dashboard(dir, out) {
                Ok(s) => {
                    println!(
                        "{} checks, {} measures: {} with images, {} of them shown{}; {} files, {:.1} MB in {}",
                        s.checks,
                        s.measures,
                        s.recorded,
                        s.shown,
                        if s.shown < s.recorded {
                            " (the rest left out to fit the budget)"
                        } else {
                            ""
                        },
                        s.files,
                        s.bytes as f64 / 1e6,
                        out.display()
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!(
                "usage: melee-render-check dump LOG.dff [FRAME] | draw LOG.dff FRAME DRAW | \
                 align DISC REPLAY LOG FROM TO | compare DISC REPLAY LOG REF_DIR FRAME [OUT] | \
                 scaled DISC REPLAY FRAME [OUT] | snap DISC REPLAY OUT FRAME... | \
                 dashboard RESULTS OUT_DIR"
            );
            ExitCode::from(2)
        }
    }
}

/// An image's RGB pixels.
fn load_png(path: &std::path::Path) -> Option<(u32, u32, Vec<u8>)> {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).ok()?));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let channels = info.color_type.samples();
    let rgb = buf[..info.buffer_size()]
        .chunks_exact(channels)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    Some((info.width, info.height, rgb))
}

/// PSNR and the shares of pixels off by more than 8 and 32 levels in some channel, as
/// ssbm-rs's `gxplay diff` measures them, with an image of where they differ.
pub fn image_diff(a: &[u8], b: &[u8]) -> (f64, f64, f64, Vec<u8>) {
    let (mut se, mut off8, mut off32) = (0f64, 0u64, 0u64);
    let mut img = Vec::with_capacity(a.len() / 3 * 4);
    for (p, q) in a.as_chunks::<3>().0.iter().zip(b.as_chunks::<3>().0) {
        let d: [i32; 3] = std::array::from_fn(|k| i32::from(p[k]) - i32::from(q[k]));
        let m = d.iter().map(|v| v.abs()).max().unwrap_or(0);
        se += d.iter().map(|&v| f64::from(v * v)).sum::<f64>();
        off8 += u64::from(m > 8);
        off32 += u64::from(m > 32);
        let v = (m * 4).min(255) as u8;
        img.extend_from_slice(&[v, v, v, 255]);
    }
    let pixels = (a.len() / 3) as f64;
    let mse = se / (pixels * 3.0);
    let psnr = if mse == 0.0 {
        f64::INFINITY
    } else {
        10.0 * (255.0f64 * 255.0 / mse).log10()
    };
    (
        psnr,
        100.0 * off8 as f64 / pixels,
        100.0 * off32 as f64 / pixels,
        img,
    )
}

/// Whether an image comparison is within ssbm-rs's render tolerance: PSNR of at least 38 dB,
/// at most 0.25% of the pixels off by more than 32 levels.
pub fn within_tolerance(psnr: f64, off32: f64) -> bool {
    psnr >= 38.0 && off32 <= 0.25
}

fn save_rgba(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) {
    let file = std::io::BufWriter::new(std::fs::File::create(path).expect("creating an image"));
    let mut enc = png::Encoder::new(file, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .and_then(|mut w| w.write_image_data(rgba))
        .expect("writing an image");
}

fn compare(args: &[String]) -> ExitCode {
    let reference = play_log(&args[3]);
    let ref_dir = std::path::PathBuf::from(&args[4]);
    let first: u32 = args[5].parse().expect("FRAME");
    let out = args.get(6).map(std::path::PathBuf::from);
    if let Some(o) = &out {
        std::fs::create_dir_all(o).expect("making the output directory");
    }
    let detail = std::env::var("CHECK_DIFFS")
        .ok()
        .map_or(20, |v| v.parse().expect("CHECK_DIFFS=N"));
    // The log's frames line up with native frames near FRAME: the two count frames from
    // different starts. Draw a window around it and take the frame whose draws sit closest
    // to the log's first.
    let search = std::env::var("CHECK_SEARCH")
        .ok()
        .map_or(16, |v| v.parse().expect("CHECK_SEARCH=N"));
    let from = first.saturating_sub(search);
    let last = first + search + reference.len() as u32 - 1;
    let mut drawn: Vec<(u32, native::Drawn)> = Vec::new();
    native::play(&args[1], &args[2], from, last, |n, d| {
        drawn.push((n, d));
        true
    });
    let best = (0..drawn.len())
        .filter(|&i| i + reference.len() <= drawn.len())
        .map(|i| {
            let (dist, matched) = native::distance(&drawn[i].1.events, &reference[0]);
            (i, dist, matched)
        })
        .filter(|x| x.2 > 0)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    let Some((start, apart, _)) = best else {
        println!("no native frame lines up with the log's first");
        return ExitCode::FAILURE;
    };
    println!(
        "the log's first frame is native frame {} ({apart:.3} apart)",
        drawn[start].0
    );
    let results = results::Results::from_env();
    let mut failed = false;
    for (k, (n, d)) in drawn
        .into_iter()
        .skip(start)
        .take(reference.len())
        .enumerate()
    {
        let theirs = &reference[k];
        println!(
            "== frame {n} (replay frame {}) against the log's frame {k}",
            d.replay_frame
        );
        for (what, count) in &d.notes {
            println!("  not drawn exactly: {what} ({count})");
        }
        println!("  {} events, the log's {}", d.events.len(), theirs.len());
        let (mut shown, mut same) = (0, 0);
        let pairs = snap::align(&d.events, theirs);
        let mut missing = Vec::new();
        for (i, j) in pairs {
            let (a, b) = match (i, j) {
                (Some(i), Some(j)) => (&d.events[i], &theirs[j]),
                (Some(i), None) => {
                    missing.push(format!(
                        "  extra {i}: {}",
                        snap::event_summary(&d.events[i])
                    ));
                    continue;
                }
                (None, Some(j)) => {
                    missing.push(format!(
                        "  missing (log {j}): {}",
                        snap::event_summary(&theirs[j])
                    ));
                    continue;
                }
                (None, None) => continue,
            };
            let i = i.unwrap_or(0);
            let diffs = match (a, b) {
                (Event::Draw(a), Event::Draw(b)) => snap::differences(a, b),
                (Event::Copy(a), Event::Copy(b)) if snap::same_copy(a, b) => Vec::new(),
                (Event::Copy(a), Event::Copy(b)) => vec![format!(
                    "copy {:08X} {:X?} vs {:08X} {:X?}",
                    a.value, a.regs, b.value, b.regs
                )],
                _ => vec!["a draw against a copy".to_owned()],
            };
            if diffs.is_empty() {
                same += 1;
            } else if shown < detail {
                shown += 1;
                match (a, b) {
                    (Event::Draw(a), Event::Draw(b)) => {
                        println!("  draw {i}: {}", snap::summary(a));
                        println!("     log: {}", snap::summary(b));
                    }
                    _ => println!("  event {i}:"),
                }
                for x in diffs {
                    println!("     {x}");
                }
            }
        }
        if let Ok(k) = std::env::var("CHECK_EVENT") {
            let k: usize = k.parse().expect("CHECK_EVENT=N");
            println!("  ours {k}: {:#?}", d.events.get(k));
        }
        println!("  {} events unmatched", missing.len());
        for m in missing.iter().take(detail) {
            println!("{m}");
        }
        println!("  {same} events the same");
        match &d.image {
            Some(img) => {
                let reference_png = ref_dir.join(format!("frame_{}.png", k + 1));
                let rgb: Vec<u8> = img
                    .rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .flat_map(|p| [p[0], p[1], p[2]])
                    .collect();
                if let Some(o) = &out {
                    let _ = img.save_png(&o.join(format!("frame_{n}.png")));
                }
                match load_png(&reference_png) {
                    Some((w, h, theirs)) if (w, h) == (img.width, img.height) => {
                        let (psnr, off8, off32, diff) = image_diff(&rgb, &theirs);
                        let ok = within_tolerance(psnr, off32);
                        failed |= !ok;
                        println!(
                            "  image: PSNR {psnr:.2} dB, {off8:.3}% off by 8, {off32:.3}% off by 32: {}",
                            if ok {
                                "within tolerance"
                            } else {
                                "OUT OF TOLERANCE"
                            }
                        );
                        if let Some(o) = &out {
                            save_rgba(&o.join(format!("diff_{n}.png")), w, h, &diff);
                        }
                        if let Some(r) = &results {
                            let what = k.to_string();
                            let m = results::Measure {
                                kind: "compare",
                                what: &what,
                                native: n,
                                psnr,
                                off8,
                                off32,
                                block: f64::NAN,
                                within: ok,
                            };
                            let img = |pixels| results::Rgb {
                                width: w,
                                height: h,
                                pixels,
                            };
                            r.record(&m, img(&rgb), img(&theirs), Some(&diff));
                        }
                    }
                    Some((w, h, _)) => {
                        failed = true;
                        println!(
                            "  image: {}x{} against the reference's {w}x{h}",
                            img.width, img.height
                        );
                    }
                    None => {
                        failed = true;
                        println!("  no reference image {}", reference_png.display());
                    }
                }
            }
            None => {
                println!("  no frame was copied to the XFB");
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// An RGB image `fx` by `fy` times smaller, each pixel its block's average (rounded).
pub fn downsample(w: u32, h: u32, rgb: &[u8], fx: u32, fy: u32) -> Vec<u8> {
    let (ow, oh) = (w / fx, h / fy);
    let n = fx * fy;
    let mut out = Vec::with_capacity((ow * oh * 3) as usize);
    for y in 0..oh {
        for x in 0..ow {
            let mut sum = [0u32; 3];
            for dy in 0..fy {
                let row = ((y * fy + dy) * w + x * fx) as usize * 3;
                for p in rgb[row..row + fx as usize * 3].as_chunks::<3>().0 {
                    for k in 0..3 {
                        sum[k] += u32::from(p[k]);
                    }
                }
            }
            out.extend(sum.map(|v| ((v + n / 2) / n) as u8));
        }
    }
    out
}

/// The side of the scaled checks' blocks in a frame `h` pixels high: 16 of the console's
/// pixels.
pub fn block_side(h: u32) -> u32 {
    16 * h / 480
}

/// Each block's mean difference of two RGB images `w` wide, in blocks `side` pixels square
/// (the whole blocks, `w / side` a row, row by row): the largest of its channels' mean
/// differences (ours less theirs, as a magnitude), in levels. Edges drawn at another
/// resolution and the dither pattern differ pixel by pixel both ways and average out over a
/// block; a region drawn wrong doesn't.
pub fn block_means(a: &[u8], b: &[u8], w: u32, side: u32) -> Vec<f64> {
    let h = (a.len() / 3) as u32 / w;
    let px = f64::from(side * side);
    let mut out = Vec::with_capacity(((w / side) * (h / side)) as usize);
    for by in (0..h / side).map(|y| y * side) {
        for bx in (0..w / side).map(|x| x * side) {
            let mut sum = [0i64; 3];
            for y in by..by + side {
                let row = ((y * w + bx) * 3) as usize;
                let n = (side * 3) as usize;
                for (i, (p, q)) in a[row..row + n].iter().zip(&b[row..row + n]).enumerate() {
                    sum[i % 3] += i64::from(*p) - i64::from(*q);
                }
            }
            out.push(
                sum.iter()
                    .map(|&s| (s as f64 / px).abs())
                    .fold(0f64, f64::max),
            );
        }
    }
    out
}

/// The largest of [`block_means`], and the block's top left corner (the first, if several).
fn worst_block(a: &[u8], b: &[u8], w: u32, side: u32) -> (f64, u32, u32) {
    let cols = w / side;
    let mut worst = (0f64, 0, 0);
    for (i, &mean) in (0u32..).zip(&block_means(a, b, w, side)) {
        if mean > worst.0 {
            worst = (mean, i % cols * side, i / cols * side);
        }
    }
    worst
}

/// Columns `x0..x0 + width` of an RGB image `w` wide.
fn columns(w: u32, rgb: &[u8], x0: u32, width: u32) -> Vec<u8> {
    rgb.chunks_exact(w as usize * 3)
        .flat_map(|row| &row[x0 as usize * 3..(x0 + width) as usize * 3])
        .copied()
        .collect()
}

fn rgb_of(f: &melee_render::gpu::Frame) -> Vec<u8> {
    f.rgba
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect()
}

/// The scaled checks' tolerance: a frame drawn at a higher scale or wider, brought back to the
/// 1x frame's pixels (box-downsampled, or cropped to the picture the two share), against the
/// frame drawn that way at 1x. Edges and texture sampling legitimately differ there (a 3x
/// edge pixel averages nine samples where 1x takes one; copies to textures average blocks
/// back), so this is looser than ssbm-rs's 38 dB; docs/player.md has the measurements.
pub const SCALED_PSNR: f64 = 24.0;
/// The largest mean difference of a block of 16 by 16 of the console's pixels, in levels.
pub const SCALED_BLOCK: f64 = 32.0;

fn within_scaled(psnr: f64, block: f64) -> bool {
    psnr >= SCALED_PSNR && block <= SCALED_BLOCK
}

fn scaled(args: &[String]) -> ExitCode {
    use native::View;
    let at: u32 = args[3].parse().expect("FRAME");
    let out = args.get(4).map(std::path::PathBuf::from);
    if let Some(o) = &out {
        std::fs::create_dir_all(o).expect("making the output directory");
    }
    let views = [
        View::REFERENCE,
        View::console(1),
        View::console(2),
        View::console(3),
        View::shown(3, melee_render::frame::GAME_ASPECT),
        View::shown(3, 16.0 / 9.0),
    ];
    let mut drawn = native::draw_views(&args[1], &args[2], &[at], &views);
    let (n, images) = drawn.pop().expect("the frame drawn");
    let results = results::Results::from_env();
    let mut failed = false;
    let check = |name: &str, ours: Vec<u8>, theirs: Vec<u8>, w: u32, h: u32| -> bool {
        let (psnr, off8, off32, diff) = image_diff(&ours, &theirs);
        let side = block_side(h);
        let (block, bx, by) = worst_block(&ours, &theirs, w, side);
        let ok = within_scaled(psnr, block);
        println!(
            "  scaled {name}: PSNR {psnr:.2} dB, {off8:.3}% off by 8, {off32:.3}% off by 32, worst block {block:.2} levels at ({}, {}): {}",
            bx / (side / 16),
            by / (side / 16),
            if ok {
                "within tolerance"
            } else {
                "OUT OF TOLERANCE"
            }
        );
        if let Some(o) = &out {
            let tag = name.replace([' ', ':'], "_");
            let rgba = |rgb: &[u8]| -> Vec<u8> {
                rgb.as_chunks::<3>()
                    .0
                    .iter()
                    .flat_map(|p| [p[0], p[1], p[2], 255])
                    .collect()
            };
            save_rgba(&o.join(format!("{tag}_ours.png")), w, h, &rgba(&ours));
            save_rgba(&o.join(format!("{tag}_theirs.png")), w, h, &rgba(&theirs));
            save_rgba(&o.join(format!("{tag}_diff.png")), w, h, &diff);
        }
        if let Some(r) = &results {
            let m = results::Measure {
                kind: "scaled",
                what: name,
                native: n,
                psnr,
                off8,
                off32,
                block,
                within: ok,
            };
            let img = |pixels| results::Rgb {
                width: w,
                height: h,
                pixels,
            };
            // The dashboard shows the blocks this measures, not the pixels.
            r.record(&m, img(&ours), img(&theirs), None);
        }
        ok
    };
    println!("== frame {n}");
    for (v, f) in views.iter().zip(&images) {
        println!(
            "  drawn at {}x, {} wide: {}x{}",
            v.scale, v.width, f.width, f.height
        );
    }
    // The player's copy to the XFB (on the GPU) against the one compared with ssbm-rs's frames.
    let (reference, one) = (&images[0], &images[1]);
    if (one.width, one.height) == (reference.width, reference.height) {
        failed |= !check(
            "1x GPU XFB",
            rgb_of(one),
            rgb_of(reference),
            one.width,
            one.height,
        );
    } else {
        println!(
            "  1x: {}x{} against the reference's {}x{}",
            one.width, one.height, reference.width, reference.height
        );
        failed = true;
    }
    for (k, f) in images.iter().enumerate().take(4).skip(2) {
        let s = views[k].scale;
        if (f.width, f.height) != (one.width * s, one.height * s) {
            println!(
                "  {s}x: {}x{}, not {s} times the 1x frame's {}x{}",
                f.width, f.height, one.width, one.height
            );
            failed = true;
            continue;
        }
        let small = downsample(f.width, f.height, &rgb_of(f), s, s);
        failed |= !check(&format!("{s}x"), small, rgb_of(one), one.width, one.height);
    }
    // 16:9 widens the cameras about the middle, so its middle columns are the 73:60 frame.
    let (g, w) = (&images[4], &images[5]);
    if w.height == g.height && w.width > g.width && (w.width - g.width).is_multiple_of(2) {
        let x0 = (w.width - g.width) / 2;
        let middle = columns(w.width, &rgb_of(w), x0, g.width);
        failed |= !check("3x 16:9", middle, rgb_of(g), g.width, g.height);
    } else {
        println!(
            "  16:9: {}x{} against 73:60's {}x{}",
            w.width, w.height, g.width, g.height
        );
        failed = true;
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(move || run(&args))
        .unwrap()
        .join()
        .unwrap()
}
