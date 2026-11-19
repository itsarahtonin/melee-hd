//! The fast renderer's comparison harness (docs/player.md, "The fast renderer"): a frame drawn
//! two ways from the same state, by a renderer under test and by the exact renderer, and
//! compared: each draw's transformed positions within a tolerance ([`geometry`]), and the image
//! by a perceptual measure ([`image`]). The exact renderer is the reference, as it is checked
//! draw for draw against ssbm-rs's; the fast renderer may differ from it in vertex float maths,
//! the rasterizer's snap and depth precision, not in what's drawn, the order, materials,
//! blending or effects.
//!
//! A renderer takes what the display pass hands on, [`Tick`] (the game's state after its
//! display pass and the pass's draws), so it works whether the fast renderer's records are
//! built on the presentation side from them or come from melee-sim itself (refactor design
//! call 6): either way they come from the tick. Until the fast renderer exists, the renderer
//! under test is the exact renderer with a change made on purpose ([`exact::Mutation`]).
//!
//! ```text
//! melee-render-check fast DISC REPLAY [--renderer exact|f2|fast] [--mutate NAME] [--scale N]
//!     [--threaded] [--out DIR] FRAME...
//! melee-render-check bench DISC REPLAY [--renderer exact|f2|fast] [--scale N] [--from A] [--to B]
//!     [--one-thread] [--adapter NAME|fallback]
//! ```

pub mod bench;
pub mod exact;
pub mod geometry;
pub mod image;

use std::process::ExitCode;
use std::time::Duration;

use melee_gpu::{Frame, Renderer};
use melee_sim::game::state::Cosmetic;
use melee_sim::hsd::display::Draw as DisplayDraw;

use crate::native::{self, Tick, View};
use exact::{Exact, Mutated, Mutation};
use geometry::Geometry;

/// What a renderer made of a frame.
#[derive(Default)]
pub struct Rendered {
    /// The frame copied to the XFB, if it drew one.
    pub image: Option<Frame>,
    /// Each draw's transformed vertices, when asked for.
    pub geometry: Option<Geometry>,
    /// The CPU's time building what it draws (the exact renderer's GX stream), and submitting
    /// it (the exact renderer's stream run and drawn, the GPU's work submitted).
    pub build: Duration,
    pub submit: Duration,
}

/// A renderer the harness compares or measures.
pub trait FrameRenderer {
    fn name(&self) -> String;

    /// Builds the tick's frame and, with `draw`, draws it; without, only follows it (the
    /// frames after may depend on what it sets). With `geometry`, reports each draw's
    /// transformed vertices.
    fn frame(&mut self, t: &Tick<'_>, draw: bool, geometry: bool) -> Rendered;

    /// Waits until the GPU has done everything submitted.
    fn wait_idle(&self);

    /// Sees the reference's geometry of the frame it draws next: a stand-in with a change made
    /// on purpose picks its target there.
    fn aim(&mut self, _reference: &Geometry, _draws: &[DisplayDraw]) {}

    /// Ends a warm-up: waits until every specialized pipeline asked for is made.
    fn warm(&mut self) {}
}

/// Which GPU adapter to draw with.
#[derive(Clone, Debug)]
pub enum Adapter {
    /// The high-performance adapter, as the player picks.
    Default,
    /// The system's software rasterizer (WARP on Direct3D 12, llvmpipe on Vulkan): no GPU.
    Fallback,
    /// The first adapter whose name contains this, ignoring case.
    Named(String),
}

impl Adapter {
    pub fn parse(s: &str) -> Adapter {
        match s {
            "default" => Adapter::Default,
            "fallback" => Adapter::Fallback,
            name => Adapter::Named(name.to_lowercase()),
        }
    }
}

/// A device on `adapter`, as melee-gpu's `Renderer::new` makes one, and the adapter's
/// description.
pub fn device(adapter: &Adapter) -> (wgpu::Device, wgpu::Queue, String) {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let a = match adapter {
        Adapter::Named(name) => {
            pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()))
                .into_iter()
                .find(|a| a.get_info().name.to_lowercase().contains(name.as_str()))
                .unwrap_or_else(|| panic!("no GPU adapter named like {name}"))
        }
        _ => pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: matches!(adapter, Adapter::Fallback),
            ..Default::default()
        }))
        .unwrap_or_else(|e| panic!("no GPU adapter: {e}")),
    };
    let info = a.get_info();
    let (device, queue) = pollster::block_on(a.request_device(&wgpu::DeviceDescriptor {
        label: Some("melee-gpu"),
        required_features: wgpu::Features::DUAL_SOURCE_BLENDING
            | (a.features() & wgpu::Features::SHADER_EARLY_DEPTH_TEST),
        required_limits: wgpu::Limits::default(),
        ..Default::default()
    }))
    .unwrap_or_else(|e| panic!("no GPU device: {e}"));
    let text = format!("{} ({:?}, {})", info.name, info.backend, info.driver);
    (device, queue, text)
}

/// A renderer on `device` for `view`.
pub fn renderer(device: &wgpu::Device, queue: &wgpu::Queue, view: View) -> Renderer {
    let mut r = Renderer::with_device_efb(device.clone(), queue.clone(), view.scale, view.width, 1);
    (r.xfb_on_gpu, r.xfb_readback) = (view.gpu_xfb, view.gpu_xfb);
    r
}

fn rgb(f: &Frame) -> Vec<u8> {
    f.rgba
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect()
}

fn save(path: &std::path::Path, w: u32, h: u32, rgb: &[u8]) {
    let file = std::io::BufWriter::new(std::fs::File::create(path).expect("an image file"));
    let mut e = png::Encoder::new(file, w, h);
    e.set_color(png::ColorType::Rgb);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .and_then(|mut w| w.write_image_data(rgb))
        .expect("writing an image");
}

/// One frame's comparison.
pub struct FrameReport {
    pub geometry: geometry::GeometryReport,
    pub image: image::ImageReport,
}

impl FrameReport {
    pub fn within(&self) -> bool {
        self.geometry.within() && self.image.within()
    }
}

/// Compares a frame drawn by `test` with the reference's; `spreads` are the reference's frame
/// shifted by sub-pixel amounts (`exact::Mutation::Spread`).
pub fn compare(
    reference: &Rendered,
    test: &Rendered,
    spreads: &[Rendered],
    scale: u32,
    out: Option<(&std::path::Path, u32)>,
) -> FrameReport {
    let (gr, gt) = (
        reference
            .geometry
            .as_ref()
            .expect("the reference's geometry"),
        test.geometry.as_ref().expect("the geometry"),
    );
    let geometry = geometry::compare(gr, gt);
    let (a, b) = (
        reference.image.as_ref().expect("the reference's frame"),
        test.image.as_ref().expect("the frame"),
    );
    let (ra, rb) = (rgb(a), rgb(b));
    let spreads: Vec<Vec<u8>> = spreads
        .iter()
        .filter_map(|s| s.image.as_ref())
        .filter(|s| (s.width, s.height) == (a.width, a.height))
        .map(rgb)
        .collect();
    let spreads: Vec<&[u8]> = spreads.iter().map(Vec::as_slice).collect();
    let image = if (a.width, a.height) == (b.width, b.height) {
        image::compare(&ra, &rb, &spreads, a.width, a.height, scale)
    } else {
        image::ImageReport::sizes_differ(b.width, b.height)
    };
    if let Some((dir, n)) = out {
        save(
            &dir.join(format!("frame_{n}_reference.png")),
            a.width,
            a.height,
            &ra,
        );
        save(
            &dir.join(format!("frame_{n}_test.png")),
            b.width,
            b.height,
            &rb,
        );
        if (a.width, a.height) == (b.width, b.height) {
            let heat = image::heatmap(&ra, &rb, &spreads, a.width, a.height, scale);
            save(
                &dir.join(format!("frame_{n}_heat.png")),
                a.width,
                a.height,
                &heat,
            );
        }
    }
    FrameReport { geometry, image }
}

/// The renderers a comparison takes: the reference, the reference shifted by sub-pixel amounts
/// for each block's spread, and the renderer under test.
pub struct Renderers<'a> {
    pub reference: &'a mut dyn FrameRenderer,
    pub spreads: Vec<&'a mut dyn FrameRenderer>,
    pub test: &'a mut dyn FrameRenderer,
}

/// Draws the replay's native frames `at` with the reference, its spreads and the renderer
/// under test, each with the frames before whose copies it may sample, and compares them;
/// calls `each` with each frame's number, its replay frame, the comparison and the display
/// draws.
pub fn run(
    disc: &str,
    replay: &str,
    at: &[u32],
    scale: u32,
    r: Renderers<'_>,
    out: Option<&std::path::Path>,
    mut each: impl FnMut(u32, i32, &FrameReport, &[DisplayDraw]),
) {
    let Renderers {
        reference,
        mut spreads,
        test,
    } = r;
    let last = at.iter().copied().max().unwrap_or(0);
    let mut seen = 0;
    native::drive(disc, replay, last, Cosmetic::default(), |t| {
        let draw = at
            .iter()
            .any(|&a| native::drawn_for(t.n, a, native::WARM_UP));
        let wanted = at.contains(&t.n);
        let r = reference.frame(t, draw, wanted);
        let shifted: Vec<Rendered> = spreads
            .iter_mut()
            .map(|s| s.frame(t, draw, false))
            .collect();
        if wanted {
            test.aim(r.geometry.as_ref().expect("geometry"), t.draws);
        }
        let s = test.frame(t, draw, wanted);
        if wanted {
            let report = compare(&r, &s, &shifted, scale, out.map(|d| (d, t.n)));
            each(t.n, t.replay_frame, &report, t.draws);
            seen += 1;
        }
        t.n < last
    });
    assert_eq!(seen, at.len(), "{replay}: the match ends before its frames");
}

/// The renderers `--renderer` names: the fast renderer as far as it goes, or the exact
/// renderer (with `--mutate`'s change, for `fast`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Exact,
    /// F2: the mesh cache, its vertices transformed on the CPU.
    F2,
    /// F3: F2's meshes transformed on the GPU, each draw's state built from the registers.
    F3,
    /// The fast renderer as far as it goes: F4, F3 with materials as records.
    Fast,
}

impl Which {
    pub fn parse(s: &str) -> Which {
        match s {
            "exact" => Which::Exact,
            "f2" => Which::F2,
            "f3" => Which::F3,
            "fast" | "f4" => Which::Fast,
            _ => panic!("--renderer {s}: exact, f2, f3 or fast (f4)"),
        }
    }
}

/// `melee-render-check fast DISC REPLAY [--renderer exact|f2|fast] [--mutate NAME] [--scale N]
/// [--threaded] [--out DIR] FRAME...`
pub fn cli(args: &[String]) -> ExitCode {
    let (disc, replay) = (&args[1], &args[2]);
    let mut mutation = Mutation::None;
    let mut which = Which::Exact;
    let mut scale = 1;
    let mut threaded = false;
    let mut out = None;
    let mut at = Vec::new();
    let mut rest = args[3..].iter();
    while let Some(a) = rest.next() {
        match a.as_str() {
            "--renderer" => which = Which::parse(rest.next().expect("--renderer NAME")),
            "--mutate" => {
                let m = rest.next().expect("--mutate NAME");
                mutation = Mutation::parse(m).unwrap_or_else(|| {
                    let names: Vec<&str> = Mutation::ALL.iter().map(|x| x.0).collect();
                    panic!("--mutate {m}: one of {}", names.join(", "))
                });
            }
            "--scale" => scale = rest.next().and_then(|s| s.parse().ok()).expect("--scale N"),
            "--threaded" => threaded = true,
            "--out" => {
                let d = std::path::PathBuf::from(rest.next().expect("--out DIR"));
                std::fs::create_dir_all(&d).expect("making the output directory");
                out = Some(d);
            }
            n => at.push(n.parse::<u32>().expect("FRAME")),
        }
    }
    let view = if scale == 1 {
        View::REFERENCE
    } else {
        View::console(scale)
    };
    let (device, queue, adapter) = device(&Adapter::Default);
    let mut reference = Exact::new(renderer(&device, &queue, view), false);
    let mut right = Mutated::new(renderer(&device, &queue, view), Mutation::Spread(true));
    let mut left = Mutated::new(renderer(&device, &queue, view), Mutation::Spread(false));
    let mut test: Box<dyn FrameRenderer> = match which {
        Which::Fast | Which::F3 | Which::F2 => {
            assert_eq!(
                mutation,
                Mutation::None,
                "--mutate changes the exact renderer"
            );
            let mut r = renderer(&device, &queue, view);
            Box::new(match which {
                // F4 draws as it does after its warm-up: every draw with its specialized
                // shader, made as the draw first needs it.
                Which::Fast => {
                    r.set_tev_mode(melee_gpu::TevMode::Sync);
                    Exact::fast(r, threaded)
                }
                Which::F3 => Exact::f3(r, threaded),
                _ => Exact::f2(r, threaded),
            })
        }
        Which::Exact => Box::new(Mutated::new(renderer(&device, &queue, view), mutation)),
    };
    println!(
        "{replay}: {} against {} at {scale}x on {adapter}",
        test.name(),
        reference.name()
    );
    let mut failed = 0;
    run(
        disc,
        replay,
        &at,
        scale,
        Renderers {
            reference: &mut reference,
            spreads: vec![&mut right, &mut left],
            test: &mut *test,
        },
        out.as_deref(),
        |n, replay_frame, report, draws| {
            let ok = report.within();
            failed += usize::from(!ok);
            println!(
                "== frame {n} (replay frame {replay_frame}): {}",
                if ok { "PASS" } else { "FAIL" }
            );
            for l in report.geometry.lines(&|s| exact::describe(draws, s)) {
                println!("  {l}");
            }
            println!("  {}", report.image.line());
        },
    );
    println!(
        "{} of {} frames within tolerance",
        at.len() - failed,
        at.len()
    );
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
