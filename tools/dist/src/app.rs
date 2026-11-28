//! The macOS app bundle, `melee-hd.app`: what the Finder starts on a double-click and shows with
//! the player's icon.
//!
//! ```text
//! melee-hd.app/Contents/Info.plist              what macOS reads about the app
//! melee-hd.app/Contents/PkgInfo                 APPL????, for older Finders
//! melee-hd.app/Contents/MacOS/melee-hd          the player (one or both architectures)
//! melee-hd.app/Contents/Resources/melee-hd.icns the icon (crates/melee-player/icon/make.py)
//! ```
//!
//! It's signed ad hoc (`codesign --sign -`), as Apple Silicon requires of anything it runs:
//! the linker signs the executable ad hoc already, but joining two architectures and putting it
//! in a bundle wants the bundle signed as a whole, which seals the Info.plist and the icon with
//! it. Not with a Developer ID and not notarized ([milestone 7's design call
//! 5](../../../docs/milestone-7.md#design-calls)), so Gatekeeper asks once before the first
//! start (README.md, Build and play).

use std::path::{Path, PathBuf};
use std::process::Command;

/// The oldest macOS the bundle says it runs on (`LSMinimumSystemVersion`), which the build
/// targets too (`MACOSX_DEPLOYMENT_TARGET`): 11, Big Sur, the first with Apple Silicon and
/// Rust's floor for it.
pub const MACOS_MIN: &str = "11.0";

/// The app's identifier, from the repository's place on GitHub.
pub const IDENTIFIER: &str = "io.github.itsarahtonin.melee-hd";

/// The two architectures a universal build joins.
pub const UNIVERSAL: [&str; 2] = ["aarch64-apple-darwin", "x86_64-apple-darwin"];

/// The icon, as make.py draws it.
const ICNS: &[u8] = include_bytes!("../../../crates/melee-player/icon/melee-hd.icns");

/// The bundle's Info.plist, for melee-hd `version`.
pub fn info_plist(version: &str) -> String {
    let strings = [
        ("CFBundleDevelopmentRegion", "en"),
        ("CFBundleDisplayName", "melee-hd"),
        ("CFBundleExecutable", "melee-hd"),
        ("CFBundleIconFile", "melee-hd.icns"),
        ("CFBundleIdentifier", IDENTIFIER),
        ("CFBundleInfoDictionaryVersion", "6.0"),
        ("CFBundleName", "melee-hd"),
        ("CFBundlePackageType", "APPL"),
        ("CFBundleShortVersionString", version),
        ("CFBundleVersion", version),
        ("LSApplicationCategoryType", "public.app-category.games"),
        ("LSMinimumSystemVersion", MACOS_MIN),
    ];
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n<dict>\n",
    );
    for (key, value) in strings {
        s.push_str(&format!("\t<key>{key}</key>\n\t<string>{value}</string>\n"));
    }
    // Drawn at the display's full resolution, not scaled up from half of it.
    s.push_str("\t<key>NSHighResolutionCapable</key>\n\t<true/>\n</dict>\n</plist>\n");
    s
}

/// Makes `out/melee-hd.app` around the player `exe`, replacing one already there (macOS lets
/// a running program's files be replaced): its path.
pub fn bundle(exe: &Path, out: &Path, version: &str) -> Result<PathBuf, String> {
    let shown = |p: &Path| p.display().to_string();
    let app = out.join("melee-hd.app");
    let new = out.join("melee-hd.app.new");
    let _ = std::fs::remove_dir_all(&new);
    let contents = new.join("Contents");
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");
    for d in [&macos, &resources] {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", shown(d)))?;
    }
    let write = |path: PathBuf, bytes: &[u8]| {
        std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", shown(&path)))
    };
    write(contents.join("Info.plist"), info_plist(version).as_bytes())?;
    write(contents.join("PkgInfo"), b"APPL????")?;
    write(resources.join("melee-hd.icns"), ICNS)?;
    let player = macos.join("melee-hd");
    std::fs::copy(exe, &player).map_err(|e| format!("copying to {}: {e}", shown(&player)))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&player, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("{}: {e}", shown(&player)))?;
    }
    if app.exists() {
        std::fs::remove_dir_all(&app).map_err(|e| format!("{}: {e}", shown(&app)))?;
    }
    std::fs::rename(&new, &app).map_err(|e| format!("{}: {e}", shown(&app)))?;
    Ok(app)
}

/// Runs a macOS tool, failing on its failure.
fn tool(name: &str, args: &[&std::ffi::OsStr]) -> Result<(), String> {
    let status = Command::new(name)
        .args(args)
        .status()
        .map_err(|e| format!("{name}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{name} failed ({status})"))
    }
}

/// Signs the bundle ad hoc, sealing its Info.plist and icon with the executable.
pub fn seal(app: &Path) -> Result<(), String> {
    tool(
        "codesign",
        &[
            "--force".as_ref(),
            "--sign".as_ref(),
            "-".as_ref(),
            "--timestamp=none".as_ref(),
            app.as_os_str(),
        ],
    )
}

/// Joins one executable per architecture into one universal executable, `out`.
pub fn lipo(parts: &[PathBuf], out: &Path) -> Result<(), String> {
    let mut args: Vec<&std::ffi::OsStr> = vec!["-create".as_ref(), "-output".as_ref()];
    args.push(out.as_os_str());
    args.extend(parts.iter().map(|p| p.as_os_str()));
    tool("lipo", &args)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `<string>` (or `<true/>`, as "true") each key of a flat plist dictionary holds.
    fn keys(plist: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut rest = plist;
        while let Some(at) = rest.find("<key>") {
            rest = &rest[at + 5..];
            let end = rest.find("</key>").expect("a closed key");
            let key = rest[..end].to_owned();
            rest = rest[end + 6..].trim_start();
            let value = if let Some(v) = rest.strip_prefix("<string>") {
                let end = v.find("</string>").expect("a closed string");
                v[..end].to_owned()
            } else if rest.starts_with("<true/>") {
                "true".to_owned()
            } else {
                panic!("{key}'s value isn't a string or true");
            };
            out.push((key, value));
        }
        out
    }

    /// The icns's elements: each type and its data.
    fn elements(icns: &[u8]) -> Vec<(String, &[u8])> {
        assert_eq!(&icns[..4], b"icns");
        let len = |b: &[u8]| u32::from_be_bytes(b[4..8].try_into().unwrap()) as usize;
        assert_eq!(len(icns), icns.len(), "the icns's length");
        let mut out = Vec::new();
        let mut at = 8;
        while at < icns.len() {
            let e = &icns[at..];
            let n = len(e);
            assert!(n >= 8 && at + n <= icns.len(), "an element inside the file");
            out.push((String::from_utf8_lossy(&e[..4]).into_owned(), &e[8..n]));
            at += n;
        }
        out
    }

    /// A PNG's width and height.
    fn png_size(png: &[u8]) -> (u32, u32) {
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let at = |i: usize| u32::from_be_bytes(png[i..i + 4].try_into().unwrap());
        (at(16), at(20))
    }

    #[test]
    fn the_icon_has_every_size_macos_asks_for() {
        let sizes: Vec<(String, u32)> = elements(ICNS)
            .into_iter()
            .map(|(kind, png)| {
                let (w, h) = png_size(png);
                assert_eq!(w, h, "{kind} is square");
                (kind, w)
            })
            .collect();
        let want = [
            ("icp4", 16),
            ("icp5", 32),
            ("ic11", 32),
            ("ic12", 64),
            ("ic07", 128),
            ("ic13", 256),
            ("ic08", 256),
            ("ic14", 512),
            ("ic09", 512),
            ("ic10", 1024),
        ];
        let want: Vec<(String, u32)> = want.iter().map(|&(k, s)| (k.to_owned(), s)).collect();
        assert_eq!(sizes, want);
    }

    #[test]
    fn makes_the_bundle() {
        let dir = std::env::temp_dir().join(format!("melee-hd-app-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("built");
        std::fs::write(&exe, "player").unwrap();
        // A bundle already there is replaced whole.
        let stale = dir.join("melee-hd.app").join("Contents").join("stale");
        std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
        std::fs::write(&stale, "old").unwrap();
        let app = bundle(&exe, &dir, "1.2.3").unwrap();
        assert_eq!(app, dir.join("melee-hd.app"));
        assert!(!stale.exists());
        assert!(!dir.join("melee-hd.app.new").exists());
        let contents = app.join("Contents");

        let plist = std::fs::read_to_string(contents.join("Info.plist")).unwrap();
        assert!(plist.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist"));
        assert!(plist.ends_with("</dict>\n</plist>\n"));
        let keys = keys(&plist);
        let get = |k: &str| {
            keys.iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.as_str())
                .unwrap_or_else(|| panic!("Info.plist has no {k}"))
        };
        assert_eq!(get("CFBundlePackageType"), "APPL");
        assert_eq!(get("CFBundleIdentifier"), IDENTIFIER);
        assert_eq!(get("CFBundleShortVersionString"), "1.2.3");
        assert_eq!(get("CFBundleVersion"), "1.2.3");
        assert_eq!(get("CFBundleInfoDictionaryVersion"), "6.0");
        assert_eq!(get("LSMinimumSystemVersion"), MACOS_MIN);
        assert_eq!(get("NSHighResolutionCapable"), "true");
        // The executable and the icon it names are where macOS looks for them.
        let player = contents.join("MacOS").join(get("CFBundleExecutable"));
        assert_eq!(std::fs::read_to_string(&player).unwrap(), "player");
        let icon = contents.join("Resources").join(get("CFBundleIconFile"));
        assert_eq!(std::fs::read(&icon).unwrap(), ICNS);
        assert_eq!(
            std::fs::read(contents.join("PkgInfo")).unwrap(),
            b"APPL????"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&player).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o755, "the player is executable");
        }
        // macOS's own reader takes it.
        #[cfg(target_os = "macos")]
        tool(
            "plutil",
            &["-lint".as_ref(), contents.join("Info.plist").as_os_str()],
        )
        .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
