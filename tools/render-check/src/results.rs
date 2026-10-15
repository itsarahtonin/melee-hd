//! What `compare` and `scaled` measured, kept for the dashboard: with `CHECK_RESULTS=DIR`
//! (and `CHECK_ID=NAME`, the check's name), each run writes `DIR/NAME.tsv`, a line per image
//! compared, and each comparison's images at full size. The tests add `DIR/NAME.status`: the
//! stage, what the check expects and whether it passed. `melee-render-check dashboard DIR
//! OUT_DIR` makes a page of them, `OUT_DIR/index.html`, with the images in `OUT_DIR/img/`: for
//! a comparison against ssbm-rs, the two frames and their difference pixel by pixel; for a
//! scaled check, the two frames and a heatmap of the blocks the check measures.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Where a run's results go, from `CHECK_RESULTS` and `CHECK_ID`.
pub struct Results {
    dir: PathBuf,
    id: String,
}

/// One image compared.
pub struct Measure<'a> {
    /// `compare` (against ssbm-rs's frame) or `scaled` (against melee-render's own at 1x).
    pub kind: &'a str,
    /// The log's frame (0 or 1), or the scaled view's name.
    pub what: &'a str,
    pub native: u32,
    pub psnr: f64,
    pub off8: f64,
    pub off32: f64,
    /// The scaled checks' worst block (`worst_block`), in levels.
    pub block: f64,
    pub within: bool,
}

/// An RGB image.
pub struct Rgb<'a> {
    pub width: u32,
    pub height: u32,
    pub pixels: &'a [u8],
}

impl Results {
    pub fn from_env() -> Option<Results> {
        let dir = PathBuf::from(std::env::var_os("CHECK_RESULTS")?);
        let id = std::env::var("CHECK_ID").unwrap_or_else(|_| "run".to_owned());
        std::fs::create_dir_all(&dir).ok()?;
        let r = Results { dir, id };
        let _ = std::fs::remove_file(r.path("tsv"));
        // The last run's images.
        let prefix = format!("{}~", r.id);
        for e in std::fs::read_dir(&r.dir).ok()?.flatten() {
            if e.file_name().to_string_lossy().starts_with(&prefix) {
                let _ = std::fs::remove_file(e.path());
            }
        }
        Some(r)
    }

    fn path(&self, ext: &str) -> PathBuf {
        self.dir.join(format!("{}.{ext}", self.id))
    }

    /// Records a measure and the images compared, at full size and losslessly (quickly
    /// compressed; the dashboard compresses them harder): `ours`, `theirs` (ssbm-rs's frame,
    /// or for a scaled check melee-render's own frame it's held against) and, if given, their
    /// difference (RGBA, gray: four times each pixel's largest channel difference, as
    /// ssbm-rs's ladder shows it). A scaled check gives none: the dashboard draws the blocks it
    /// measures from the two frames.
    pub fn record(&self, m: &Measure<'_>, ours: Rgb<'_>, theirs: Rgb<'_>, diff: Option<&[u8]>) {
        let line = format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            m.kind, m.what, m.native, m.psnr, m.off8, m.off32, m.within, m.block
        );
        let path = self.path("tsv");
        let mut text = std::fs::read_to_string(&path).unwrap_or_default();
        text.push_str(&line);
        let _ = std::fs::write(&path, text);
        let name = |kind: &str| {
            self.dir
                .join(format!("{}~{}~{kind}.png", self.id, tag(m.what)))
        };
        let rgb = png::ColorType::Rgb;
        save(&name("ours"), ours.width, ours.height, rgb, ours.pixels);
        save(
            &name("theirs"),
            theirs.width,
            theirs.height,
            rgb,
            theirs.pixels,
        );
        if let Some(diff) = diff {
            let gray: Vec<u8> = diff.iter().step_by(4).copied().collect();
            let (w, h) = (ours.width, ours.height);
            save(&name("diff"), w, h, png::ColorType::Grayscale, &gray);
        }
    }
}

/// A measure's name as file names take it: `3x 16:9` is `3x-16-9`.
pub fn tag(what: &str) -> String {
    let mut out = String::new();
    for c in what.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_owned()
}

fn save(path: &Path, w: u32, h: u32, color: png::ColorType, pixels: &[u8]) {
    let Ok(file) = std::fs::File::create(path) else {
        return;
    };
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(color);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Fast);
    let _ = enc
        .write_header()
        .and_then(|mut wr| wr.write_image_data(pixels));
}

/// The page and its images must fit one artifact publish (255 files, 64 MB) with room to
/// spare: at most this many files, `index.html` included...
pub const MAX_FILES: usize = 250;
/// ...and this many bytes.
pub const MAX_BYTES: u64 = 60_000_000;
// An artifact publish takes 255 files and 64 MB.
const _: () = assert!(MAX_FILES < 255 && MAX_BYTES < 64_000_000);

/// A measure's three images: the source (`theirs` as recorded), ours, and the difference, as
/// the page's viewer numbers them. A scaled check's third is the heatmap of its blocks, drawn
/// from the first two ([`heatmap`]), named `heat`.
const KINDS: [(&str, &str); 3] = [("theirs", "ref"), ("ours", "ours"), ("diff", "diff")];

/// The page's name for one of a measure's images: `img/` holds `CHECK-FRAME-KIND.png`, KIND
/// `ref`, `ours`, `diff` or `heat`.
pub fn image_name(id: &str, what: &str, kind: &str) -> String {
    format!("{id}-{}-{kind}.png", tag(what))
}

/// An image decoded: gray when every pixel is, else RGB; or RGBA (a heatmap).
struct Image {
    width: u32,
    height: u32,
    color: png::ColorType,
    pixels: Vec<u8>,
}

impl Image {
    fn load(path: &Path) -> Option<Image> {
        let file = std::io::BufReader::new(std::fs::File::open(path).ok()?);
        let mut decoder = png::Decoder::new(file);
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().ok()?;
        let mut buf = vec![0; reader.output_buffer_size()?];
        let info = reader.next_frame(&mut buf).ok()?;
        let channels = info.color_type.samples();
        let buf = &buf[..info.buffer_size()];
        let (width, height) = (info.width, info.height);
        let image = |color, pixels| {
            Some(Image {
                width,
                height,
                color,
                pixels,
            })
        };
        if channels <= 2 {
            let pixels = buf.chunks_exact(channels).map(|p| p[0]).collect();
            return image(png::ColorType::Grayscale, pixels);
        }
        let px = buf.chunks_exact(channels);
        if px.clone().all(|p| p[0] == p[1] && p[1] == p[2]) {
            let pixels = px.map(|p| p[0]).collect();
            return image(png::ColorType::Grayscale, pixels);
        }
        let pixels = px.flat_map(|p| [p[0], p[1], p[2]]).collect();
        image(png::ColorType::Rgb, pixels)
    }

    /// The image's pixels as RGB.
    fn rgb(&self) -> Vec<u8> {
        match self.color {
            png::ColorType::Grayscale => self.pixels.iter().flat_map(|&v| [v, v, v]).collect(),
            png::ColorType::Rgba => self
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|p| [p[0], p[1], p[2]])
                .collect(),
            _ => self.pixels.clone(),
        }
    }

    /// What tells two images apart: the same for the same pixels, however they were stored.
    fn key(&self) -> (u64, u32, u32) {
        let mut h = std::hash::DefaultHasher::new();
        (self.color as u8).hash(&mut h);
        self.pixels.hash(&mut h);
        (h.finish(), self.width, self.height)
    }

    /// The image as the smallest PNG the encoder's strongest compression makes of it, of a few
    /// filter choices; lossless.
    fn encode(&self) -> Vec<u8> {
        let mut best: Option<Vec<u8>> = None;
        for filter in [png::Filter::Adaptive, png::Filter::MinEntropy] {
            let mut buf = Vec::new();
            let mut enc = png::Encoder::new(&mut buf, self.width, self.height);
            enc.set_color(self.color);
            enc.set_depth(png::BitDepth::Eight);
            enc.set_compression(png::Compression::High);
            enc.set_filter(filter);
            enc.write_header()
                .and_then(|mut w| {
                    w.write_image_data(&self.pixels)?;
                    w.finish()
                })
                .expect("encoding an image in memory");
            if best.as_ref().is_none_or(|b| buf.len() < b.len()) {
                best = Some(buf);
            }
        }
        best.unwrap_or_default()
    }
}

/// The heatmap's ramp from no difference to the scaled checks' block limit, evenly spaced:
/// matplotlib's inferno up to nine tenths, near black for the few levels normal detail makes
/// (edges, mip levels and dither at a higher resolution average out over a block), through
/// purple and red to orange and yellow as a block nears the limit.
pub const HEAT_RAMP: [[u8; 3]; 10] = [
    [0x00, 0x00, 0x04],
    [0x16, 0x0b, 0x39],
    [0x42, 0x0a, 0x68],
    [0x6a, 0x17, 0x6e],
    [0x93, 0x26, 0x67],
    [0xbc, 0x37, 0x54],
    [0xdd, 0x51, 0x3a],
    [0xf3, 0x78, 0x19],
    [0xfc, 0xa5, 0x0a],
    [0xf6, 0xd7, 0x46],
];
/// A block past the limit: a color the ramp never reaches.
pub const HEAT_OVER: [u8; 3] = [0x00, 0xe5, 0xff];

/// A block's color in the heatmap, `mean` levels against a limit of `limit`.
pub fn heat_color(mean: f64, limit: f64) -> [u8; 3] {
    if mean > limit {
        return HEAT_OVER;
    }
    let t = (mean / limit).clamp(0.0, 1.0) * (HEAT_RAMP.len() - 1) as f64;
    let i = (t.floor() as usize).min(HEAT_RAMP.len() - 2);
    let f = t - i as f64;
    let (a, b) = (HEAT_RAMP[i], HEAT_RAMP[i + 1]);
    std::array::from_fn(|k| {
        (f64::from(a[k]) + (f64::from(b[k]) - f64::from(a[k])) * f).round() as u8
    })
}

/// A scaled check's blocks, as [`crate::block_means`] measures them.
#[derive(Debug, Clone, PartialEq)]
struct Blocks {
    side: u32,
    cols: u32,
    rows: u32,
    means: Vec<f64>,
}

/// The heatmap of what a scaled check measures, at the frames' size: each block
/// ([`crate::block_side`] pixels square, [`crate::block_means`]) filled with its color on the
/// ramp ([`heat_color`] against [`crate::SCALED_BLOCK`]), the pixels past the last whole block
/// (which the check doesn't measure) transparent. `None` if the frames' sizes differ.
fn heatmap(ours: &Image, theirs: &Image) -> Option<(Image, Blocks)> {
    let (w, h) = (ours.width, ours.height);
    if (theirs.width, theirs.height) != (w, h) {
        return None;
    }
    let side = crate::block_side(h).max(1);
    let means = crate::block_means(&ours.rgb(), &theirs.rgb(), w, side);
    let (cols, rows) = (w / side, h / side);
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    for y in 0..rows * side {
        for x in 0..cols * side {
            let [r, g, b] = heat_color(
                means[(y / side * cols + x / side) as usize],
                crate::SCALED_BLOCK,
            );
            let at = ((y * w + x) * 4) as usize;
            pixels[at..at + 4].copy_from_slice(&[r, g, b, 255]);
        }
    }
    let image = Image {
        width: w,
        height: h,
        color: png::ColorType::Rgba,
        pixels,
    };
    Some((
        image,
        Blocks {
            side,
            cols,
            rows,
            means,
        },
    ))
}

/// Where one of the page's images comes from: a recorded image, or a scaled check's heatmap,
/// drawn from its two frames.
enum Source {
    File(PathBuf),
    Heat { ours: PathBuf, theirs: PathBuf },
}

impl Source {
    fn image(&self) -> Option<(Image, Option<Blocks>)> {
        match self {
            Source::File(p) => Some((Image::load(p)?, None)),
            Source::Heat { ours, theirs } => {
                let (image, blocks) = heatmap(&Image::load(ours)?, &Image::load(theirs)?)?;
                Some((image, Some(blocks)))
            }
        }
    }

    fn describe(&self) -> String {
        match self {
            Source::File(p) => p.display().to_string(),
            Source::Heat { ours, .. } => format!("the heatmap of {}", ours.display()),
        }
    }
}

/// `f` of each item, on up to four threads.
fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(4));
    let next = AtomicUsize::new(0);
    let mut done: Vec<(usize, R)> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut out = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(i) else {
                            break;
                        };
                        out.push((i, f(item)));
                    }
                    out
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().expect("a worker"))
            .collect()
    });
    done.sort_by_key(|d| d.0);
    done.into_iter().map(|d| d.1).collect()
}

fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '<' => out.push_str("\\u003c"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_num(v: f64) -> String {
    if v.is_finite() {
        format!("{v:.4}")
    } else {
        "null".to_owned()
    }
}

/// A check's measure as the page shows it.
struct Row {
    kind: String,
    what: String,
    native: u32,
    psnr: f64,
    off8: f64,
    off32: f64,
    within: bool,
    block: f64,
    /// Of the check's first frame: a compare check's log frame 0; a scaled check's views are
    /// all of one frame.
    first: bool,
    /// The page's images (indices into the unique images), source, ours and difference (a
    /// scaled check's heatmap).
    images: [Option<usize>; 3],
    /// A scaled check's blocks, for the viewer's readout.
    blocks: Option<Blocks>,
    /// Whether the page shows the images (within the budget).
    shown: bool,
}

struct Check {
    id: String,
    fields: Vec<(&'static str, String)>,
    rows: Vec<Row>,
}

/// One image the page shows, written once however many measures show it.
struct Unique {
    source: Source,
    name: String,
    width: u32,
    height: u32,
    bytes: u64,
}

/// What the dashboard wrote.
pub struct Summary {
    pub checks: usize,
    pub measures: usize,
    /// Measures with images recorded, and those the page shows them for.
    pub recorded: usize,
    pub shown: usize,
    /// Files written, `index.html` included, and their bytes.
    pub files: usize,
    pub bytes: u64,
}

/// Whether a page of `files` files (`index.html` included) and `bytes` bytes fits the budget.
pub fn fits(files: usize, bytes: u64) -> bool {
    files <= MAX_FILES && bytes <= MAX_BYTES
}

/// Which measures keep their images within the budget, given each measure's priority (lower
/// first; ties in the order given) and its images (unique image, bytes), starting from
/// `files` and `bytes` (the page itself): in priority order, each measure whose images not
/// yet kept still fit, all three or none.
fn within_budget(measures: &[(u8, Vec<(usize, u64)>)], files: usize, bytes: u64) -> Vec<bool> {
    let mut order: Vec<usize> = (0..measures.len()).collect();
    order.sort_by_key(|&i| measures[i].0);
    let (mut files, mut bytes) = (files, bytes);
    let mut kept = std::collections::HashSet::new();
    let mut shown = vec![false; measures.len()];
    for i in order {
        let mut new: Vec<(usize, u64)> = measures[i]
            .1
            .iter()
            .copied()
            .filter(|(u, _)| !kept.contains(u))
            .collect();
        new.sort_unstable();
        new.dedup();
        let add: u64 = new.iter().map(|x| x.1).sum();
        if fits(files + new.len(), bytes + add) {
            files += new.len();
            bytes += add;
            kept.extend(new.iter().map(|x| x.0));
            shown[i] = true;
        }
    }
    shown
}

const FIELDS: [&str; 10] = [
    "stage", "match", "frame", "group", "kind", "expect", "why", "result", "detail", "seconds",
];

/// The checks recorded in `dir`, their images not yet read.
fn read_checks(dir: &Path) -> Result<Vec<Check>, String> {
    let mut ids: Vec<String> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            let ext = p.extension()?.to_str()?;
            (ext == "tsv" || ext == "status").then(|| p.file_stem()?.to_str().map(str::to_owned))?
        })
        .collect();
    ids.sort();
    ids.dedup();
    let mut checks = Vec::new();
    for id in ids {
        let status = std::fs::read_to_string(dir.join(format!("{id}.status"))).unwrap_or_default();
        let fields = FIELDS
            .iter()
            .map(|&k| {
                let v = status
                    .lines()
                    .find_map(|l| l.strip_prefix(k).and_then(|r| r.strip_prefix('\t')))
                    .unwrap_or("");
                (k, v.to_owned())
            })
            .collect();
        let mut rows = Vec::new();
        for line in std::fs::read_to_string(dir.join(format!("{id}.tsv")))
            .unwrap_or_default()
            .lines()
        {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 7 {
                continue;
            }
            let num = |s: &str| s.parse::<f64>().unwrap_or(f64::NAN);
            rows.push(Row {
                kind: f[0].to_owned(),
                what: f[1].to_owned(),
                native: f[2].parse().unwrap_or(0),
                psnr: num(f[3]),
                off8: num(f[4]),
                off32: num(f[5]),
                within: f[6] == "true",
                block: num(f.get(7).copied().unwrap_or("")),
                first: f[0] != "compare" || f[1] == "0",
                images: [None; 3],
                blocks: None,
                shown: false,
            });
        }
        checks.push(Check { id, fields, rows });
    }
    Ok(checks)
}

/// The page, each measure's images as `img/NAME` where it shows them.
fn page(checks: &[Check], uniques: &[Unique], images: &str) -> String {
    let mut out = Vec::new();
    for c in checks {
        let measures: Vec<String> = c
            .rows
            .iter()
            .map(|r| {
                let img = |k: usize| match r.images[k] {
                    Some(u) if r.shown => json_str(&format!("img/{}", uniques[u].name)),
                    _ => "null".to_owned(),
                };
                let size = r.images.iter().flatten().next().map(|&u| &uniques[u]);
                // Each block's mean in tenths of a level, row by row.
                let blocks = match &r.blocks {
                    Some(b) if r.shown && r.images[2].is_some() => {
                        let tenths: Vec<String> = b
                            .means
                            .iter()
                            .map(|m| format!("{}", (m * 10.0).round() as i64))
                            .collect();
                        format!(
                            ",\"blocks\":{{\"side\":{},\"cols\":{},\"rows\":{},\"tenths\":[{}]}}",
                            b.side,
                            b.cols,
                            b.rows,
                            tenths.join(",")
                        )
                    }
                    _ => String::new(),
                };
                format!(
                    "{{\"kind\":{},\"what\":{},\"native\":{},\"psnr\":{},\"off8\":{},\"off32\":{},\"within\":{},\"block\":{},\"first\":{},\"w\":{},\"h\":{},\"ref\":{},\"ours\":{},\"diff\":{}{blocks}}}",
                    json_str(&r.kind),
                    json_str(&r.what),
                    r.native,
                    json_num(r.psnr),
                    json_num(r.off8),
                    json_num(r.off32),
                    r.within,
                    json_num(r.block),
                    r.first,
                    size.map_or(0, |u| u.width),
                    size.map_or(0, |u| u.height),
                    img(0),
                    img(1),
                    img(2),
                )
            })
            .collect();
        let fields: String = c
            .fields
            .iter()
            .map(|(k, v)| format!(",{}:{}", json_str(k), json_str(v)))
            .collect();
        out.push(format!(
            "{{\"id\":{}{fields},\"measures\":[{}]}}",
            json_str(&c.id),
            measures.join(",")
        ));
    }
    let generated = std::process::Command::new("git")
        .args(["log", "-1", "--format=%h %s"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default();
    let hex = |c: [u8; 3]| json_str(&format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]));
    let ramp: Vec<String> = HEAT_RAMP.iter().map(|&c| hex(c)).collect();
    let data = format!(
        "{{\"commit\":{},\"generated\":{},\"tolerance\":{{\"psnr\":38,\"off32\":0.25}},\"scaled\":{{\"psnr\":{},\"block\":{},\"ramp\":[{}],\"over\":{}}},\"images\":{images},\"checks\":[{}]}}",
        json_str(&generated),
        json_str(&now()),
        crate::SCALED_PSNR,
        crate::SCALED_BLOCK,
        ramp.join(","),
        hex(HEAT_OVER),
        out.join(",")
    );
    include_str!("dashboard.html").replace("/*DATA*/null", &data)
}

/// The dashboard of the results in `dir`: `out/index.html`, and each measure's three images
/// at full size in `out/img/` (a scaled check's third its heatmap, [`heatmap`]; an image
/// several measures show written once), all of them if
/// they fit the budget ([`MAX_FILES`], [`MAX_BYTES`]), else as many as fit: the frames at the
/// console's size first (each check's first frame before its others), then the larger ones.
pub fn dashboard(dir: &Path, out: &Path) -> Result<Summary, String> {
    let mut checks = read_checks(dir)?;
    let img_dir = out.join("img");
    std::fs::create_dir_all(&img_dir).map_err(|e| format!("{}: {e}", img_dir.display()))?;
    // The last page's images.
    for e in std::fs::read_dir(&img_dir).into_iter().flatten().flatten() {
        if e.path().extension().is_some_and(|x| x == "png") {
            let _ = std::fs::remove_file(e.path());
        }
    }
    // Each measure's images, in the page's order; the same pixels are one image, named for
    // the first measure that shows them.
    let mut wanted = Vec::new();
    for (ci, c) in checks.iter().enumerate() {
        for (ri, r) in c.rows.iter().enumerate() {
            let stored = |kind: &str| dir.join(format!("{}~{}~{kind}.png", c.id, tag(&r.what)));
            for (k, (kind, name)) in KINDS.iter().enumerate() {
                if k == 2 && r.kind == "scaled" {
                    // The heatmap of what the check measures, in place of the pixels'
                    // difference: drawn at a higher resolution, the frame differs pixel by
                    // pixel in detail the check tolerates.
                    let (ours, theirs) = (stored("ours"), stored("theirs"));
                    if ours.exists() && theirs.exists() {
                        wanted.push((ci, ri, k, "heat", Source::Heat { ours, theirs }));
                    }
                    continue;
                }
                let p = stored(kind);
                if p.exists() {
                    wanted.push((ci, ri, k, *name, Source::File(p)));
                }
            }
        }
    }
    let keys = par_map(&wanted, |w| w.4.image().map(|(i, b)| (i.key(), b)));
    let mut uniques: Vec<Unique> = Vec::new();
    let mut seen: HashMap<(u64, u32, u32), usize> = HashMap::new();
    for ((ci, ri, k, name, source), key) in wanted.into_iter().zip(keys) {
        let Some((key, blocks)) = key else {
            continue;
        };
        let u = *seen.entry(key).or_insert_with(|| {
            let c = &checks[ci];
            uniques.push(Unique {
                source,
                name: image_name(&c.id, &c.rows[ri].what, name),
                width: key.1,
                height: key.2,
                bytes: 0,
            });
            uniques.len() - 1
        });
        let row = &mut checks[ci].rows[ri];
        row.images[k] = Some(u);
        if blocks.is_some() {
            row.blocks = blocks;
        }
    }
    let sizes = par_map(&uniques, |u| -> Result<u64, String> {
        let (img, _) = u
            .source
            .image()
            .ok_or_else(|| format!("{}: unreadable", u.source.describe()))?;
        let bytes = img.encode();
        let path = img_dir.join(&u.name);
        std::fs::write(&path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(bytes.len() as u64)
    });
    for (u, s) in uniques.iter_mut().zip(sizes) {
        u.bytes = s?;
    }
    // The most the budget allows, as many measures as can be: each check's first frame at the
    // console's size first, then its other frames at that size, then the larger images (16:9
    // drawn at 3x, about 4 MB a comparison: checks not passing, then the standard set's, first).
    let mut at = Vec::new();
    let mut measures = Vec::new();
    for (ci, c) in checks.iter().enumerate() {
        let field = |k: &str| {
            c.fields
                .iter()
                .find(|f| f.0 == k)
                .map_or("", |f| f.1.as_str())
        };
        let later =
            u8::from(field("result") == "pass") + 2 * u8::from(field("group") != "standard");
        for (ri, r) in c.rows.iter().enumerate() {
            let images: Vec<(usize, u64)> = r
                .images
                .iter()
                .flatten()
                .map(|&u| (u, uniques[u].bytes))
                .collect();
            let Some(&(u, _)) = images.first() else {
                continue;
            };
            let large = uniques[u].width * uniques[u].height > 640 * 480;
            let priority = match (r.first, large) {
                (true, false) => 0,
                (false, false) => 1,
                (_, true) => 2 + later + 4 * u8::from(!r.first),
            };
            at.push((ci, ri));
            measures.push((priority, images));
        }
    }
    let info = |files: usize, bytes: u64, all: bool| {
        format!("{{\"files\":{files},\"bytes\":{bytes},\"all\":{all}}}")
    };
    // The page is a little shorter for each image it leaves out.
    for &(ci, ri) in &at {
        checks[ci].rows[ri].shown = true;
    }
    let most = page(&checks, &uniques, &info(MAX_FILES, MAX_BYTES, false)).len() as u64;
    let shown = within_budget(&measures, 1, most);
    let mut used = vec![false; uniques.len()];
    for (&(ci, ri), &s) in at.iter().zip(&shown) {
        let r = &mut checks[ci].rows[ri];
        r.shown = s;
        if s {
            for &u in r.images.iter().flatten() {
                used[u] = true;
            }
        }
    }
    for (u, _) in uniques.iter().zip(&used).filter(|x| !*x.1) {
        let _ = std::fs::remove_file(img_dir.join(&u.name));
    }
    let files = 1 + used.iter().filter(|&&u| u).count();
    let image_bytes: u64 = uniques
        .iter()
        .zip(&used)
        .filter(|x| *x.1)
        .map(|x| x.0.bytes)
        .sum();
    let all = shown.iter().all(|&s| s);
    let guess = page(&checks, &uniques, &info(files, image_bytes, all)).len() as u64;
    let html = page(&checks, &uniques, &info(files, image_bytes + guess, all));
    let index = out.join("index.html");
    std::fs::write(&index, &html).map_err(|e| format!("{}: {e}", index.display()))?;
    Ok(Summary {
        checks: checks.len(),
        measures: checks.iter().map(|c| c.rows.len()).sum(),
        recorded: shown.len(),
        shown: shown.iter().filter(|&&s| s).count(),
        files,
        bytes: image_bytes + html.len() as u64,
    })
}

/// The time now, UTC, as `YYYY-MM-DD HH:MM`.
fn now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = (secs / 86_400) as i64;
    // Days to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02} UTC",
        secs / 3600 % 24,
        secs / 60 % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(tag("3x 16:9"), "3x-16-9");
        assert_eq!(tag("1x GPU XFB"), "1x-gpu-xfb");
        assert_eq!(tag("0"), "0");
        assert_eq!(
            image_name("fox-marth-ps-f00600-scaled", "3x 16:9", "diff"),
            "fox-marth-ps-f00600-scaled-3x-16-9-diff.png"
        );
        assert_eq!(
            image_name("fox-fox-fd-f00140", "1", "ref"),
            "fox-fox-fd-f00140-1-ref.png"
        );
    }

    #[test]
    fn budget() {
        assert!(fits(MAX_FILES, MAX_BYTES));
        assert!(!fits(MAX_FILES + 1, 1));
        assert!(!fits(2, MAX_BYTES + 1));
    }

    #[test]
    fn budget_order() {
        let mb = 1_000_000;
        // First frames first; a measure that doesn't fit is left out, and a later smaller one
        // still goes in; an image already kept costs nothing again.
        let measures = [
            (5, vec![(0, 20 * mb), (1, mb)]),
            (0, vec![(2, 30 * mb), (2, 30 * mb), (3, mb)]),
            (1, vec![(4, 40 * mb)]),
            (5, vec![(2, 30 * mb), (5, mb)]),
        ];
        assert_eq!(
            within_budget(&measures, 1, 100_000),
            [true, true, false, true]
        );
        // The file count binds too.
        let many: Vec<_> = (0..MAX_FILES).map(|u| (0, vec![(u, 1)])).collect();
        let kept = within_budget(&many, 1, 0);
        assert_eq!(kept.iter().filter(|&&k| k).count(), MAX_FILES - 1);
    }

    /// Two checks' results, a frame identical and a frame not, made into a page.
    #[test]
    fn page_and_images() {
        let base = std::env::temp_dir().join(format!("render-check-dash-{}", std::process::id()));
        let (dir, out) = (base.join("results"), base.join("site"));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&dir).unwrap();
        let (w, h) = (8u32, 6u32);
        let a: Vec<u8> = (0..w * h * 3).map(|i| (i * 7) as u8).collect();
        let mut b = a.clone();
        b[0] ^= 0x40;
        fn rgb(p: &[u8]) -> Rgb<'_> {
            Rgb {
                width: 8,
                height: 6,
                pixels: p,
            }
        }
        let diff = |p: &[u8], q: &[u8]| crate::image_diff(p, q).3;
        let r = Results {
            dir: dir.clone(),
            id: "x-f00600".to_owned(),
        };
        let m = |what, psnr| Measure {
            kind: "compare",
            what,
            native: 597,
            psnr,
            off8: 0.0,
            off32: 0.0,
            block: f64::NAN,
            within: true,
        };
        r.record(
            &m("0", f64::INFINITY),
            rgb(&a),
            rgb(&a),
            Some(&diff(&a, &a)),
        );
        r.record(&m("1", 40.0), rgb(&b), rgb(&a), Some(&diff(&b, &a)));
        std::fs::write(dir.join("x-f00600.status"), "stage\tFD\nresult\tpass\n").unwrap();
        let s = dashboard(&dir, &out).unwrap();
        assert_eq!((s.checks, s.measures, s.recorded, s.shown), (1, 2, 2, 2));
        // Frame 0's source and ours are one image, and frame 1's source is frame 0's too.
        let mut files: Vec<String> = std::fs::read_dir(out.join("img"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        files.sort();
        assert_eq!(
            files,
            [
                "x-f00600-0-diff.png",
                "x-f00600-0-ref.png",
                "x-f00600-1-diff.png",
                "x-f00600-1-ours.png"
            ]
        );
        assert_eq!(s.files, 5);
        let html = std::fs::read_to_string(out.join("index.html")).unwrap();
        assert!(html.contains("<title>Render Check Coverage</title>"));
        assert!(
            html.contains("\"ref\":\"img/x-f00600-0-ref.png\",\"ours\":\"img/x-f00600-0-ref.png\"")
        );
        // Lossless: the images decode to what was recorded.
        let back = Image::load(&out.join("img/x-f00600-1-ours.png")).unwrap();
        assert_eq!((back.width, back.height, back.pixels), (w, h, b));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn heat_ramp() {
        let limit = crate::SCALED_BLOCK;
        assert_eq!(heat_color(0.0, limit), HEAT_RAMP[0]);
        assert_eq!(heat_color(limit, limit), HEAT_RAMP[9]);
        assert_eq!(heat_color(limit + 0.01, limit), HEAT_OVER);
        // Halfway up, halfway between the fifth and sixth stops.
        let mid = heat_color(limit / 2.0, limit);
        let (a, b) = (HEAT_RAMP[4], HEAT_RAMP[5]);
        for k in 0..3 {
            let want = (f64::from(a[k]) + f64::from(b[k])) / 2.0;
            assert!((f64::from(mid[k]) - want).abs() <= 0.5);
        }
        assert!(!HEAT_RAMP.contains(&HEAT_OVER));
    }

    /// Two frames 20 by 96 (blocks of 3, six a row and two columns past them), one block 40
    /// levels redder and one 10 bluer.
    fn heat_frames() -> (Vec<u8>, Vec<u8>) {
        let (w, h) = (20usize, 96usize);
        let theirs: Vec<u8> = (0..w * h * 3).map(|i| (i * 13 % 180 + 15) as u8).collect();
        let mut ours = theirs.clone();
        for y in 3..6 {
            for x in 6..9 {
                ours[(y * w + x) * 3] += 40;
            }
        }
        for y in 30..33 {
            for x in 15..18 {
                ours[(y * w + x) * 3 + 2] -= 10;
            }
        }
        (ours, theirs)
    }

    #[test]
    fn heatmap_blocks() {
        let (ours, theirs) = heat_frames();
        let rgb = |pixels: Vec<u8>| Image {
            width: 20,
            height: 96,
            color: png::ColorType::Rgb,
            pixels,
        };
        let (img, blocks) = heatmap(&rgb(ours.clone()), &rgb(theirs.clone())).unwrap();
        assert_eq!((blocks.side, blocks.cols, blocks.rows), (3, 6, 32));
        // What the check measures: its worst block is the heatmap's largest.
        let (worst, bx, by) = crate::worst_block(&ours, &theirs, 20, 3);
        assert_eq!((worst, bx, by), (40.0, 6, 3));
        assert_eq!(blocks.means.len(), 6 * 32);
        assert_eq!(blocks.means[6 + 2], 40.0);
        assert_eq!(blocks.means[10 * 6 + 5], 10.0);
        assert_eq!(blocks.means.iter().filter(|&&m| m != 0.0).count(), 2);
        let px = |x: usize, y: usize| -> [u8; 4] {
            let at = (y * 20 + x) * 4;
            img.pixels[at..at + 4].try_into().unwrap()
        };
        let [r, g, b] = HEAT_OVER;
        assert_eq!(px(7, 4), [r, g, b, 255]);
        let [r, g, b] = heat_color(10.0, crate::SCALED_BLOCK);
        assert_eq!(px(17, 32), [r, g, b, 255]);
        let [r, g, b] = HEAT_RAMP[0];
        assert_eq!(px(0, 0), [r, g, b, 255]);
        // Past the last whole block, not measured.
        assert_eq!(px(18, 0)[3], 0);
        assert_eq!(px(19, 95)[3], 0);
    }

    /// A scaled check's third image is the heatmap of its blocks, its readout in the data.
    #[test]
    fn scaled_heatmap_page() {
        let base = std::env::temp_dir().join(format!("render-check-heat-{}", std::process::id()));
        let (dir, out) = (base.join("results"), base.join("site"));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&dir).unwrap();
        let (ours, theirs) = heat_frames();
        let r = Results {
            dir: dir.clone(),
            id: "x-f00600-scaled".to_owned(),
        };
        let m = Measure {
            kind: "scaled",
            what: "2x",
            native: 600,
            psnr: 30.0,
            off8: 0.0,
            off32: 0.0,
            block: 40.0,
            within: false,
        };
        let rgb = |pixels| Rgb {
            width: 20,
            height: 96,
            pixels,
        };
        r.record(&m, rgb(&ours), rgb(&theirs), None);
        let s = dashboard(&dir, &out).unwrap();
        assert_eq!((s.measures, s.shown, s.files), (1, 1, 4));
        assert!(out.join("img/x-f00600-scaled-2x-heat.png").exists());
        assert!(!out.join("img/x-f00600-scaled-2x-diff.png").exists());
        let back = Image::load(&out.join("img/x-f00600-scaled-2x-heat.png")).unwrap();
        assert_eq!((back.width, back.height), (20, 96));
        let html = std::fs::read_to_string(out.join("index.html")).unwrap();
        assert!(html.contains("\"diff\":\"img/x-f00600-scaled-2x-heat.png\""));
        assert!(html.contains(
            "\"blocks\":{\"side\":3,\"cols\":6,\"rows\":32,\"tenths\":[0,0,0,0,0,0,0,0,400,"
        ));
        assert!(html.contains("\"over\":\"#00e5ff\""));
        let _ = std::fs::remove_dir_all(&base);
    }
}
