//! Checks the tracked files of the checkout it is run in, as CI does:
//!
//! ```text
//! cargo run -p path-guard
//! ```

use std::process::ExitCode;

fn main() -> ExitCode {
    let here = std::env::current_dir().expect("current directory");
    let Some(root) = path_guard::repo_root(&here) else {
        eprintln!("path-guard: not in a git checkout, nothing to check");
        return ExitCode::SUCCESS;
    };
    let result = std::fs::read_to_string(root.join(path_guard::CONFIG))
        .map_err(|e| format!("{}: {e}", path_guard::CONFIG))
        .and_then(|text| path_guard::Config::parse(&text))
        .and_then(|config| path_guard::check(&root, &config, &path_guard::names(&root)));
    match result {
        Ok(found) if found.is_empty() => ExitCode::SUCCESS,
        Ok(found) => {
            for line in path_guard::report(&found) {
                eprintln!("  {line}");
            }
            eprintln!(
                "path-guard: {} finding(s): personal paths or names, build output or oversized files (rules: {})",
                found.len(),
                path_guard::CONFIG
            );
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("path-guard: {e}");
            ExitCode::from(2)
        }
    }
}
