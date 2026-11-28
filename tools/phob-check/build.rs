//! Builds PhobGCC-SW's own source as the tests' oracle, when `PHOBGCC_SW` names a checkout of it at
//! the pinned commit (melee-phob's `UPSTREAM_COMMIT`). Without it the tests skip.
//!
//! Upstream's files are used as they are, with two mechanical changes the host's C++ compiler
//! needs: `curveFitting.h`'s variable-length arrays get fixed sizes (MSVC has none; the bounds
//! it indexes within are unchanged), and the functions needed from `phobGCC.h` and the RP2040
//! board's `readHardware.h` are copied out by name, since those files pull in the hardware.

use std::path::{Path, PathBuf};

/// melee-phob's `UPSTREAM_COMMIT`.
const COMMIT: &str = "b4f175eaff2578193a82ad9706f92bf1cd733312";

fn read(path: &Path) -> String {
    println!("cargo::rerun-if-changed={}", path.display());
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The text from the line starting with `start` through the first line after it that is `}` or
/// `};` alone: a function or an initializer at file level.
fn extract<'a>(text: &'a str, start: &str) -> &'a str {
    let begin = text
        .match_indices('\n')
        .map(|(i, _)| i + 1)
        .find(|&i| text[i..].starts_with(start))
        .unwrap_or_else(|| panic!("upstream has no line starting {start:?}"));
    let mut end = begin;
    for line in text[begin..].split_inclusive('\n') {
        end += line.len();
        if matches!(line.trim_end(), "}" | "};") {
            return &text[begin..end];
        }
    }
    panic!("{start:?} doesn't end");
}

/// The text from the line containing `from` through the line containing `to`.
fn between<'a>(text: &'a str, from: &str, to: &str) -> &'a str {
    let a = text.find(from).unwrap_or_else(|| panic!("no {from:?}"));
    let a = text[..a].rfind('\n').map_or(0, |i| i + 1);
    let b = a + text[a..].find(to).unwrap_or_else(|| panic!("no {to:?}"));
    let b = b + text[b..].find('\n').unwrap_or(text.len() - b) + 1;
    &text[a..b]
}

/// `text` with `from` replaced, which it must hold `count` times.
fn replace(text: &str, from: &str, to: &str, count: usize) -> String {
    assert_eq!(
        text.matches(from).count(),
        count,
        "curveFitting.h: {from:?}"
    );
    text.replace(from, to)
}

fn main() {
    println!("cargo::rustc-check-cfg=cfg(phob_oracle)");
    println!("cargo::rerun-if-env-changed=PHOBGCC_SW");
    println!("cargo::rerun-if-changed=oracle");
    let Some(root) = std::env::var_os("PHOBGCC_SW") else {
        return;
    };
    let root = PathBuf::from(root);
    let head = read(&root.join(".git/HEAD"));
    assert_eq!(
        head.trim(),
        COMMIT,
        "PHOBGCC_SW must be checked out at {COMMIT} (detached)"
    );
    let common = root.join("PhobGCC/common");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("phob");
    std::fs::create_dir_all(&out).unwrap();
    for f in ["structsAndEnums.h", "variables.h", "filter.h", "stick.h"] {
        std::fs::write(out.join(f), read(&common.join(f))).unwrap();
    }
    let fit = read(&common.join("curveFitting.h"));
    let fit = replace(&fit, "double *m[n];", "double *m[MAX_ORDER];", 1);
    let fit = replace(&fit, "double px[nPoints];", "double px[MAX_ORDER * 8];", 1);
    let fit = replace(
        &fit,
        "double masterMat[nCoeffs*nCoeffs];",
        "double masterMat[MAX_ORDER*MAX_ORDER];",
        2,
    );
    let fit = replace(
        &fit,
        "double mat[nCoeffs*nCoeffs];",
        "double mat[MAX_ORDER*MAX_ORDER];",
        2,
    );
    std::fs::write(out.join("curveFitting.h"), fit).unwrap();

    let phob = read(&common.join("phobGCC.h"));
    let board = read(&root.join("PhobGCC/rp2040/include/readHardware.h"));
    let mut shim = read(Path::new("oracle/head.cpp"));
    for start in [
        "ControlConfig _controls{",
        "FilterGains _gains {",
        "void copyButtons(",
        "void applyRemaps(",
        "void applyCalFromPoints(",
        "void calibrationSkipMeasurement(",
        "void calibrationUndo(",
        "void calibrationAdvance(",
        "void readSticks(",
    ] {
        shim += extract(&phob, start);
    }
    shim += extract(&board, "int readLa(");
    shim += extract(&board, "int readRa(");
    shim += "void phob_trigger_block(Buttons &btn, Buttons &hardware, ControlConfig &controls, Pins &pin) {\n";
    shim += "\tButtons tempBtn;\n\tcopyButtons(hardware, tempBtn);\n\tapplyRemaps(controls, hardware, tempBtn);\n";
    shim += between(
        &phob,
        "//We multiply the analog trigger reads",
        "copyButtons(tempBtn, btn);",
    );
    shim += "}\n";
    shim += &read(Path::new("oracle/api.cpp"));
    let shim_path = out.join("oracle.cpp");
    std::fs::write(&shim_path, shim).unwrap();

    let mut b = cc::Build::new();
    b.cpp(true)
        .file(&shim_path)
        .include(&out)
        // A reference: unoptimized, so nothing is folded or reassociated away from the source.
        .opt_level(0)
        .warnings(false);
    if b.get_compiler().is_like_msvc() {
        b.flag("/std:c++20").flag("/fp:precise");
    } else {
        b.flag("-std=c++20").flag("-ffp-contract=off");
    }
    b.compile("phob_oracle");
    println!("cargo::rustc-cfg=phob_oracle");
}
