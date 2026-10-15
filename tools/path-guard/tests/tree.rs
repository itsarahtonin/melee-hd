//! The repository's tracked files pass the check, and the check catches what it should in a
//! scratch repository. The personal paths below are built at run time, so this file's own text
//! doesn't match.

use path_guard::{Config, Finding, check, names, repo_root};
use std::path::{Path, PathBuf};
use std::process::Command;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn config(root: &Path) -> Config {
    let text = std::fs::read_to_string(root.join(path_guard::CONFIG)).expect(path_guard::CONFIG);
    Config::parse(&text).unwrap()
}

#[test]
fn the_tracked_files_are_clean() {
    let Some(root) = repo_root(&repository()) else {
        eprintln!("skipping: not a git checkout (or no git), so there are no tracked files");
        return;
    };
    let found = check(&root, &config(&root), &names(&root)).unwrap();
    let list: Vec<String> = path_guard::report(&found)
        .iter()
        .map(|l| format!("  {l}"))
        .collect();
    assert!(
        found.is_empty(),
        "personal paths or names, build output or oversized files (rules: {}):\n{}",
        path_guard::CONFIG,
        list.join("\n")
    );
}

fn git(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .is_ok_and(|o| o.status.success())
}

#[test]
fn catches_paths_names_build_output_and_size() {
    let Some(root) = repo_root(&repository()) else {
        eprintln!("skipping: not a git checkout (or no git)");
        return;
    };
    let mut config = config(&root);
    config.max_size = 1000;
    config.large = vec!["big-but-fine.bin".into()];

    let dir = std::env::temp_dir().join(format!("path-guard-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert!(git(&dir, &["init", "-q", "."]), "git init");

    let b = '\\';
    let files: Vec<(&str, Vec<u8>)> = vec![
        (
            "notes.md",
            format!(
                "Built in C:{b}Users{b}alice{b}src, cached under {}/.cargo.\n\
                 Placeholders are fine: C:/Users/USER/melee-hd, C:{b}Users{b}<name>, C:{b}Users{b}Public{b}x, C:{b}Users{b}USER.\n\
                 So is prose: Users' rights, a home/ folder, OneDrive itself.\n",
                concat!("/ho", "me/bob")
            )
            .into_bytes(),
        ),
        (
            "src/lib.rs",
            format!("const P: &str = \"C:{b}{b}Users{b}{b}carol{b}{b}x\";\n").into_bytes(),
        ),
        (
            "app.exe",
            [
                &[0u8, 1, 2, 0xff][..],
                concat!("C:", "\\Us", "ers\\dave\\.cargo\\registry\\src").as_bytes(),
                &[0, 0xfe],
            ]
            .concat(),
        ),
        (
            "sync.txt",
            format!("D:{b}OneDrive - Contoso{b}docs and E:/OneDrive{}x\n", '/').into_bytes(),
        ),
        (
            "scratch.txt",
            format!("C:{b}AppData{b}Local{b}Temp{b}claude{b}C--Projects\n").into_bytes(),
        ),
        ("hello.txt", b"Hello ZorBlax, how are you\n".to_vec()),
        ("dist/melee-hd.exe", b"x".to_vec()),
        ("target/release/x", b"x".to_vec()),
        ("tools/dist/main.rs", b"fine\n".to_vec()),
        ("big.bin", vec![b'a'; 1001]),
        ("big-but-fine.bin", vec![b'a'; 1001]),
    ];
    for (path, content) in &files {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    assert!(
        git(&dir, &["-c", "core.autocrlf=false", "add", "-A"]),
        "git add"
    );

    let found = check(&dir, &config, &["zorblax".into()]).unwrap();
    let mut contents: Vec<(String, String)> = Vec::new();
    let mut rest = Vec::new();
    for f in found {
        match f {
            Finding::Content { path, text, .. } => contents.push((path, text)),
            other => rest.push(other),
        }
    }
    contents.sort();
    let pair = |p: &str, t: String| (p.to_owned(), t);
    let mut expected = vec![
        pair("app.exe", format!("{b}Users{b}dave")),
        pair("hello.txt", "ZorBlax".into()),
        pair("notes.md", format!("{b}Users{b}alice")),
        pair("notes.md", concat!("/ho", "me/bob").into()),
        pair("scratch.txt", format!("AppData{b}Local{b}Temp{b}claude")),
        pair("src/lib.rs", format!("{b}Users{b}{b}carol")),
        pair("sync.txt", format!("OneDrive - Contoso{b}")),
        pair("sync.txt", concat!("OneDrive", "/").into()),
    ];
    expected.sort();
    assert_eq!(contents, expected);
    assert_eq!(
        rest,
        vec![
            Finding::Forbidden {
                path: "dist/melee-hd.exe".into(),
                dir: "dist/".into()
            },
            Finding::Forbidden {
                path: "target/release/x".into(),
                dir: "target/".into()
            },
            Finding::Large {
                path: "big.bin".into(),
                size: 1001,
                max: 1000
            },
        ]
    );
    let _ = std::fs::remove_dir_all(&dir);
}
