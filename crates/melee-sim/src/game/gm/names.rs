//! The characters' names as the game shows them (`gm_1601.c`): Shift-JIS strings in the
//! executable, the English table spelling them in full-width letters.

use super::super::state::Game;

/// `CKind_Zelda` and `CKind_Seak`.
const CKIND_ZELDA: u8 = 0x12;
const CKIND_SEAK: u8 = 0x13;

impl Game {
    /// The name of the character a slot plays, as the saved language shows it, or none for
    /// an empty slot, `gm_80160A60`: Zelda's or Sheik's as the slot's fighter is now.
    #[doc(alias = "gm_80160A60")]
    pub fn player_name(&self, slot: i32) -> Option<&'static [u8]> {
        if self.player_pkind(slot) == super::super::pl::player::PKIND_NONE {
            return None;
        }
        let mut ckind = self.player_ckind(slot) as u8;
        if ckind == CKIND_ZELDA || ckind == CKIND_SEAK {
            ckind = if self.player_fighter_kind(slot) == super::super::ft::seak::KIND_SEAK {
                CKIND_SEAK
            } else {
                CKIND_ZELDA
            };
        }
        let table = if self.gm.unlocks.is_us() {
            &NAMES_US
        } else {
            &NAMES_JP
        };
        *table
            .get(usize::from(ckind))
            .unwrap_or_else(|| panic!("character kind {ckind} reads past the names' table"))
    }
}

/// `lbl_803D4FDC`, by character kind: the names the English game shows.
pub const NAMES_US: [Option<&[u8]>; 33] = [
    Some(b"\x82b\x81D \x82e\x82\x81\x82\x8c\x82\x83\x82\x8f\x82\x8e"),
    Some(b"\x82c\x82j"),
    Some(b"\x82e\x82\x8f\x82\x98"),
    Some(b"\x82l\x82\x92\x81D \x82f\x82\x81\x82\x8d\x82\x85 \x81\x95 \x82v\x82\x81\x82\x94\x82\x83\x82\x88"),
    Some(b"\x82j\x82\x89\x82\x92\x82\x82\x82\x99"),
    Some(b"\x82a\x82\x8f\x82\x97\x82\x93\x82\x85\x82\x92"),
    Some(b"\x82k\x82\x89\x82\x8e\x82\x8b"),
    Some(b"\x82k\x82\x95\x82\x89\x82\x87\x82\x89"),
    Some(b"\x82l\x82\x81\x82\x92\x82\x89\x82\x8f"),
    Some(b"\x82l\x82\x81\x82\x92\x82\x94\x82\x88"),
    Some(b"\x82l\x82\x85\x82\x97\x82\x94\x82\x97\x82\x8f"),
    Some(b"\x82m\x82\x85\x82\x93\x82\x93"),
    Some(b"\x82o\x82\x85\x82\x81\x82\x83\x82\x88"),
    Some(b"\x82o\x82\x89\x82\x8b\x82\x81\x82\x83\x82\x88\x82\x95"),
    Some(b"\x82h\x82\x83\x82\x85 \x82b\x82\x8c\x82\x89\x82\x8d\x82\x82\x82\x85\x82\x92\x82\x93"),
    Some(b"\x82i\x82\x89\x82\x87\x82\x87\x82\x8c\x82\x99\x82\x90\x82\x95\x82\x86\x82\x86"),
    Some(b"\x82r\x82\x81\x82\x8d\x82\x95\x82\x93"),
    Some(b"\x82x\x82\x8f\x82\x93\x82\x88\x82\x89"),
    Some(b"\x82y\x82\x85\x82\x8c\x82\x84\x82\x81"),
    Some(b"\x82r\x82\x88\x82\x85\x82\x89\x82\x8b"),
    Some(b"\x82e\x82\x81\x82\x8c\x82\x83\x82\x8f"),
    Some(b"\x82x\x82\x8f\x82\x95\x82\x8e\x82\x87 \x82k\x82\x89\x82\x8e\x82\x8b"),
    Some(b"\x82c\x82\x92\x81D \x82l\x82\x81\x82\x92\x82\x89\x82\x8f"),
    Some(b"\x82q\x82\x8f\x82\x99"),
    Some(b"\x82o\x82\x89\x82\x83\x82\x88\x82\x95"),
    Some(b"\x82f\x82\x81\x82\x8e\x82\x8f\x82\x8e\x82\x84\x82\x8f\x82\x92\x82\x86"),
    Some(b"\x82l\x82\x81\x82\x93\x82\x94\x82\x85\x82\x92\x82\x88\x82\x81\x82\x8e\x82\x84"),
    Some(b"\x82y\x82`\x82j\x82n \x82\x8f\x82\x94\x82\x8f\x82\x8b\x82\x8f"),
    Some(b"\x82y\x82`\x82j\x82n \x82\x8f\x82\x8e\x82\x8e\x82\x81"),
    Some(b"\x82f\x81|\x82a\x82\x8f\x82\x97\x82\x93\x82\x85\x82\x92"),
    Some(b"\x82b\x82\x92\x82\x81\x82\x9a\x82\x99\x82\x88\x82\x81\x82\x8e\x82\x84"),
    Some(b"\x82r\x82\x81\x82\x8e\x82\x84\x82\x82\x82\x81\x82\x87"),
    None,
];

/// `lbl_803D4D74`, by character kind: the names the Japanese game shows.
pub const NAMES_JP: [Option<&[u8]>; 33] = [
    Some(b"\x82b\x81D\x83t\x83@\x83\x8b\x83R\x83\x93"),
    Some(b"\x83h\x83\x93\x83L\x81|\x83R\x83\x93\x83O"),
    Some(b"\x83t\x83H\x83b\x83N\x83X"),
    Some(b"\x82l\x82\x92\x81D\x83Q\x81|\x83\x80\x81\x95\x83E\x83H\x83b\x83`"),
    Some(b"\x83J\x81|\x83r\x83B"),
    Some(b"\x83N\x83b\x83p"),
    Some(b"\x83\x8a\x83\x93\x83N"),
    Some(b"\x83\x8b\x83C\x81|\x83W"),
    Some(b"\x83}\x83\x8a\x83I"),
    Some(b"\x83}\x83\x8b\x83X"),
    Some(b"\x83~\x83\x85\x83E\x83c\x81|"),
    Some(b"\x83l\x83X"),
    Some(b"\x83s\x81|\x83`"),
    Some(b"\x83s\x83J\x83`\x83\x85\x83E"),
    Some(b"\x83A\x83C\x83X\x83N\x83\x89\x83C\x83}\x81["),
    Some(b"\x83v\x83\x8a\x83\x93"),
    Some(b"\x83T\x83\x80\x83X"),
    Some(b"\x83\x88\x83b\x83V\x81|"),
    Some(b"\x83[\x83\x8b\x83_"),
    Some(b"\x83V\x81[\x83N"),
    Some(b"\x83t\x83@\x83\x8b\x83R"),
    Some(b"\x82\xb1\x82\xc7\x82\xe0\x83\x8a\x83\x93\x83N"),
    Some(b"\x83h\x83N\x83^\x81|\x83}\x83\x8a\x83I"),
    Some(b"\x83\x8d\x83C"),
    Some(b"\x83s\x83`\x83\x85\x81|"),
    Some(b"\x83K\x83m\x83\x93\x83h\x83\x8d\x83t"),
    Some(b"\x83}\x83X\x83^\x81[\x83n\x83\x93\x83h"),
    Some(b"\x93\xe4\x82\xcc\x83U\x83R\x93G\x81F\x92j"),
    Some(b"\x93\xe4\x82\xcc\x83U\x83R\x93G\x81F\x8f\x97"),
    Some(b"\x83M\x83K\x83N\x83b\x83p"),
    Some(b"\x83N\x83\x8c\x83C\x83W\x81[\x83n\x83\x93\x83h"),
    Some(b"\x83T\x83\x93\x83h\x83o\x83b\x83O\x8cN"),
    None,
];

impl Game {
    /// A character kind's name as the saved language shows it, `gm_80160980`.
    #[doc(alias = "gm_80160980")]
    pub fn character_name(&self, ckind: u8) -> Option<&'static [u8]> {
        let table = if self.gm.unlocks.is_us() {
            &NAMES_US
        } else {
            &NAMES_JP
        };
        *table
            .get(usize::from(ckind))
            .unwrap_or_else(|| panic!("character kind {ckind} reads past the names' table"))
    }

    /// Name tag `slot`'s name (`GetNameText`), none for an empty slot (`IsNameValid`: its
    /// first byte the terminator): its Shift-JIS bytes up to the terminator, which the
    /// name's 8 bytes leave to the byte after them.
    #[doc(alias = "GetNameText")]
    #[doc(alias = "IsNameValid")]
    pub fn name_text(&mut self, slot: u8) -> Option<Vec<u8>> {
        let t = self.game_data.name_tag(usize::from(slot));
        if t.namedata[0] == 0 {
            return None;
        }
        let mut out: Vec<u8> = t.namedata.iter().map(|&b| b as u8).collect();
        out.push(t.x1A0 as u8);
        let end = out
            .iter()
            .position(|&b| b == 0)
            .unwrap_or_else(|| panic!("name tag {slot}'s name runs past its terminator"));
        out.truncate(end);
        Some(out)
    }

    /// Whether every name tag slot has a name, `IsNameListFull` (`GetNumNameList`).
    // Ports `IsNameListFull`; aliased once a check reaches it.
    pub fn name_list_full(&mut self) -> bool {
        (0..120u8).filter(|&i| self.name_text(i).is_some()).count() >= 0x78
    }
}
