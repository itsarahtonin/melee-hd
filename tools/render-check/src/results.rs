//! What `compare` and `scaled` measured, kept for the dashboard: with `CHECK_RESULTS=DIR`
//! (and `CHECK_ID=NAME`, the check's name), each run writes `DIR/NAME.tsv`, a line per image
//! compared, and thumbnails of the first frame's images. The tests add `DIR/NAME.status`:
//! the stage, what the check expects and whether it passed. `melee-render-check dashboard DIR
//! OUT.html` makes one self-contained page of them.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

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
        // The last run's thumbnails.
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

    /// Records a measure, with thumbnails about 213 pixels wide, to keep the dashboard to a few
    /// MB: of `ours` (for a scaled check only out of tolerance) and, when the two differ, of
    /// `theirs` (only against ssbm-rs: a scaled check's is the 1x frame, much like ours) and of
    /// their difference image (RGBA, gray).
    pub fn record(&self, m: &Measure<'_>, images: Option<(Rgb<'_>, Rgb<'_>, &[u8])>) {
        let line = format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            m.kind, m.what, m.native, m.psnr, m.off8, m.off32, m.within, m.block
        );
        let path = self.path("tsv");
        let mut text = std::fs::read_to_string(&path).unwrap_or_default();
        text.push_str(&line);
        let _ = std::fs::write(&path, text);
        let Some((ours, theirs, diff)) = images else {
            return;
        };
        if m.kind == "scaled" && !m.psnr.is_finite() {
            return;
        }
        let name = |what: &str| {
            self.dir.join(format!(
                "{}~{}~{}.png",
                self.id,
                m.what.replace(' ', "_"),
                what
            ))
        };
        let k = (ours.width / 213).max(1);
        let small = |img: &Rgb<'_>| crate::downsample(img.width, img.height, img.pixels, k, k);
        let (w, h) = (ours.width / k, ours.height / k);
        // A scaled check within its tolerance keeps only the difference.
        if m.kind == "compare" || !m.within {
            save_rgb(&name("ours"), w, h, &small(&ours));
        }
        if !m.psnr.is_finite() {
            return;
        }
        if m.kind == "compare" {
            save_rgb(&name("theirs"), w, h, &small(&theirs));
        }
        // The difference's thumbnail keeps each block's largest difference, so a pixel's stays
        // visible.
        let mut out = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            for x in 0..w {
                let mut v = 0u8;
                for dy in 0..k {
                    for dx in 0..k {
                        v = v.max(diff[(((k * y + dy) * ours.width + k * x + dx) * 4) as usize]);
                    }
                }
                out.extend_from_slice(&[v, v, v]);
            }
        }
        save_rgb(&name("diff"), w, h, &out);
    }
}

fn save_rgb(path: &Path, w: u32, h: u32, rgb: &[u8]) {
    let Ok(file) = std::fs::File::create(path) else {
        return;
    };
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::High);
    let _ = enc
        .write_header()
        .and_then(|mut wr| wr.write_image_data(rgb));
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for k in 0..4 {
            if k <= c.len() {
                out.push(T[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
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

/// The dashboard page of the results in `dir`.
pub fn dashboard(dir: &Path, out: &Path) -> Result<usize, String> {
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
    for id in &ids {
        let status = std::fs::read_to_string(dir.join(format!("{id}.status"))).unwrap_or_default();
        let field = |k: &str| {
            status
                .lines()
                .find_map(|l| l.strip_prefix(k).and_then(|r| r.strip_prefix('\t')))
                .unwrap_or("")
                .to_owned()
        };
        let mut measures = Vec::new();
        for line in std::fs::read_to_string(dir.join(format!("{id}.tsv")))
            .unwrap_or_default()
            .lines()
        {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 7 {
                continue;
            }
            let num = |s: &str| s.parse::<f64>().unwrap_or(f64::NAN);
            let img = |what: &str| {
                let p = dir.join(format!("{id}~{}~{what}.png", f[1].replace(' ', "_")));
                std::fs::read(p).ok().map_or("null".to_owned(), |b| {
                    json_str(&format!("data:image/png;base64,{}", base64(&b)))
                })
            };
            measures.push(format!(
                "{{\"kind\":{},\"what\":{},\"native\":{},\"psnr\":{},\"off8\":{},\"off32\":{},\"within\":{},\"block\":{},\"ours\":{},\"theirs\":{},\"diff\":{}}}",
                json_str(f[0]),
                json_str(f[1]),
                f[2].parse::<u32>().unwrap_or(0),
                json_num(num(f[3])),
                json_num(num(f[4])),
                json_num(num(f[5])),
                f[6] == "true",
                json_num(num(f.get(7).copied().unwrap_or(""))),
                img("ours"),
                img("theirs"),
                img("diff"),
            ));
        }
        checks.push(format!(
            "{{\"id\":{},\"stage\":{},\"match\":{},\"frame\":{},\"group\":{},\"kind\":{},\"expect\":{},\"why\":{},\"result\":{},\"detail\":{},\"seconds\":{},\"measures\":[{}]}}",
            json_str(id),
            json_str(&field("stage")),
            json_str(&field("match")),
            json_str(&field("frame")),
            json_str(&field("group")),
            json_str(&field("kind")),
            json_str(&field("expect")),
            json_str(&field("why")),
            json_str(&field("result")),
            json_str(&field("detail")),
            json_str(&field("seconds")),
            measures.join(","),
        ));
    }
    let generated = std::process::Command::new("git")
        .args(["log", "-1", "--format=%h %s"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default();
    let data = format!(
        "{{\"commit\":{},\"generated\":{},\"tolerance\":{{\"psnr\":38,\"off32\":0.25}},\"scaled\":{{\"psnr\":{},\"block\":{}}},\"checks\":[{}]}}",
        json_str(&generated),
        json_str(&now()),
        crate::SCALED_PSNR,
        crate::SCALED_BLOCK,
        checks.join(",")
    );
    let page = include_str!("dashboard.html").replace("/*DATA*/null", &data);
    std::fs::write(out, page).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(checks.len())
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
