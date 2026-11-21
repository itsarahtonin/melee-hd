//! The subsystems behind their own lines (`docs/refactor.md`, task 13's rule): each one's state
//! struct keeps its fields private, and only its raw parts (`raw_parts`, `raw_parts_mut`), one
//! door per state, lay it open, for the heap's image in game code and for the oracle's tools,
//! leaf-check and tests outside it.

/// A subsystem behind its line.
struct Boundary {
    name: &'static str,
    /// Its sources (a file, or a folder ending in `/`), which reach its state freely.
    own: &'static [&'static str],
    /// Its state structs, `(file, name)`, whose fields stay private to it.
    states: &'static [(&'static str, &'static str)],
    /// The files outside it that may open its raw parts: the heap's image.
    readers: &'static [&'static str],
}

const BOUNDARIES: [Boundary; 2] = [
    Boundary {
        name: "the camera (task 13)",
        own: &["melee-sim/src/game/camera.rs"],
        states: &[("melee-sim/src/game/camera.rs", "CameraState")],
        // The heap's image of the camera's boxes and the disc's check of whose heap holds them.
        readers: &["melee-sim/src/game/export.rs", "melee-sim/src/game/disc.rs"],
    },
    Boundary {
        name: "collision (task 21)",
        own: &["melee-sim/src/mp/"],
        states: &[
            ("melee-sim/src/mp/mod.rs", "Geometry"),
            ("melee-sim/src/mp/coll/mod.rs", "CollStatics"),
        ],
        // The heap's image of the vertices, lines, islands and joints.
        readers: &["melee-sim/src/game/export.rs"],
    },
];

/// The crates game code lives in, from melee-sim's manifest.
fn crates() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Every `.rs` file under `dir`.
fn sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            sources(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// `text`'s lines before its first test module.
fn code(text: &str) -> &str {
    text.split("\n#[cfg(test)]").next().unwrap()
}

/// No game, presentation or player code outside a subsystem opens its raw parts but the heap's
/// image (each [`Boundary`]'s `readers`). The doors share one name, so a line is let through
/// where any subsystem it isn't in allows it. Test modules (from a file's first
/// `#[cfg(test)]`, and `*tests.rs` files) may, as the oracle's tools do.
#[test]
fn subsystems_are_reached_through_their_modules() {
    let crates = crates();
    let mut files = Vec::new();
    for c in ["melee-sim", "melee-render", "melee-player"] {
        sources(&crates.join(c).join("src"), &mut files);
    }
    assert!(
        files.len() > 400,
        "the crates' sources: {} files",
        files.len()
    );
    let mut found = Vec::new();
    for p in files {
        let rel = p
            .strip_prefix(&crates)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if rel.ends_with("tests.rs") {
            continue;
        }
        let inside = |b: &Boundary| {
            b.own.iter().any(|o| match o.strip_suffix('/') {
                Some(dir) => rel.starts_with(&format!("{dir}/")),
                None => rel == *o,
            })
        };
        if BOUNDARIES
            .iter()
            .any(|b| inside(b) || b.readers.contains(&rel.as_str()))
        {
            continue;
        }
        let text = std::fs::read_to_string(&p).unwrap();
        for (i, l) in code(&text).lines().enumerate() {
            if l.contains("raw_parts") && !l.trim_start().starts_with("//") {
                found.push(format!("{rel}:{}: {}", i + 1, l.trim()));
            }
        }
    }
    assert!(
        found.is_empty(),
        "a subsystem's raw parts read outside it:\n{}",
        found.join("\n")
    );
}

/// Each subsystem's state structs keep their fields private (or private to the subsystem,
/// `pub(in ...)`), so the compiler lists any reader that goes around its functions.
#[test]
fn the_subsystems_states_keep_their_fields_private() {
    let crates = crates();
    let mut open = Vec::new();
    for b in &BOUNDARIES {
        for &(file, name) in b.states {
            let text = std::fs::read_to_string(crates.join(file)).unwrap();
            let start = text
                .find(&format!("pub struct {name}"))
                .unwrap_or_else(|| panic!("{}: no `pub struct {name}` in {file}", b.name));
            let body = &text[start..];
            let body = &body[..body.find("\n}").expect("the struct's end")];
            for l in body.lines().skip(1) {
                let t = l.trim_start();
                if t.starts_with("pub ") {
                    open.push(format!("{}: {file}: {name}: {}", b.name, t));
                }
            }
        }
    }
    assert!(
        open.is_empty(),
        "a subsystem's state field is public:\n{}",
        open.join("\n")
    );
}
