//! Which build this is: its channel and commit, stamped in when it's compiled. The dist tool
//! sets them (`cargo run -p dist -- --channel stable|dev`, `MELEE_HD_CHANNEL` and
//! `MELEE_HD_COMMIT`): stable is `dist/`, built from main after a full verify, and dev is
//! `dist-dev/`, built from the staging branch after a smoke check. A plain `cargo build` is a
//! local build of no known commit. The window's title, the startup line and the captures name
//! the build; a dev build keeps its own settings and caches (`melee-hd-dev`), so nothing it
//! writes reaches a stable build's. None of it reaches a tick.

/// Where a build comes from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Channel {
    /// `dist/`: main after a full verify.
    Stable,
    /// `dist-dev/`: the staging branch after a smoke check.
    Dev,
    /// A plain `cargo build`, not stamped by the dist tool: behaves as stable.
    Local,
}

impl Channel {
    /// The channel a stamp names; a missing or unknown one is a local build.
    fn parse(stamp: Option<&str>) -> Channel {
        match stamp {
            Some("stable") => Channel::Stable,
            Some("dev") => Channel::Dev,
            _ => Channel::Local,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Channel::Stable => "stable",
            Channel::Dev => "dev",
            Channel::Local => "local",
        }
    }

    /// The name of the channel's settings and cache folders: the dev build's are its own.
    pub fn folder(self) -> &'static str {
        match self {
            Channel::Dev => "melee-hd-dev",
            Channel::Stable | Channel::Local => "melee-hd",
        }
    }
}

/// The commit a stamp names (the short hash, `-dirty` when the tree had changes), else
/// `unknown`.
fn parse_commit(stamp: Option<&str>) -> &str {
    stamp.filter(|c| !c.is_empty()).unwrap_or("unknown")
}

/// This build's channel.
pub fn channel() -> Channel {
    Channel::parse(option_env!("MELEE_HD_CHANNEL"))
}

/// This build's commit.
pub fn commit() -> &'static str {
    parse_commit(option_env!("MELEE_HD_COMMIT"))
}

/// The window's title: the dev build's says so with its commit, stable's keeps the commit
/// quieter, and a local build says it's local.
fn title_of(channel: Channel, commit: &str) -> String {
    match channel {
        Channel::Stable => format!("melee-hd ({commit})"),
        Channel::Dev | Channel::Local => format!("melee-hd {} {commit}", channel.name()),
    }
}

pub fn title() -> String {
    title_of(channel(), commit())
}

/// The build in a line, for the startup line and the captures: `melee-hd dev build 3cbfe08a`.
pub fn stamp() -> String {
    format!("melee-hd {} build {}", channel().name(), commit())
}

/// An environment of `vars`, for the folders' tests.
#[cfg(test)]
pub fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<std::ffi::OsString> {
    let vars: Vec<(String, String)> = vars
        .iter()
        .map(|&(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    move |name| vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unstamped_build_is_local_of_no_known_commit() {
        assert_eq!(Channel::parse(None), Channel::Local);
        assert_eq!(Channel::parse(Some("")), Channel::Local);
        assert_eq!(Channel::parse(Some("nightly")), Channel::Local);
        assert_eq!(Channel::parse(Some("stable")), Channel::Stable);
        assert_eq!(Channel::parse(Some("dev")), Channel::Dev);
        assert_eq!(parse_commit(None), "unknown");
        assert_eq!(parse_commit(Some("")), "unknown");
        assert_eq!(parse_commit(Some("3cbfe08a-dirty")), "3cbfe08a-dirty");
        // `cargo test` builds aren't stamped by the dist tool.
        if option_env!("MELEE_HD_CHANNEL").is_none() {
            assert_eq!(channel(), Channel::Local);
        }
    }

    #[test]
    fn titles_name_the_build() {
        assert_eq!(title_of(Channel::Dev, "3cbfe08a"), "melee-hd dev 3cbfe08a");
        assert_eq!(title_of(Channel::Stable, "3cbfe08a"), "melee-hd (3cbfe08a)");
        assert_eq!(
            title_of(Channel::Local, "unknown"),
            "melee-hd local unknown"
        );
    }

    #[test]
    fn only_dev_keeps_folders_of_its_own() {
        assert_eq!(Channel::Dev.folder(), "melee-hd-dev");
        assert_eq!(Channel::Stable.folder(), "melee-hd");
        assert_eq!(Channel::Local.folder(), "melee-hd");
    }
}
