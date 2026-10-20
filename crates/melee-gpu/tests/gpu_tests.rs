//! Every test in the workspace that draws gets its GPU from `melee_gpu::testing`, which skips on
//! a software adapter (WARP, which CI's Windows runners fall back to, draws gx.wgsl wrong, hangs
//! and crashes; the module has the details). This finds test code that asks wgpu for an
//! adapter, or makes a renderer or the player's GPU, any other way.

use std::path::{Path, PathBuf};

/// Ways to a GPU device other than `melee_gpu::testing`.
const WAYS: [&str; 6] = [
    "request_adapter(",
    "enumerate_adapters(",
    "initialize_adapter_from_env",
    "Renderer::new(",
    "Renderer::with_adapter(",
    // The player's `present::Gpu`.
    "Gpu::new()",
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if !name.starts_with('.') && name != "target" {
                rust_files(&path, out);
            }
        } else if name.ends_with(".rs") {
            out.push(path);
        }
    }
}

/// The test code in a file: all of it under a `tests` folder or in a `*tests.rs` module, else
/// from an inline `#[cfg(test)] mod NAME {` on (test modules come last).
fn test_code(path: &Path, text: &str) -> Option<String> {
    let whole = path.components().any(|c| c.as_os_str() == "tests")
        || path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().ends_with("tests.rs"));
    if whole {
        return Some(text.to_owned());
    }
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.windows(2).position(|w| {
        let next = w[1].trim();
        w[0].trim() == "#[cfg(test)]" && next.starts_with("mod ") && next.ends_with('{')
    })?;
    Some(lines[start..].join("\n"))
}

#[test]
fn gpu_tests_get_their_gpu_from_the_testing_module() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for dir in ["crates", "tools"] {
        rust_files(&root.join(dir), &mut files);
    }
    assert!(!files.is_empty(), "no sources under {}", root.display());
    let mut found = Vec::new();
    for path in &files {
        // The module itself, and this file's list.
        if path.ends_with("melee-gpu/src/testing.rs")
            || path.ends_with("melee-gpu/tests/gpu_tests.rs")
        {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let Some(code) = test_code(path, &text) else {
            continue;
        };
        for way in WAYS {
            if code.contains(way) {
                let shown = path.strip_prefix(&root).unwrap_or(path);
                found.push(format!("  {}: {way}", shown.display()));
            }
        }
    }
    assert!(
        found.is_empty(),
        "test code that gets a GPU without melee_gpu::testing (adapter() or renderer()), so it \
         would run on a software adapter:\n{}",
        found.join("\n")
    );
}
