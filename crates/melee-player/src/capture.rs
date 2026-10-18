//! F12's bug capture: a folder beside the executable (`captures/<time>`) with the frame as
//! shown (`shown.png`), the same draws drawn again by a new frame builder and renderer, with
//! the same blend (`fresh.png`) and with none (`fresh-unblended.png`), and what drew them
//! (`info.txt`). A glitch the fresh frame doesn't have lives in what the player keeps between
//! frames (the frame builder's caches, the GPU's state, the renderer's textures); one it has
//! too lives in the draws, the state they read, or the blend. Two more split the first case:
//! `live-stream.png`, the stream the player drew run by a new renderer from the GPU's registers
//! as the player's frame found them (broken: the registers or the stream; whole: the
//! renderer's own textures, depth or caches), and `registers.txt`, every register whose value
//! after the frame differs from a new frame builder's and renderer's after the same draws.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use melee_render::blend::Blend;
use melee_render::frame::Frames;
use melee_render::gpu::Gpu;
use melee_sim::game::vs_match::Match;
use melee_sim::hsd::display::Draw;

use crate::{pipelines, present};

/// The GPU's registers, as a frame found or left them.
pub struct Regs {
    bp: [u32; 256],
    cp: [u32; 256],
    xf: Vec<u32>,
    colors: [[u32; 4]; 4],
    tmem: Vec<u8>,
}

impl Regs {
    pub fn of(s: &melee_gx::State) -> Regs {
        Regs {
            bp: s.bp,
            cp: s.cp,
            xf: s.xf.clone(),
            colors: [s.color_ra, s.color_bg, s.konst_ra, s.konst_bg],
            tmem: s.tmem.clone(),
        }
    }

    fn set(&self, s: &mut melee_gx::State) {
        s.bp = self.bp;
        s.cp = self.cp;
        s.xf.clone_from(&self.xf);
        [s.color_ra, s.color_bg, s.konst_ra, s.konst_bg] = self.colors;
        s.tmem.clone_from(&self.tmem);
    }

    /// Each register that differs from `other`'s, one a line.
    fn diff(&self, other: &Regs, out: &mut String) {
        let mut n = 0;
        let mut line = |space: &str, i: usize, a: u32, b: u32| {
            n += 1;
            let _ = writeln!(out, "{space} {i:#05x}: live {a:#010x} fresh {b:#010x}");
        };
        for (i, (a, b)) in self.bp.iter().zip(&other.bp).enumerate() {
            if a != b {
                line("bp", i, *a, *b);
            }
        }
        for (i, (a, b)) in self.cp.iter().zip(&other.cp).enumerate() {
            if a != b {
                line("cp", i, *a, *b);
            }
        }
        for (i, (a, b)) in self.xf.iter().zip(&other.xf).enumerate() {
            if a != b {
                line("xf", i, *a, *b);
            }
        }
        for (k, (a, b)) in self.colors.iter().zip(&other.colors).enumerate() {
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                if x != y {
                    line(
                        ["color_ra", "color_bg", "konst_ra", "konst_bg"][k],
                        i,
                        *x,
                        *y,
                    );
                }
            }
        }
        let tmem = self
            .tmem
            .iter()
            .zip(&other.tmem)
            .filter(|(a, b)| a != b)
            .count();
        let _ = writeln!(out, "{n} registers differ; {tmem} bytes of texture memory");
    }
}

/// The player's frame as the GPU took it: its registers before and after, and its stream.
pub struct Live<'a> {
    pub before: Regs,
    pub after: Regs,
    pub fifo: &'a [u8],
    /// The address space the stream's addresses are in.
    pub space: &'a melee_render::mem::Space,
    /// The frame builder's HSD state cache after the frame, as text.
    pub hsd: String,
}

/// What a capture records besides the frames.
pub struct About<'a> {
    pub mode: &'a str,
    pub tick: u64,
    pub scale: u32,
    pub msaa: u32,
    pub aspect: present::Aspect,
    pub widescreen: Option<f32>,
    pub notes: String,
}

/// Saves a capture of the frame just drawn: `texture` is it as shown, `draws` and `blend` what
/// drew it from `m`. Returns the folder.
#[allow(clippy::too_many_arguments)]
pub fn save(
    gpu: &present::Gpu,
    texture: &wgpu::Texture,
    m: &Match,
    dol: &[u8],
    draws: &[Draw],
    blend: Option<Blend<'_>>,
    about: &About,
    live: &Live,
) -> Result<PathBuf, String> {
    let dir = folder()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let save = |name: &str, f: &melee_gpu::Frame| {
        let p = dir.join(name);
        f.save_png(&p).map_err(|e| format!("{}: {e}", p.display()))
    };
    save("shown.png", &read_back(gpu, texture))?;
    // A new frame builder and renderer, on this thread, drawing the same draws.
    let mut frames = Frames::new();
    frames.space.load_dol(dol);
    frames.hsd.widescreen = about.widescreen;
    let mut r = pipelines::renderer(gpu, about.scale, about.msaa, about.aspect);
    r.xfb_readback = true;
    let mut fresh = Gpu::new(r);
    let mut regs = String::new();
    for (name, b) in [("fresh.png", blend), ("fresh-unblended.png", None)] {
        let fifo = frames.frame_blended(&m.game, &m.data, draws, b);
        let frame = fresh.run(&frames.space, &m.data, &fifo, None).pop();
        match frame {
            Some(f) => save(name, &f)?,
            None => return Err(format!("{name}: the stream copied no frame")),
        }
        if regs.is_empty() {
            live.after.diff(&Regs::of(&fresh.state), &mut regs);
            let _ = writeln!(
                regs,
                "
HSD state cache, live:
{}
fresh:
{:?}",
                live.hsd, frames.hsd.state
            );
        }
    }
    std::fs::write(dir.join("registers.txt"), regs).map_err(|e| format!("registers.txt: {e}"))?;
    // The player's own stream, from its registers, on a new renderer.
    let mut r = pipelines::renderer(gpu, about.scale, about.msaa, about.aspect);
    r.xfb_readback = true;
    let mut again = Gpu::new(r);
    live.before.set(&mut again.state);
    match again.run(live.space, &m.data, live.fifo, None).pop() {
        Some(f) => save("live-stream.png", &f)?,
        None => return Err("live-stream.png: the stream copied no frame".into()),
    }
    let mut info = String::new();
    let _ = writeln!(info, "mode {}", about.mode);
    let _ = writeln!(info, "tick {}", about.tick);
    match blend {
        Some(b) => {
            let _ = writeln!(
                info,
                "blend t {} (capture held: {})",
                b.t,
                b.from.is_filled()
            );
        }
        None => {
            let _ = writeln!(info, "no blend");
        }
    }
    let _ = writeln!(
        info,
        "scale {}, msaa {}, aspect {:?}, widescreen {:?}",
        about.scale, about.msaa, about.aspect, about.widescreen
    );
    let _ = writeln!(info, "draws {}", draws.len());
    let _ = writeln!(info, "notes:\n{}", about.notes);
    std::fs::write(dir.join("info.txt"), info).map_err(|e| format!("info.txt: {e}"))?;
    Ok(dir)
}

/// Draws the player's frame again on its own renderer, `live`, from the registers the frame
/// found: as it is (`again.png`), after clearing its EFB as a new renderer's is
/// (`again-cleared.png`), and after dropping its decoded textures too
/// (`again-new-textures.png`). Whichever comes back whole names what went stale. Only with
/// the renderer on the game's thread (`--one-thread`).
pub fn experiments(
    gpu: &present::Gpu,
    live: &mut Gpu,
    l: &Live,
    data: &melee_data::archive::Archives,
    dir: &Path,
) -> Result<(), String> {
    if live.renderer().is_none() {
        return std::fs::write(
            dir.join("experiments.txt"),
            "the renderer runs on its own threads: run with --one-thread for the experiments
",
        )
        .map_err(|e| format!("experiments.txt: {e}"));
    }
    let again = |name: &str, live: &mut Gpu| -> Result<(), String> {
        l.before.set(&mut live.state);
        let frame = live.run(l.space, data, l.fifo, None).pop();
        let Some(tex) = frame.as_ref().and_then(|f| f.texture.as_ref()) else {
            return Err(format!("{name}: the stream copied no frame"));
        };
        let p = dir.join(name);
        read_back(gpu, tex)
            .save_png(&p)
            .map_err(|e| format!("{}: {e}", p.display()))
    };
    // The EFB as the shown frame left it: its copy to the XFB clears it, color and depth.
    if let Some(mut r) = live.renderer()
        && let Some((w, h, depth, color)) = r.read_targets()
    {
        let far = depth.iter().filter(|&&d| d >= 0.999).count();
        let near = depth.iter().filter(|&&d| d < 0.5).count();
        let (lo, hi) = depth
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), &d| (a.min(d), b.max(d)));
        let colored = color
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[..3] != [0, 0, 0])
            .count();
        let text = format!(
            "EFB after the shown frame, {w}x{h}: depth {far} texels far (>= 0.999), {near} near (< 0.5), min {lo}, max {hi}; color {colored} texels not black
"
        );
        std::fs::write(dir.join("efb-after.txt"), text)
            .map_err(|e| format!("efb-after.txt: {e}"))?;
        let gray: Vec<u8> = depth
            .iter()
            .flat_map(|&d| {
                let v = (d.clamp(0.0, 1.0) * 255.0) as u8;
                [v, v, v, 255]
            })
            .collect();
        let save = |name: &str, rgba: Vec<u8>| {
            let f = melee_gpu::Frame {
                width: w,
                height: h,
                rgba,
                texture: None,
            };
            let p = dir.join(name);
            f.save_png(&p).map_err(|e| format!("{}: {e}", p.display()))
        };
        save("efb-depth-after.png", gray)?;
        save("efb-color-after.png", color)?;
    }
    again("again.png", live)?;
    live.clear_efb_new();
    again("again-cleared.png", live)?;
    if let Some(mut r) = live.renderer() {
        r.drop_textures();
    }
    again("again-new-textures.png", live)?;
    Ok(())
}

/// `captures/<seconds since 1970>` beside the executable.
fn folder() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("the executable's path: {e}"))?;
    let base = exe.parent().unwrap_or(Path::new(".")).join("captures");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    Ok(base.join(now.to_string()))
}

/// A frame on the GPU, read back.
fn read_back(gpu: &present::Gpu, texture: &wgpu::Texture) -> melee_gpu::Frame {
    let (width, height) = (texture.width(), texture.height());
    let row = width * 4;
    let padded = row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("capture readback"),
        size: u64::from(padded * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        texture.size(),
    );
    gpu.queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    let mut rgba = Vec::with_capacity((row * height) as usize);
    if let Ok(data) = slice.get_mapped_range() {
        for r in data.chunks(padded as usize) {
            rgba.extend_from_slice(&r[..row as usize]);
        }
    }
    buffer.unmap();
    melee_gpu::Frame {
        width,
        height,
        rgba,
        texture: None,
    }
}
