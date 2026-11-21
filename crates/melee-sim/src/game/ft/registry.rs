//! The character registry: one [`Character`] entry per fighter kind ([`FighterKind`], the
//! index of the executable's per-kind tables), holding everything that names the character:
//! its name, its tables in the executable (its files, costumes and hooks' addresses,
//! `kind_tables`; the callbacks the common code calls by kind, `kind_callbacks`), the ports of
//! its hooks (the fighters' first setup, creation, spawn and attribute change), its module's
//! callbacks for the callback table, its attributes' size, and its roster line (the character
//! kind a match takes it as and the command line's names, [`roster`](super::super::roster)).
//!
//! The game numbers its characters twice, and both numbers are the game's own, so both are
//! enums here with those numbers: a [`FighterKind`] says which fighter's code and files run
//! (`Fighter::kind`, 33 of them: the Ice Climbers are two, Popo and Nana; Zelda and Sheik two,
//! each its own kind), a [`CharacterKind`] which character a player chose (the character
//! select's, the rules', the replays', the save's number, 33 of them: the 26 the select screen
//! offers, then the bosses, the wireframes, the Sandbag and Popo alone). The game's
//! `ftMapping_list` takes a character kind to its fighter kinds (`pl::player::ckind_ftkind`).
//! One type can't hold both: Fox is 1 in one and 2 in the other.
//!
//! A new character's port writes its module (its hooks and its `#[callbacks]` dispatch) and
//! fills its entry here; nothing else lists characters. The state keeps the game's numbers
//! (`Fighter::kind`, the players' character kinds), so it and the oracle are unchanged; code
//! reads them through the enums (`fp.kind == FighterKind::Fox`, `match fp.fighter_kind() {
//! FighterKind::Link | FighterKind::Clink => .. }`, `CharacterKind::Fox.id()`).

use super::super::code::{Entries, kind};
use super::super::state::Running;
use super::co::kind_callbacks::{self, KindCallbacks};
use super::create_model::Creator;
use super::data::KindTables;
use super::kind_tables;
use super::kirby::copy::{KirbyCopy, NO_COPY};
use super::vars;
use super::{
    captain, clink, crazyhand, donkey, drmario, emblem, falco, fox, gamewatch, gkoopa, kinds,
    kirby, koopa, link, luigi, mario, mars, masterhand, mewtwo, ness, peach, pichu, pikachu, popo,
    purin, samus, sandbag, seak, yoshi, zako, zelda,
};
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;

/// Declares a kind enum with the game's numbers: `ALL`, `COUNT`, `from_id`, `id`, and
/// comparisons with the numbers the state holds.
macro_rules! kind_enum {
    ($(#[$m:meta])* $name:ident: $($(#[$vm:meta])* $v:ident = $n:expr,)*) => {
        $(#[$m])*
        #[repr(u8)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name {
            $($(#[$vm])* $v = $n,)*
        }

        impl $name {
            /// Every kind, by number.
            pub const ALL: [$name; [$($name::$v),*].len()] = [$($name::$v),*];

            /// How many there are.
            pub const COUNT: usize = Self::ALL.len();

            /// The kind numbered `id`, if the game has one.
            pub const fn from_id(id: i32) -> Option<$name> {
                if id >= 0 && (id as usize) < Self::COUNT {
                    Some(Self::ALL[id as usize])
                } else {
                    None
                }
            }

            /// Its number, as the state and the game's tables hold it.
            pub const fn id(self) -> i32 {
                self as i32
            }

            /// Its number as an index into the game's tables by kind.
            pub const fn index(self) -> usize {
                self as usize
            }
        }

        impl PartialEq<$name> for i32 {
            fn eq(&self, k: &$name) -> bool {
                *self == k.id()
            }
        }

        impl PartialEq<i32> for $name {
            fn eq(&self, id: &i32) -> bool {
                self.id() == *id
            }
        }

        impl PartialEq<$name> for u8 {
            fn eq(&self, k: &$name) -> bool {
                i32::from(*self) == k.id()
            }
        }

        impl PartialEq<$name> for i8 {
            fn eq(&self, k: &$name) -> bool {
                i32::from(*self) == k.id()
            }
        }
    };
}
pub(crate) use kind_enum;

kind_enum! {
    /// A fighter kind (`FighterKind`, `Ft_Kind_*`): which fighter's code and files run, the
    /// index of the executable's per-kind tables and of this registry ([`CHARACTERS`]). The
    /// game's own numbers.
    FighterKind:
    /// `Ft_Kind_Mario`.
    Mario = 0,
    /// `Ft_Kind_Fox`.
    Fox = 1,
    /// `Ft_Kind_Captain`: Captain Falcon.
    Captain = 2,
    /// `Ft_Kind_Donkey`: Donkey Kong.
    Donkey = 3,
    /// `Ft_Kind_Kirby`.
    Kirby = 4,
    /// `Ft_Kind_Koopa`: Bowser.
    Koopa = 5,
    /// `Ft_Kind_Link`.
    Link = 6,
    /// `Ft_Kind_Seak`: Sheik.
    Seak = 7,
    /// `Ft_Kind_Ness`.
    Ness = 8,
    /// `Ft_Kind_Peach`.
    Peach = 9,
    /// `Ft_Kind_Popo`: the Ice Climbers' leader.
    Popo = 10,
    /// `Ft_Kind_Nana`: the Ice Climbers' partner.
    Nana = 11,
    /// `Ft_Kind_Pikachu`.
    Pikachu = 12,
    /// `Ft_Kind_Samus`.
    Samus = 13,
    /// `Ft_Kind_Yoshi`.
    Yoshi = 14,
    /// `Ft_Kind_Purin`: Jigglypuff.
    Purin = 15,
    /// `Ft_Kind_Mewtwo`.
    Mewtwo = 16,
    /// `Ft_Kind_Luigi`.
    Luigi = 17,
    /// `Ft_Kind_Mars`: Marth.
    Mars = 18,
    /// `Ft_Kind_Zelda`.
    Zelda = 19,
    /// `Ft_Kind_CLink`: Young Link.
    Clink = 20,
    /// `Ft_Kind_DrMario`.
    DrMario = 21,
    /// `Ft_Kind_Falco`.
    Falco = 22,
    /// `Ft_Kind_Pichu`.
    Pichu = 23,
    /// `Ft_Kind_GameWatch`: Mr. Game & Watch.
    GameWatch = 24,
    /// `Ft_Kind_Ganon`: Ganondorf.
    Ganon = 25,
    /// `Ft_Kind_Emblem`: Roy.
    Emblem = 26,
    /// `Ft_Kind_MasterH`: Master Hand.
    MasterHand = 27,
    /// `Ft_Kind_CrezyH`: Crazy Hand.
    CrazyHand = 28,
    /// `Ft_Kind_Boy`: the male wireframe.
    Boy = 29,
    /// `Ft_Kind_Girl`: the female wireframe.
    Girl = 30,
    /// `Ft_Kind_GKoops`: Giga Bowser.
    GKoopa = 31,
    /// `Ft_Kind_Sandbag`.
    Sandbag = 32,
}

kind_enum! {
    /// A character kind (`CharacterKind`, `CKind_*`): which character a player chose, as the
    /// character select, the rules, the replays and the save number it; the game's
    /// `ftMapping_list` takes it to its [`FighterKind`]s. The game's own numbers.
    CharacterKind:
    /// `CKind_Captain`: Captain Falcon.
    Captain = 0,
    /// `CKind_Donkey`: Donkey Kong.
    Donkey = 1,
    /// `CKind_Fox`.
    Fox = 2,
    /// `CKind_GameWatch`: Mr. Game & Watch.
    GameWatch = 3,
    /// `CKind_Kirby`.
    Kirby = 4,
    /// `CKind_Koopa`: Bowser.
    Koopa = 5,
    /// `CKind_Link`.
    Link = 6,
    /// `CKind_Luigi`.
    Luigi = 7,
    /// `CKind_Mario`.
    Mario = 8,
    /// `CKind_Mars`: Marth.
    Mars = 9,
    /// `CKind_Mewtwo`.
    Mewtwo = 10,
    /// `CKind_Ness`.
    Ness = 11,
    /// `CKind_Peach`.
    Peach = 12,
    /// `CKind_Pikachu`.
    Pikachu = 13,
    /// `CKind_PopoNana`: the Ice Climbers.
    IceClimbers = 14,
    /// `CKind_Purin`: Jigglypuff.
    Purin = 15,
    /// `CKind_Samus`.
    Samus = 16,
    /// `CKind_Yoshi`.
    Yoshi = 17,
    /// `CKind_Zelda`: Zelda, who changes into Sheik.
    Zelda = 18,
    /// `CKind_Seak`: Sheik, who changes into Zelda.
    Seak = 19,
    /// `CKind_Falco`.
    Falco = 20,
    /// `CKind_CLink`: Young Link.
    Clink = 21,
    /// `CKind_DrMario`.
    DrMario = 22,
    /// `CKind_Emblem`: Roy.
    Emblem = 23,
    /// `CKind_Pichu`.
    Pichu = 24,
    /// `CKind_Ganon`: Ganondorf.
    Ganon = 25,
    /// `CKind_MasterH`: Master Hand.
    MasterHand = 26,
    /// `CKind_Boy`: the male wireframe.
    Boy = 27,
    /// `CKind_Girl`: the female wireframe.
    Girl = 28,
    /// `CKind_GKoops`: Giga Bowser.
    GKoopa = 29,
    /// `CKind_CrezyH`: Crazy Hand.
    CrazyHand = 30,
    /// `ChKind_Sandbag`.
    Sandbag = 31,
    /// `ChKind_Popo`: Popo alone.
    Popo = 32,
}

impl FighterKind {
    /// Its registry entry.
    pub fn character(self) -> &'static Character {
        character(self.id())
    }
}

impl CharacterKind {
    /// How many the character select offers (`CKind_Playable_Count`): the kinds before
    /// Master Hand.
    pub const PLAYABLE: usize = 26;

    /// `ChKind_None`: no character.
    pub const NONE: u8 = 33;

    /// Its number as a byte, as the rules and the replays hold it.
    pub const fn byte(self) -> u8 {
        self as u8
    }
}

impl super::super::structs::Fighter {
    /// The fighter's kind (`kind`), which the game only ever sets to one of its own.
    pub fn fighter_kind(&self) -> FighterKind {
        FighterKind::from_id(self.kind)
            .unwrap_or_else(|| panic!("Fighter::kind {} is no fighter kind", self.kind))
    }
}

impl KindCallbacks {
    /// Its specials' state changes: neutral, side, up and down, on the ground then in the air.
    pub fn specials(&self) -> [u32; 8] {
        [
            self.special_n,
            self.special_s,
            self.special_hi,
            self.special_lw,
            self.special_air_n,
            self.special_air_s,
            self.special_air_hi,
            self.special_air_lw,
        ]
    }
}

/// A fighter's hook taking its GObj (`void (*)(HSD_GObj*)`).
pub type Hook = fn(&mut Running<'_>, GObjId);

/// One character, by its fighter kind.
#[derive(Clone, Copy)]
pub struct Character {
    pub kind: FighterKind,
    /// Its name (the roster's, which the player shows, for a roster character).
    pub name: &'static str,
    /// Its tables in the executable: its files, costumes, animations, action states and its
    /// hooks' addresses (`ftData_OnLoad` and the others).
    pub tables: &'static KindTables,
    /// The callbacks the common code calls by its kind (`ftData_SpecialN` and the others).
    pub callbacks: &'static KindCallbacks,
    /// The port of `tables.on_first_init` (`ftData_Table_Unk1`), which the fighters' first
    /// setup runs for every kind; none where the table has none.
    pub first_init: Option<fn(&mut Running<'_>)>,
    /// The ports of `tables`' hooks: `on_load` (creation's, from the fighter's creation or a
    /// demo's), `on_death` (a spawn's) and `load_special_attrs` (an attribute change's).
    pub on_load: fn(&mut Running<'_>, GObjId, Creator),
    pub on_death: Hook,
    pub load_special_attrs: Hook,
    /// Its module's `#[callbacks(Fighter)]` dispatch: its action states' callbacks and hooks.
    /// One module's for kinds that share one (Popo's for Nana, the male wireframe's for the
    /// female, Captain Falcon's for Ganondorf).
    pub fighter: Option<fn(&mut Entries<kind::Fighter>)>,
    /// Its module's `#[callbacks(Call)]` dispatch: its functions other code calls by address.
    pub calls: Option<fn(&mut Entries<kind::Call>)>,
    /// How many bytes of its attributes (`ftData::ext_attr`) its hooks copy.
    pub attrs_size: usize,
    /// Kirby's copy of it: his neutral special, hat, cleanups and items while he has it.
    pub kirby_copy: KirbyCopy,
    /// The words of its variables (`Fighter::u`) that keep GObjs (its items, Fox's blaster,
    /// Sheik's chain), which the importer types as handles (`vars::gobj_words`).
    pub item_words: &'static [usize],
    /// The character kind a match takes it as, for a character the roster lists: the player's
    /// command line and launcher offer it once the fighter audit finds nothing missing.
    pub vs: Option<CharacterKind>,
    /// The names the command line takes, the first the usual one (a roster character's).
    pub keys: &'static [&'static str],
}

impl std::fmt::Debug for Character {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Character")
            .field("kind", &self.kind)
            .field("name", &self.name)
            .field("vs", &self.vs)
            .field("keys", &self.keys)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Character {
    fn eq(&self, o: &Self) -> bool {
        self.kind == o.kind
    }
}

impl Character {
    /// The character kind a match takes it as, as `Player::character` holds it: a roster
    /// character's.
    pub fn vs_kind(&self) -> u8 {
        self.vs
            .unwrap_or_else(|| panic!("{} isn't a roster character", self.name))
            .byte()
    }
}

/// The character of fighter kind `kind`; stops if there's none.
pub fn character(kind: i32) -> &'static Character {
    match FighterKind::from_id(kind).and_then(|k| CHARACTERS[k.index()].as_ref()) {
        Some(c) => c,
        None => unimplemented!("fighter kind {kind} isn't ported"),
    }
}

/// The ported characters, by fighter kind.
pub fn ported() -> impl Iterator<Item = &'static Character> {
    CHARACTERS.iter().flatten()
}

/// The characters' entries in the callback table: each module's fighter callbacks and calls.
pub fn register(fighter: &mut Entries<kind::Fighter>, calls: &mut Entries<kind::Call>) {
    for c in ported() {
        if let Some(f) = c.fighter {
            f(fighter);
        }
        if let Some(f) = c.calls {
            f(calls);
        }
    }
}

/// Every character, by fighter kind; none where the character isn't ported.
pub static CHARACTERS: [Option<Character>; FighterKind::COUNT] = [
    Some(Character {
        kind: FighterKind::Mario,
        name: "Mario",
        tables: &kind_tables::KINDS[0],
        callbacks: &kind_callbacks::KINDS[0],
        first_init: None,
        on_load: |r, g, _| r.mr_on_load(g),
        on_death: |r, g| r.mr_on_death(g),
        load_special_attrs: |r, g| r.mr_load_special_attrs(g),
        fighter: Some(mario::run),
        calls: Some(mario::call),
        attrs_size: mario::attr::SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.mr_special_n_enter(g, &kirby::special_mario::KIRBY),
            special_air_n: |r, g| r.mr_special_air_n_enter(g, &kirby::special_mario::KIRBY),
            hat_on: Some(|r, g| r.kb_mario_hat_on(g)),
            hat_off: Some(|r, g| r.kb_head_hat_off(g)),
            cleanup: None,
            hit_cleanup: None,
            items: &[(0, ItKind::KirbyMarioFireball)],
            hat_jumps: false,
        },
        item_words: &[vars::MARIO_CAPE, vars::MARIO_X2240],
        vs: Some(CharacterKind::Mario),
        keys: &["mario"],
    }),
    Some(Character {
        kind: FighterKind::Fox,
        name: "Fox",
        tables: &kind_tables::KINDS[1],
        callbacks: &kind_callbacks::KINDS[1],
        first_init: None,
        on_load: |r, g, _| r.fox_on_load(g),
        on_death: |r, g| r.fox_on_death(g),
        load_special_attrs: |r, g| r.fox_load_special_attrs(g),
        fighter: Some(fox::run),
        calls: Some(fox::call),
        attrs_size: fox::attr::SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.kb_fx_enter(g),
            special_air_n: |r, g| r.kb_fx_air_enter(g),
            hat_on: Some(|r, g| r.kb_fox_hat_on(g)),
            hat_off: Some(|r, g| r.kb_head_hat_off(g)),
            cleanup: Some(|r, g| r.kb_fx_remove_blaster(g)),
            hit_cleanup: Some(|r, g| r.kb_fx_remove_blaster(g)),
            items: &[(0, ItKind::KirbyFoxLaser), (1, ItKind::KirbyFoxBlaster)],
            hat_jumps: false,
        },
        item_words: &[vars::FOX_BLASTER],
        vs: Some(CharacterKind::Fox),
        keys: &["fox"],
    }),
    Some(Character {
        kind: FighterKind::Captain,
        name: "Captain Falcon",
        tables: &kind_tables::KINDS[2],
        callbacks: &kind_callbacks::KINDS[2],
        first_init: None,
        on_load: |r, g, _| r.ca_on_load(g),
        on_death: |r, g| r.ca_on_death(g),
        load_special_attrs: |r, g| r.ca_load_special_attrs(g),
        fighter: Some(captain::run),
        calls: Some(captain::call),
        attrs_size: kinds::CAPTAIN_ATTRS_SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.ca_special_n_enter(g, &kirby::special_captain::KIRBY),
            special_air_n: |r, g| r.ca_special_air_n_enter(g, &kirby::special_captain::KIRBY),
            hat_on: Some(|r, g| r.kb_captain_hat_on(g)),
            hat_off: Some(|r, g| r.kb_head_hat_off(g)),
            cleanup: None,
            hit_cleanup: None,
            items: &[],
            hat_jumps: true,
        },
        item_words: &[],
        vs: Some(CharacterKind::Captain),
        keys: &["falcon", "captain-falcon", "cf"],
    }),
    Some(Character {
        kind: FighterKind::Donkey,
        name: "Donkey Kong",
        tables: &kind_tables::KINDS[3],
        callbacks: &kind_callbacks::KINDS[3],
        first_init: None,
        on_load: |r, g, _| r.dk_on_load(g),
        on_death: |r, g| r.dk_on_death(g),
        load_special_attrs: |r, g| r.dk_load_special_attrs(g),
        fighter: Some(donkey::run),
        calls: Some(donkey::call),
        attrs_size: donkey::attr::SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.dk_special_n_enter(g, &kirby::special_donkey::KIRBY),
            special_air_n: |r, g| r.dk_special_air_n_enter(g, &kirby::special_donkey::KIRBY),
            hat_on: Some(|r, g| r.kb_donkey_hat_on(g)),
            hat_off: Some(|r, g| r.kb_donkey_hat_off(g)),
            cleanup: Some(|r, g| {
                r.dk_special_n_destroy_effects_plus(g, &kirby::special_donkey::KIRBY)
            }),
            hit_cleanup: Some(|r, g| {
                r.dk_special_n_destroy_effects_plus(g, &kirby::special_donkey::KIRBY)
            }),
            items: &[],
            hat_jumps: false,
        },
        item_words: &[],
        vs: Some(CharacterKind::Donkey),
        keys: &["donkey-kong", "dk", "donkey"],
    }),
    Some(Character {
        kind: FighterKind::Kirby,
        name: "Kirby",
        tables: &kind_tables::KINDS[4],
        callbacks: &kind_callbacks::KINDS[4],
        first_init: Some(|r| r.game.ft.kirby.reset()),
        on_load: |r, g, _| r.kb_on_load(g),
        on_death: |r, g| r.kb_on_death(g),
        load_special_attrs: |r, g| r.kb_load_special_attrs(g),
        fighter: Some(kirby::run),
        calls: Some(kirby::call),
        attrs_size: kirby::attr::SIZE,
        kirby_copy: NO_COPY,
        item_words: &vars::KIRBY_ITEMS,
        vs: Some(CharacterKind::Kirby),
        keys: &["kirby"],
    }),
    Some(Character {
        kind: FighterKind::Koopa,
        name: "Bowser",
        tables: &kind_tables::KINDS[5],
        callbacks: &kind_callbacks::KINDS[5],
        first_init: None,
        on_load: |r, g, _| r.kp_on_load(g),
        on_death: |r, g| r.kp_on_death(g),
        load_special_attrs: |r, g| r.kp_load_special_attrs(g),
        fighter: Some(koopa::run),
        calls: Some(koopa::call),
        attrs_size: koopa::attr::SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.kp_special_n_enter(g, &kirby::special_koopa::KIRBY),
            special_air_n: |r, g| r.kp_special_air_n_enter(g, &kirby::special_koopa::KIRBY),
            hat_on: Some(|r, g| r.kb_koopa_hat_on(g)),
            hat_off: Some(|r, g| r.kb_pikachu_hat_off(g)),
            cleanup: None,
            hit_cleanup: None,
            items: &[(0, ItKind::KirbyBowserFlame)],
            hat_jumps: false,
        },
        item_words: &[],
        vs: Some(CharacterKind::Koopa),
        keys: &["bowser", "koopa"],
    }),
    Some(Character {
        kind: FighterKind::Link,
        name: "Link",
        tables: &kind_tables::KINDS[6],
        callbacks: &kind_callbacks::KINDS[6],
        first_init: None,
        on_load: |r, g, by| r.link_on_load(g, by),
        on_death: |r, g| r.link_on_death(g),
        load_special_attrs: |r, g| r.link_load_special_attrs(g),
        fighter: Some(link::run),
        calls: Some(link::call),
        attrs_size: link::attr::SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.link_special_n_enter(g, &kirby::special_link::KIRBY, false),
            special_air_n: |r, g| r.link_special_n_enter(g, &kirby::special_link::KIRBY, true),
            hat_on: Some(|r, g| r.kb_link_hat_on(g)),
            hat_off: Some(|r, g| r.kb_pikachu_hat_off(g)),
            cleanup: Some(|r, g| r.kb_lk_cleanup(g)),
            hit_cleanup: Some(|r, g| r.kb_lk_cleanup(g)),
            items: &[(0, ItKind::KirbyLinkArrow), (1, ItKind::KirbyLinkBow)],
            hat_jumps: false,
        },
        item_words: &vars::LINK_ITEMS,
        vs: Some(CharacterKind::Link),
        keys: &["link"],
    }),
    Some(Character {
        kind: FighterKind::Seak,
        name: "Sheik",
        tables: &kind_tables::KINDS[7],
        callbacks: &kind_callbacks::KINDS[7],
        first_init: None,
        on_load: |r, g, _| r.seak_on_load(g),
        on_death: |r, g| r.seak_on_death(g),
        load_special_attrs: |r, g| r.seak_load_special_attrs(g),
        fighter: Some(seak::run),
        calls: Some(seak::call),
        attrs_size: kinds::SEAK_ATTRS_SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.seak_special_n_enter(g, &kirby::special_seak::KIRBY),
            special_air_n: |r, g| r.seak_special_air_n_enter(g, &kirby::special_seak::KIRBY),
            hat_on: Some(|r, g| r.kb_seak_hat_on(g)),
            hat_off: Some(|r, g| r.kb_pikachu_hat_off(g)),
            cleanup: Some(|r, g| r.seak_drop_needles(g, &kirby::special_seak::KIRBY)),
            hit_cleanup: Some(|r, g| r.seak_drop_needles(g, &kirby::special_seak::KIRBY)),
            items: &[
                (0, ItKind::KirbySheikNeedleThrown),
                (1, ItKind::KirbySheikNeedleHeld),
            ],
            hat_jumps: true,
        },
        item_words: &[vars::SEAK_NEEDLES, vars::SEAK_CHAIN],
        vs: Some(CharacterKind::Seak),
        keys: &["sheik"],
    }),
    Some(Character {
        kind: FighterKind::Ness,
        name: "Ness",
        tables: &kind_tables::KINDS[8],
        callbacks: &kind_callbacks::KINDS[8],
        first_init: None,
        on_load: |r, g, _| r.ness_on_load(g),
        on_death: |r, g| r.ness_on_death(g),
        load_special_attrs: |r, g| r.ness_load_special_attrs(g),
        fighter: Some(ness::run),
        calls: Some(ness::call),
        attrs_size: ness::init::ATTRS_SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.ness_special_n_start_enter(g, &kirby::special_ness::KIRBY),
            special_air_n: |r, g| r.ness_special_air_n_start_enter(g, &kirby::special_ness::KIRBY),
            hat_on: Some(|r, g| r.kb_ness_hat_on(g)),
            hat_off: Some(|r, g| r.kb_head_hat_off(g)),
            cleanup: Some(|r, g| r.ness_pkflash_set_null(g, &kirby::special_ness::KIRBY)),
            hit_cleanup: Some(|r, g| r.ness_pkflash_set_null(g, &kirby::special_ness::KIRBY)),
            items: &[
                (0, ItKind::KirbyNessPKFlash),
                (1, ItKind::KirbyNessPKFlashExplosion),
            ],
            hat_jumps: false,
        },
        item_words: &vars::NESS_ITEMS,
        vs: Some(CharacterKind::Ness),
        keys: &["ness"],
    }),
    Some(Character {
        kind: FighterKind::Peach,
        name: "Peach",
        tables: &kind_tables::KINDS[9],
        callbacks: &kind_callbacks::KINDS[9],
        first_init: None,
        on_load: |r, g, _| r.peach_on_load(g),
        on_death: |r, g| r.peach_on_death(g),
        load_special_attrs: |r, g| r.peach_load_special_attrs(g),
        fighter: Some(peach::run),
        calls: Some(peach::call),
        attrs_size: peach::attr::SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.peach_special_n_enter(g, &kirby::special_peach::KIRBY),
            special_air_n: |r, g| r.peach_special_air_n_enter(g, &kirby::special_peach::KIRBY),
            hat_on: Some(|r, g| r.kb_peach_hat_on(g)),
            hat_off: Some(|r, g| r.kb_head_hat_off(g)),
            cleanup: Some(|r, g| r.peach_toad_on_death2(g, &kirby::special_peach::KIRBY)),
            hit_cleanup: Some(|r, g| r.peach_toad_on_death2(g, &kirby::special_peach::KIRBY)),
            items: &[
                (0, ItKind::KirbyPeachToad),
                (1, ItKind::KirbyPeachToadSpore),
            ],
            hat_jumps: false,
        },
        item_words: &vars::PEACH_ITEMS,
        vs: Some(CharacterKind::Peach),
        keys: &["peach"],
    }),
    Some(Character {
        kind: FighterKind::Popo,
        name: "Ice Climbers",
        tables: &kind_tables::KINDS[10],
        callbacks: &kind_callbacks::KINDS[10],
        first_init: None,
        on_load: |r, g, _| r.popo_on_load(g),
        on_death: |r, g| r.popo_on_death(g),
        load_special_attrs: |r, g| r.popo_load_special_attrs(g),
        fighter: Some(popo::run),
        calls: Some(popo::call),
        attrs_size: popo::attr::SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.popo_special_n_enter(g, &kirby::special_iceclimber::KIRBY),
            special_air_n: |r, g| r.popo_special_air_n_enter(g, &kirby::special_iceclimber::KIRBY),
            hat_on: Some(|r, g| r.kb_popo_hat_on(g)),
            hat_off: Some(|r, g| r.kb_head_hat_off(g)),
            cleanup: Some(|r, g| r.popo_remove_ice(g, &kirby::special_iceclimber::KIRBY)),
            hit_cleanup: Some(|r, g| r.popo_remove_ice(g, &kirby::special_iceclimber::KIRBY)),
            items: &[(0, ItKind::KirbyIceClimbersIceShot)],
            hat_jumps: true,
        },
        item_words: &vars::ICE_CLIMBER_ITEMS,
        vs: Some(CharacterKind::IceClimbers),
        keys: &["ice-climbers", "ics"],
    }),
    Some(Character {
        kind: FighterKind::Nana,
        name: "Nana",
        tables: &kind_tables::KINDS[11],
        callbacks: &kind_callbacks::KINDS[11],
        first_init: None,
        on_load: |r, g, _| r.nana_on_load(g),
        on_death: |r, g| r.nana_on_death(g),
        load_special_attrs: |r, g| r.popo_load_special_attrs(g),
        fighter: None,
        calls: None,
        attrs_size: popo::attr::SIZE,
        kirby_copy: KirbyCopy {
            hat_jumps: true,
            ..NO_COPY
        },
        item_words: &vars::ICE_CLIMBER_ITEMS,
        vs: None,
        keys: &[],
    }),
    Some(Character {
        kind: FighterKind::Pikachu,
        name: "Pikachu",
        tables: &kind_tables::KINDS[12],
        callbacks: &kind_callbacks::KINDS[12],
        first_init: None,
        on_load: |r, g, _| r.pk_on_load(g),
        on_death: |r, g| r.pk_on_death(g),
        load_special_attrs: |r, g| r.pk_load_special_attrs(g),
        fighter: Some(pikachu::run),
        calls: Some(pikachu::call),
        attrs_size: pikachu::attr::SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.pk_special_n_enter(g, &kirby::special_pikachu::KIRBY),
            special_air_n: |r, g| r.pk_special_air_n_enter(g, &kirby::special_pikachu::KIRBY),
            hat_on: Some(|r, g| r.kb_pikachu_hat_on(g)),
            hat_off: Some(|r, g| r.kb_pikachu_hat_off(g)),
            cleanup: None,
            hit_cleanup: None,
            items: &[
                (0, ItKind::KirbyPikachuThunderJoltGround),
                (1, ItKind::KirbyPikachuThunderJoltAir),
            ],
            hat_jumps: false,
        },
        item_words: &[],
        vs: Some(CharacterKind::Pikachu),
        keys: &["pikachu", "pika"],
    }),
    Some(Character {
        kind: FighterKind::Samus,
        name: "Samus",
        tables: &kind_tables::KINDS[13],
        callbacks: &kind_callbacks::KINDS[13],
        first_init: None,
        on_load: |r, g, _| r.samus_on_load(g),
        on_death: |r, g| r.samus_on_death(g),
        load_special_attrs: |r, g| r.samus_load_special_attrs(g),
        fighter: Some(samus::run),
        calls: Some(samus::call),
        attrs_size: samus::attr::SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.samus_special_n_enter(g, &kirby::special_samus::KIRBY),
            special_air_n: |r, g| r.samus_special_air_n_enter(g, &kirby::special_samus::KIRBY),
            hat_on: Some(|r, g| r.kb_samus_hat_on(g)),
            hat_off: Some(|r, g| r.kb_samus_hat_off(g)),
            cleanup: Some(|r, g| r.samus_drop_charge(g, &kirby::special_samus::KIRBY)),
            hit_cleanup: Some(|r, g| r.samus_drop_charge(g, &kirby::special_samus::KIRBY)),
            items: &[(0, ItKind::KirbySamusChargeShot)],
            hat_jumps: true,
        },
        item_words: &vars::SAMUS_ITEMS,
        vs: Some(CharacterKind::Samus),
        keys: &["samus"],
    }),
    Some(Character {
        kind: FighterKind::Yoshi,
        name: "Yoshi",
        tables: &kind_tables::KINDS[14],
        callbacks: &kind_callbacks::KINDS[14],
        first_init: None,
        on_load: |r, g, _| r.ys_on_load(g),
        on_death: |r, g| r.ys_on_death(g),
        load_special_attrs: |r, g| r.ys_load_special_attrs(g),
        fighter: Some(yoshi::run),
        calls: Some(yoshi::call),
        attrs_size: yoshi::attr::SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.kb_ys_enter(g),
            special_air_n: |r, g| r.kb_ys_air_enter(g),
            hat_on: Some(|r, g| r.kb_yoshi_hat_on(g)),
            hat_off: Some(|r, g| r.kb_head_hat_off(g)),
            cleanup: Some(|r, g| r.kb_ys_cleanup(g)),
            hit_cleanup: Some(|r, g| r.kb_ys_cleanup(g)),
            items: &[(5, ItKind::KirbyYoshiEggLay)],
            hat_jumps: false,
        },
        item_words: &[vars::YOSHI_EGG],
        vs: Some(CharacterKind::Yoshi),
        keys: &["yoshi"],
    }),
    Some(Character {
        kind: FighterKind::Purin,
        name: "Jigglypuff",
        tables: &kind_tables::KINDS[15],
        callbacks: &kind_callbacks::KINDS[15],
        first_init: Some(|r| r.game.ft.purin_hats = [None; 5]),
        on_load: |r, g, _| r.purin_on_load(g),
        on_death: |r, g| r.purin_on_death(g),
        load_special_attrs: |r, g| r.purin_load_special_attrs(g),
        fighter: Some(purin::run),
        calls: Some(purin::call),
        attrs_size: purin::attr::SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.rollout_enter(g, &kirby::special_purin::KIRBY),
            special_air_n: |r, g| r.rollout_air_enter(g, &kirby::special_purin::KIRBY),
            hat_on: Some(|r, g| r.kb_purin_hat_on(g)),
            hat_off: Some(|r, g| r.kb_purin_hat_off(g)),
            cleanup: None,
            hit_cleanup: None,
            items: &[],
            hat_jumps: false,
        },
        item_words: &[],
        vs: Some(CharacterKind::Purin),
        keys: &["jigglypuff", "puff", "jiggs"],
    }),
    Some(Character {
        kind: FighterKind::Mewtwo,
        name: "Mewtwo",
        tables: &kind_tables::KINDS[16],
        callbacks: &kind_callbacks::KINDS[16],
        first_init: None,
        on_load: |r, g, _| r.mt_on_load(g),
        on_death: |r, g| r.mt_on_death(g),
        load_special_attrs: |r, g| r.mt_load_special_attrs(g),
        fighter: Some(mewtwo::run),
        calls: Some(mewtwo::call),
        attrs_size: mewtwo::attr::SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.mt_special_n_enter(g, &kirby::special_mewtwo::KIRBY),
            special_air_n: |r, g| r.mt_special_air_n_enter(g, &kirby::special_mewtwo::KIRBY),
            hat_on: Some(|r, g| r.kb_mewtwo_hat_on(g)),
            hat_off: Some(|r, g| r.kb_mewtwo_hat_off(g)),
            cleanup: Some(|r, g| r.mt_n_lose_ball(g, &kirby::special_mewtwo::KIRBY)),
            hit_cleanup: Some(|r, g| r.mt_n_drop_ball(g, &kirby::special_mewtwo::KIRBY)),
            items: &[(3, ItKind::KirbyMewtwoShadowBall)],
            hat_jumps: false,
        },
        item_words: &vars::MEWTWO_ITEMS,
        vs: Some(CharacterKind::Mewtwo),
        keys: &["mewtwo"],
    }),
    Some(Character {
        kind: FighterKind::Luigi,
        name: "Luigi",
        tables: &kind_tables::KINDS[17],
        callbacks: &kind_callbacks::KINDS[17],
        first_init: None,
        on_load: |r, g, _| r.lg_on_load(g),
        on_death: |r, g| r.lg_on_death(g),
        load_special_attrs: |r, g| r.lg_load_special_attrs(g),
        fighter: Some(luigi::run),
        calls: Some(luigi::call),
        attrs_size: luigi::attr::SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.lg_special_n_enter(g, &kirby::special_luigi::KIRBY),
            special_air_n: |r, g| r.lg_special_air_n_enter(g, &kirby::special_luigi::KIRBY),
            hat_on: Some(|r, g| r.kb_luigi_hat_on(g)),
            hat_off: Some(|r, g| r.kb_head_hat_off(g)),
            cleanup: None,
            hit_cleanup: None,
            items: &[(0, ItKind::KirbyLuigiFireball)],
            hat_jumps: false,
        },
        item_words: &[],
        vs: Some(CharacterKind::Luigi),
        keys: &["luigi"],
    }),
    Some(Character {
        kind: FighterKind::Mars,
        name: "Marth",
        tables: &kind_tables::KINDS[18],
        callbacks: &kind_callbacks::KINDS[18],
        first_init: None,
        on_load: |r, g, _| r.mars_on_load(g),
        on_death: |r, g| r.mars_on_death(g),
        load_special_attrs: |r, g| r.mars_load_special_attrs(g),
        fighter: Some(mars::run),
        calls: Some(mars::call),
        attrs_size: kinds::MARS_ATTRS_SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.mars_special_n_enter(g, &kirby::special_mars::KIRBY),
            special_air_n: |r, g| r.mars_special_air_n_enter(g, &kirby::special_mars::KIRBY),
            hat_on: Some(|r, g| r.kb_mars_hat_on(g)),
            hat_off: Some(|r, g| r.kb_pikachu_hat_off(g)),
            cleanup: None,
            hit_cleanup: None,
            items: &[],
            hat_jumps: false,
        },
        item_words: &[],
        vs: Some(CharacterKind::Mars),
        keys: &["marth"],
    }),
    Some(Character {
        kind: FighterKind::Zelda,
        name: "Zelda",
        tables: &kind_tables::KINDS[19],
        callbacks: &kind_callbacks::KINDS[19],
        first_init: None,
        on_load: |r, g, _| r.zelda_on_load(g),
        on_death: |r, g| r.zelda_on_death(g),
        load_special_attrs: |r, g| r.zelda_load_special_attrs(g),
        fighter: Some(zelda::run),
        calls: Some(zelda::call),
        attrs_size: kinds::ZELDA_ATTRS_SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.zelda_special_n_enter(g, &kirby::special_zelda::KIRBY),
            special_air_n: |r, g| r.zelda_special_air_n_enter(g, &kirby::special_zelda::KIRBY),
            hat_on: Some(|r, g| r.kb_zelda_hat_on(g)),
            hat_off: Some(|r, g| r.kb_pikachu_hat_off(g)),
            cleanup: None,
            hit_cleanup: None,
            items: &[],
            hat_jumps: false,
        },
        item_words: &[vars::ZELDA_DIN_FIRE],
        vs: Some(CharacterKind::Zelda),
        keys: &["zelda"],
    }),
    Some(Character {
        kind: FighterKind::Clink,
        name: "Young Link",
        tables: &kind_tables::KINDS[20],
        callbacks: &kind_callbacks::KINDS[20],
        first_init: None,
        on_load: |r, g, by| r.clink_on_load(g, by),
        on_death: |r, g| r.link_on_death(g),
        load_special_attrs: |r, g| r.link_load_special_attrs(g),
        fighter: Some(clink::run),
        calls: Some(clink::call),
        attrs_size: link::attr::SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.link_special_n_enter(g, &kirby::special_link::KIRBY, false),
            special_air_n: |r, g| r.link_special_n_enter(g, &kirby::special_link::KIRBY, true),
            hat_on: Some(|r, g| r.kb_clink_hat_on(g)),
            hat_off: Some(|r, g| r.kb_pikachu_hat_off(g)),
            cleanup: Some(|r, g| r.kb_lk_cleanup(g)),
            hit_cleanup: Some(|r, g| r.kb_lk_cleanup(g)),
            items: &[
                (0, ItKind::KirbyYoungLinkFireArrow),
                (1, ItKind::KirbyYoungLinkBow),
            ],
            hat_jumps: false,
        },
        item_words: &vars::LINK_ITEMS,
        vs: Some(CharacterKind::Clink),
        keys: &["young-link", "ylink", "yl"],
    }),
    Some(Character {
        kind: FighterKind::DrMario,
        name: "Dr. Mario",
        tables: &kind_tables::KINDS[21],
        callbacks: &kind_callbacks::KINDS[21],
        first_init: None,
        on_load: |r, g, _| r.dr_on_load(g),
        on_death: |r, g| r.dr_on_death(g),
        load_special_attrs: |r, g| r.dr_load_special_attrs(g),
        fighter: Some(drmario::run),
        calls: Some(drmario::call),
        attrs_size: mario::attr::SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.mr_special_n_enter(g, &kirby::special_mario::KIRBY),
            special_air_n: |r, g| r.mr_special_air_n_enter(g, &kirby::special_mario::KIRBY),
            hat_on: Some(|r, g| r.kb_drmario_hat_on(g)),
            hat_off: Some(|r, g| r.kb_head_hat_off(g)),
            cleanup: None,
            hit_cleanup: None,
            items: &[(0, ItKind::KirbyDrMarioMegavitamin)],
            hat_jumps: false,
        },
        item_words: &[vars::MARIO_CAPE, vars::MARIO_X2240],
        vs: Some(CharacterKind::DrMario),
        keys: &["drmario", "dr-mario", "doc"],
    }),
    Some(Character {
        kind: FighterKind::Falco,
        name: "Falco",
        tables: &kind_tables::KINDS[22],
        callbacks: &kind_callbacks::KINDS[22],
        first_init: None,
        on_load: |r, g, _| r.falco_on_load(g),
        on_death: |r, g| r.falco_on_death(g),
        load_special_attrs: |r, g| r.falco_load_special_attrs(g),
        fighter: Some(falco::run),
        calls: Some(falco::call),
        attrs_size: fox::attr::SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.kb_fx_enter(g),
            special_air_n: |r, g| r.kb_fx_air_enter(g),
            hat_on: Some(|r, g| r.kb_falco_hat_on(g)),
            hat_off: Some(|r, g| r.kb_falco_hat_off(g)),
            cleanup: Some(|r, g| r.kb_fx_remove_blaster(g)),
            hit_cleanup: Some(|r, g| r.kb_fx_remove_blaster(g)),
            items: &[(3, ItKind::KirbyFalcoLaser), (4, ItKind::KirbyFalcoBlaster)],
            hat_jumps: false,
        },
        item_words: &[vars::FOX_BLASTER],
        vs: Some(CharacterKind::Falco),
        keys: &["falco"],
    }),
    Some(Character {
        kind: FighterKind::Pichu,
        name: "Pichu",
        tables: &kind_tables::KINDS[23],
        callbacks: &kind_callbacks::KINDS[23],
        first_init: None,
        on_load: |r, g, _| r.pc_on_load(g),
        on_death: |r, g| r.pc_on_death(g),
        load_special_attrs: |r, g| r.pk_load_special_attrs(g),
        fighter: Some(pichu::run),
        calls: Some(pichu::call),
        attrs_size: pikachu::attr::SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.pk_special_n_enter(g, &kirby::special_pikachu::KIRBY),
            special_air_n: |r, g| r.pk_special_air_n_enter(g, &kirby::special_pikachu::KIRBY),
            hat_on: Some(|r, g| r.kb_pichu_hat_on(g)),
            hat_off: Some(|r, g| r.kb_pikachu_hat_off(g)),
            cleanup: None,
            hit_cleanup: None,
            items: &[
                (0, ItKind::KirbyPichuThunderJoltGround),
                (1, ItKind::KirbyPichuThunderJoltAir),
            ],
            hat_jumps: false,
        },
        item_words: &[],
        vs: Some(CharacterKind::Pichu),
        keys: &["pichu"],
    }),
    Some(Character {
        kind: FighterKind::GameWatch,
        name: "Mr. Game & Watch",
        tables: &kind_tables::KINDS[24],
        callbacks: &kind_callbacks::KINDS[24],
        first_init: None,
        on_load: |r, g, _| r.gw_on_load(g),
        on_death: |r, g| r.gw_on_death(g),
        load_special_attrs: |r, g| r.gw_load_special_attrs(g),
        fighter: Some(gamewatch::run),
        calls: Some(gamewatch::call),
        attrs_size: gamewatch::attr::SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.gw_special_n_enter(g, &kirby::special_gamewatch::KIRBY, false),
            special_air_n: |r, g| r.gw_special_n_enter(g, &kirby::special_gamewatch::KIRBY, true),
            hat_on: Some(|r, g| r.kb_gamewatch_hat_on(g)),
            hat_off: Some(|r, g| r.kb_gamewatch_hat_off(g)),
            cleanup: Some(|r, g| r.kb_gw_cleanup(g)),
            hit_cleanup: Some(|r, g| r.kb_gw_cleanup(g)),
            items: &[
                (5, ItKind::KirbyGameAndWatchSausage),
                (6, ItKind::KirbyGameAndWatchPan),
            ],
            hat_jumps: false,
        },
        item_words: &gamewatch::u::ITEMS,
        vs: Some(CharacterKind::GameWatch),
        keys: &["game-and-watch", "gnw", "gw", "mr-game-and-watch"],
    }),
    Some(Character {
        kind: FighterKind::Ganon,
        name: "Ganondorf",
        tables: &kind_tables::KINDS[25],
        callbacks: &kind_callbacks::KINDS[25],
        first_init: None,
        on_load: |r, g, _| r.gn_on_load(g),
        on_death: |r, g| r.gn_on_death(g),
        load_special_attrs: |r, g| r.ca_load_special_attrs(g),
        fighter: None,
        calls: None,
        attrs_size: kinds::CAPTAIN_ATTRS_SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.ca_special_n_enter(g, &kirby::special_captain::KIRBY),
            special_air_n: |r, g| r.ca_special_air_n_enter(g, &kirby::special_captain::KIRBY),
            hat_on: Some(|r, g| r.kb_ganon_hat_on(g)),
            hat_off: Some(|r, g| r.kb_head_hat_off(g)),
            cleanup: None,
            hit_cleanup: None,
            items: &[],
            hat_jumps: false,
        },
        item_words: &[],
        vs: Some(CharacterKind::Ganon),
        keys: &["ganondorf", "ganon"],
    }),
    Some(Character {
        kind: FighterKind::Emblem,
        name: "Roy",
        tables: &kind_tables::KINDS[26],
        callbacks: &kind_callbacks::KINDS[26],
        first_init: None,
        on_load: |r, g, _| r.fe_on_load(g),
        on_death: |r, g| r.fe_on_death(g),
        load_special_attrs: |r, g| r.fe_load_special_attrs(g),
        fighter: Some(emblem::run),
        calls: Some(emblem::call),
        attrs_size: kinds::MARS_ATTRS_SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.mars_special_n_enter(g, &kirby::special_mars::KIRBY),
            special_air_n: |r, g| r.mars_special_air_n_enter(g, &kirby::special_mars::KIRBY),
            hat_on: Some(|r, g| r.kb_emblem_hat_on(g)),
            hat_off: Some(|r, g| r.kb_pikachu_hat_off(g)),
            cleanup: None,
            hit_cleanup: None,
            items: &[],
            hat_jumps: false,
        },
        item_words: &[],
        vs: Some(CharacterKind::Emblem),
        keys: &["roy"],
    }),
    Some(Character {
        kind: FighterKind::MasterHand,
        name: "Master Hand",
        tables: &kind_tables::KINDS[27],
        callbacks: &kind_callbacks::KINDS[27],
        first_init: None,
        on_load: |r, g, _| r.hand_on_load(g, &masterhand::HAND),
        on_death: |r, g| r.hand_nothing(g),
        load_special_attrs: |r, g| r.hand_load_special_attrs(g, &masterhand::HAND),
        fighter: Some(masterhand::run),
        calls: None,
        attrs_size: masterhand::attr::SIZE,
        kirby_copy: NO_COPY,
        item_words: &[],
        vs: None,
        keys: &[],
    }),
    Some(Character {
        kind: FighterKind::CrazyHand,
        name: "Crazy Hand",
        tables: &kind_tables::KINDS[28],
        callbacks: &kind_callbacks::KINDS[28],
        first_init: None,
        on_load: |r, g, _| r.hand_on_load(g, &crazyhand::HAND),
        on_death: |r, g| r.hand_nothing(g),
        load_special_attrs: |r, g| r.hand_load_special_attrs(g, &crazyhand::HAND),
        fighter: Some(crazyhand::run),
        calls: None,
        attrs_size: crazyhand::attr::SIZE,
        kirby_copy: NO_COPY,
        item_words: &[],
        vs: None,
        keys: &[],
    }),
    Some(Character {
        kind: FighterKind::Boy,
        name: "The male wireframe",
        tables: &kind_tables::KINDS[29],
        callbacks: &kind_callbacks::KINDS[29],
        first_init: None,
        on_load: |r, g, _| r.zako_on_load(g),
        on_death: |r, g| r.zako_on_death(g),
        load_special_attrs: |r, g| r.zako_load_special_attrs(g),
        fighter: Some(zako::run),
        calls: Some(zako::call),
        attrs_size: zako::ATTRS_SIZE,
        kirby_copy: NO_COPY,
        item_words: &[],
        vs: None,
        keys: &[],
    }),
    Some(Character {
        kind: FighterKind::Girl,
        name: "The female wireframe",
        tables: &kind_tables::KINDS[30],
        callbacks: &kind_callbacks::KINDS[30],
        first_init: None,
        on_load: |r, g, _| r.zako_on_load(g),
        on_death: |r, g| r.zako_on_death(g),
        load_special_attrs: |r, g| r.zako_load_special_attrs(g),
        fighter: None,
        calls: None,
        attrs_size: zako::ATTRS_SIZE,
        kirby_copy: NO_COPY,
        item_words: &[],
        vs: None,
        keys: &[],
    }),
    Some(Character {
        kind: FighterKind::GKoopa,
        name: "Giga Bowser",
        tables: &kind_tables::KINDS[31],
        callbacks: &kind_callbacks::KINDS[31],
        first_init: None,
        on_load: |r, g, _| r.gk_on_load(g),
        on_death: |r, g| r.gk_on_death(g),
        load_special_attrs: |r, g| r.gk_load_special_attrs(g),
        fighter: Some(gkoopa::run),
        calls: Some(gkoopa::call),
        attrs_size: koopa::attr::SIZE,
        kirby_copy: KirbyCopy {
            special_n: |r, g| r.kp_special_n_enter(g, &kirby::special_koopa::KIRBY),
            special_air_n: |r, g| r.kp_special_air_n_enter(g, &kirby::special_koopa::KIRBY),
            hat_on: Some(|r, g| r.kb_koopa_hat_on(g)),
            hat_off: Some(|r, g| r.kb_pikachu_hat_off(g)),
            cleanup: None,
            hit_cleanup: None,
            items: &[(0, ItKind::KirbyBowserFlame)],
            hat_jumps: false,
        },
        item_words: &[],
        vs: None,
        keys: &[],
    }),
    Some(Character {
        kind: FighterKind::Sandbag,
        name: "The Sandbag",
        tables: &kind_tables::KINDS[32],
        callbacks: &kind_callbacks::KINDS[32],
        first_init: None,
        on_load: |r, g, _| r.sb_on_load(g),
        on_death: |r, g| r.sb_on_death(g),
        load_special_attrs: |r, g| r.sb_load_special_attrs(g),
        fighter: Some(sandbag::run),
        calls: None,
        attrs_size: sandbag::ATTRS_SIZE,
        kirby_copy: NO_COPY,
        item_words: &[],
        vs: None,
        keys: &[],
    }),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Each entry sits at its fighter kind's index, its tables are its kind's, and each kind's
    /// number is its index.
    #[test]
    fn the_registry_is_indexed_by_fighter_kind() {
        for (i, k) in FighterKind::ALL.iter().enumerate() {
            assert_eq!(k.id(), i as i32);
            assert_eq!(FighterKind::from_id(i as i32), Some(*k));
        }
        for (i, k) in CharacterKind::ALL.iter().enumerate() {
            assert_eq!(k.id(), i as i32);
            assert_eq!(CharacterKind::from_id(i as i32), Some(*k));
        }
        assert_eq!(FighterKind::COUNT, 33);
        assert_eq!(CharacterKind::COUNT, usize::from(CharacterKind::NONE));
        assert_eq!(FighterKind::from_id(-1), None);
        assert_eq!(FighterKind::from_id(FighterKind::COUNT as i32), None);
        for (i, c) in CHARACTERS.iter().enumerate() {
            let c = c.as_ref().expect("every fighter kind is ported");
            assert_eq!(c.kind.index(), i, "{}", c.name);
            assert!(std::ptr::eq(c.tables, &kind_tables::KINDS[i]), "{}", c.name);
            assert!(
                std::ptr::eq(c.callbacks, &kind_callbacks::KINDS[i]),
                "{}",
                c.name
            );
        }
    }

    /// Each roster character's character kind plays its fighter kind (`ftMapping_list`), and
    /// no two roster lines share a character kind.
    #[test]
    fn roster_characters_play_their_fighter_kinds() {
        let mut seen = Vec::new();
        for c in ported() {
            if let Some(ck) = c.vs {
                let ft = super::super::super::pl::player::ckind_ftkind(ck.id(), 0);
                assert_eq!(c.kind, ft, "{}", c.name);
                assert!(!seen.contains(&ck), "{ck:?} twice");
                seen.push(ck);
            }
        }
        assert_eq!(seen.len(), CharacterKind::PLAYABLE);
    }

    /// Each entry's Kirby copy registers only the copy items of its own character (the item
    /// kinds named `Kirby` and the character, `KirbyBowserFlame` for Bowser and Giga Bowser),
    /// and a copy without a hat has no items or cleanups either.
    #[test]
    fn kirby_copies_take_their_own_items() {
        for c in ported() {
            let who: String = c
                .name
                .trim_start_matches("Mr. ")
                .trim_start_matches("Giga ")
                .replace('&', "And")
                .chars()
                .filter(char::is_ascii_alphanumeric)
                .collect();
            for &(_, item) in c.kirby_copy.items {
                let name = format!("{item:?}");
                assert!(
                    name.starts_with(&format!("Kirby{who}")),
                    "{}'s copy registers {name}",
                    c.name
                );
            }
            if c.kirby_copy.hat_on.is_none() {
                assert!(c.kirby_copy.items.is_empty(), "{}", c.name);
                assert!(c.kirby_copy.hat_off.is_none(), "{}", c.name);
                assert!(c.kirby_copy.cleanup.is_none(), "{}", c.name);
            }
        }
    }

    /// Each kind's first setup hook is ported where its table names one.
    #[test]
    fn first_inits_follow_the_tables() {
        for c in ported() {
            assert_eq!(
                c.first_init.is_some(),
                c.tables.on_first_init != 0,
                "{}",
                c.name
            );
        }
    }
}
