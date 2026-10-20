// SPDX-License-Identifier: GPL-3.0-or-later

//! Writing a match's recording (`melee_sim::game::record`) as a `.slp` file: the UBJSON object
//! Slippi Dolphin writes, the raw event stream and then the metadata.

use std::time::{SystemTime, UNIX_EPOCH};

use melee_sim::game::record::{Recorder, Setup};

use crate::replay::{Replay, event};

/// What the metadata element holds.
#[derive(Clone, Debug)]
pub struct Metadata {
    /// When the match started (`startAt`, written in UTC).
    pub start_at: SystemTime,
    /// The last frame recorded (`lastFrame`).
    pub last_frame: i32,
    /// For each port with a player, the frames it was each internal character
    /// (`players.N.characters`).
    pub characters: [Vec<(u8, u32)>; 4],
    /// What played the match (`playedOn`).
    pub played_on: String,
}

impl Metadata {
    /// The metadata of a recording made by melee-hd, started at `start_at`.
    pub fn of(r: &Recorder, start_at: SystemTime) -> Metadata {
        Metadata {
            start_at,
            last_frame: r
                .last_frame()
                .unwrap_or(melee_sim::game::record::FIRST_FRAME - 1),
            characters: core::array::from_fn(|p| r.characters(p).to_vec()),
            played_on: "melee-hd".into(),
        }
    }
}

/// A `.slp` file: `raw`, a raw event stream, and the metadata.
pub fn slp(raw: &[u8], meta: &Metadata) -> Vec<u8> {
    let mut o = Vec::with_capacity(raw.len() + 512);
    o.push(b'{');
    key(&mut o, "raw");
    o.extend_from_slice(b"[$U#l");
    o.extend_from_slice(&(raw.len() as u32).to_be_bytes());
    o.extend_from_slice(raw);
    key(&mut o, "metadata");
    o.push(b'{');
    key(&mut o, "startAt");
    string(&mut o, &iso8601(meta.start_at));
    key(&mut o, "lastFrame");
    o.push(b'l');
    o.extend_from_slice(&meta.last_frame.to_be_bytes());
    key(&mut o, "players");
    o.push(b'{');
    for (port, chars) in meta.characters.iter().enumerate() {
        if chars.is_empty() {
            continue;
        }
        key(&mut o, &port.to_string());
        o.push(b'{');
        key(&mut o, "names");
        o.extend_from_slice(b"{}");
        key(&mut o, "characters");
        o.push(b'{');
        for &(kind, frames) in chars {
            key(&mut o, &kind.to_string());
            o.push(b'l');
            o.extend_from_slice(&(frames as i32).to_be_bytes());
        }
        o.extend_from_slice(b"}}");
    }
    o.push(b'}');
    key(&mut o, "playedOn");
    string(&mut o, &meta.played_on);
    o.extend_from_slice(b"}}");
    o
}

/// A recorder's `.slp` file, its match started at `start_at`.
pub fn recording(r: &Recorder, start_at: SystemTime) -> Vec<u8> {
    slp(r.raw(), &Metadata::of(r, start_at))
}

/// An object key: its length as a `uint8` and its bytes.
fn key(o: &mut Vec<u8>, k: &str) {
    o.push(b'U');
    o.push(k.len() as u8);
    o.extend_from_slice(k.as_bytes());
}

fn string(o: &mut Vec<u8>, s: &str) {
    o.push(b'S');
    key(o, s);
}

/// `YYYY-MM-DDTHH:MM:SSZ`, as Slippi writes `startAt`.
pub fn iso8601(t: SystemTime) -> String {
    let [y, mo, d, h, mi, s] = civil(t);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// A time's UTC date and time of day: year, month, day, hours, minutes, seconds.
pub fn civil(t: SystemTime) -> [u32; 6] {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Howard Hinnant's days-to-civil.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    [
        y as u32,
        m as u32,
        d as u32,
        (rem / 3600) as u32,
        (rem / 60 % 60) as u32,
        (rem % 60) as u32,
    ]
}

/// Slippi's file name for a match started at this date and time of day:
/// `Game_YYYYMMDDTHHMMSS.slp`.
pub fn file_name([y, mo, d, h, mi, s]: [u32; 6]) -> String {
    format!("Game_{y:04}{mo:02}{d:02}T{h:02}{mi:02}{s:02}.slp")
}

/// What a recording of a replay's match reads from outside it, from the replay: the code list
/// it ran, its players' nametags, the PAL and frozen Pokémon Stadium toggles and the language.
pub fn setup(r: &Replay) -> Setup {
    let language = r
        .events
        .iter()
        .find(|e| e.command == event::GAME_START)
        .and_then(|e| e.payload.get(LANGUAGE).copied())
        .unwrap_or(1);
    Setup {
        codes: r.gecko_codes.clone(),
        nametags: r.nametags,
        pal: r.is_pal != 0,
        frozen_ps: r.is_frozen_ps != 0,
        language,
    }
}

/// Where the game start's payload holds the language.
const LANGUAGE: usize = 0x2BC;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn dates() {
        let t = UNIX_EPOCH + Duration::from_secs(1_529_653_979);
        assert_eq!(iso8601(t), "2018-06-22T07:52:59Z");
        assert_eq!(iso8601(UNIX_EPOCH), "1970-01-01T00:00:00Z");
        let leap = UNIX_EPOCH + Duration::from_secs(951_782_400);
        assert_eq!(iso8601(leap), "2000-02-29T00:00:00Z");
        assert_eq!(file_name(civil(t)), "Game_20180622T075259.slp");
    }

    #[test]
    fn file_reads_back() {
        let raw = [event::PAYLOAD_SIZES, 4, event::GAME_END, 0, 6];
        let meta = Metadata {
            start_at: UNIX_EPOCH,
            last_frame: 100,
            characters: [vec![(1, 224)], vec![], vec![(18, 3), (19, 221)], vec![]],
            played_on: "melee-hd".into(),
        };
        let file = slp(&raw, &meta);
        assert_eq!(crate::replay::raw_events(&file).unwrap(), raw);
        let tail = &file[15 + raw.len()..];
        assert!(tail.starts_with(b"U\x08metadata{U\x07startAtSU\x141970-01-01T00:00:00Z"));
        assert!(tail.ends_with(b"U\x08playedOnSU\x08melee-hd}}"));
    }
}
