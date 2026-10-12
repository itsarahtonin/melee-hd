//! Checks melee-render's frames against ssbm-rs's renderer.
//!
//! ssbm-rs records the GPU command stream of frames of a run as a Dolphin FIFO log
//! (`GX_DFF=FILE GX_DFF_FRAMES=START,COUNT ssbm-run ...`; see docs/player.md). This tool
//! plays such a log through ssbm-gx, and plays the same replay natively in melee-sim, drawing
//! its display pass through melee-render, then compares the two streams draw by draw (each
//! draw's transformed vertices, raster and TEV state and sampled textures) and the frames as
//! images, within ssbm-rs's render tolerance (PSNR of at least 38 dB, at most 0.25% of the
//! pixels off by more than 32 levels).
//!
//! ```text
//! melee-render-check dump LOG.dff [FRAME]          the log's draws, one line each
//! melee-render-check draw LOG.dff FRAME DRAW       one draw of the log in full
//! melee-render-check align DISC REPLAY.slp LOG.dff FROM TO
//!     which native frame (FROM to TO) the log's first frame is, by its draws' geometry
//! melee-render-check compare DISC REPLAY.slp LOG.dff REF_DIR FRAME [OUT_DIR]
//!     the log's frames (drawn by ssbm-rs, REF_DIR/frame_N.png from gxplay) against the
//!     native frames from FRAME on: draw by draw, and as images within tolerance
//! ```

mod native;
mod snap;

use std::cell::RefCell;
use std::process::ExitCode;

use ssbm_gx::dff::File;
use ssbm_gx::{Draw, Memory, Produced, State};

use snap::{Event, Recorder};

/// Main memory as a FIFO log sets it.
const MEM_SIZE: usize = 0x0180_0000;

/// Plays a FIFO log, recording what each frame draws.
struct LogPlayer {
    ram: RefCell<Vec<u8>>,
    recorder: Recorder,
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
    }

    fn copy(&self, state: &State, value: u32) {
        self.recorder.copy(state, value);
    }
}

/// Each frame's events of a FIFO log.
fn play_log(path: &str) -> Vec<Vec<Event>> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let log = File::read(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"));
    let player = LogPlayer {
        ram: RefCell::new(vec![0; MEM_SIZE]),
        recorder: Recorder::default(),
    };
    let mut state = State::load(&log.bp, &log.cp, &log.xf, &log.tmem);
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
    }
    frames
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
        _ => {
            eprintln!(
                "usage: melee-render-check dump LOG.dff [FRAME] | draw LOG.dff FRAME DRAW | \
                 align DISC REPLAY LOG FROM TO | compare DISC REPLAY LOG REF_DIR FRAME [OUT]"
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
                    }
                    Some((w, h, _)) => println!(
                        "  image: {}x{} against the reference's {w}x{h}",
                        img.width, img.height
                    ),
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

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(move || run(&args))
        .unwrap()
        .join()
        .unwrap()
}
