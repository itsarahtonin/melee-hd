//! The single-player matches' joining opponents (`gm_16A2.c`'s `lbl_8046B488`): who joins a
//! match as its players fall (Event Match, Multi-Man Melee, Classic's and Adventure's teams and
//! hordes, the adventure routes' arenas), their characters, costumes and Kirby's copies, how
//! many at once and from which stage points, and the callback told when the last is beaten.

use super::super::code::{self, Val};
use super::super::handles::CodeRef;
use super::super::pl::player::{CKIND_KIRBY, PKIND_CPU, PKIND_NONE};
use super::super::state::{Game, Running};

/// The lists' length (`x20`, `xA2` and `x124`, 0x82 bytes each).
pub const LIST_LEN: usize = 0x82;
/// A list's end (`-2`).
pub const END: i8 = -2;
/// No character (`ChKind_None`).
pub const NONE: i8 = 0x21;

/// `unk_10_b0`: the last opponent beaten.
pub const BEATEN: u8 = 0x80;
/// `unk_10_b1`: opponents join as slots free.
pub const JOINING: u8 = 0x40;
/// `unk_10_b2`: the match goes on once they're beaten.
pub const GOES_ON: u8 = 0x20;
/// `unk_10_b3`: the 1P stock display's horde count (`gm_80169394`).
pub const COUNTED: u8 = 0x10;
/// `unk_10_b4`: they join metal, computer kind 0x1B.
pub const METAL: u8 = 0x08;
/// `unk_10_b5`: each joiner's flags' bit 6.
pub const FLAG6: u8 = 0x04;
/// `unk_10_b6`: each joiner's `flagsAE` bit 0.
pub const FLAG_AE0: u8 = 0x02;

/// `fn_8016A488`, the joiners' slot callback.
pub const ON_JOINED: u32 = 0x8016_A488;

/// The joining opponents' setup, `lbl_8046B488` (0x1E0 bytes; `gm_1601_GetUnkData` and
/// `fn_8016AE60` hand it out).
#[doc(alias = "gm_1601_GetUnkData")]
#[doc(alias = "fn_8016AE60")]
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct Opponents {
    /// The setup's characters (`x0` to `x2`, `NONE` for none).
    pub kinds: [i8; 3],
    /// Their costumes as the setup was given them (`x3` to `x5`).
    pub colors: [i8; 3],
    /// The joiners' computer level (`x6`).
    pub cpu_level: u8,
    /// How many are still to join (`x7`).
    pub left: u8,
    /// How many join in all (`x8`).
    pub total: u8,
    /// How many play at once (`x9`, at most 5).
    pub at_once: u8,
    /// The stage point the second slot joins at (`xA`; slot `s` at `xA + s - 1`).
    pub first_point: u8,
    /// How the costumes are drawn (`xB`: 0 shuffled per character, 1 one costume).
    pub costume_mode: u8,
    /// The costume of mode 1 (`xC`).
    pub costume: u8,
    /// How the characters are drawn (`xD`: 0 the first, 1 either of two, 2 mostly the first).
    pub kind_mode: u8,
    /// Kirby's copies (`xE`: 0 none, 1 drawn).
    pub copy_mode: u8,
    /// Each joiner's `more_flags` bit 6 (`xF`).
    pub more_flag6: u8,
    /// The flags (`x10`: [`BEATEN`], [`JOINING`] and the rest).
    pub flags: u8,
    pub x11: [u8; 3],
    /// The joiners' attack and defense ratios and model scale (`x14`, `x18`, `x1C`).
    pub attack_ratio: f32,
    pub defense_ratio: f32,
    pub model_scale: f32,
    /// Each joiner's costume, `END`-terminated (`x20`).
    pub costumes: [i8; LIST_LEN],
    /// Each joiner's character (`xA2`).
    pub characters: [i8; LIST_LEN],
    /// Each Kirby joiner's copy (`x124`).
    pub copies: [i8; LIST_LEN],
    /// Each slot's joiner's index in the lists (`x1A6`).
    pub slot_index: [i8; 6],
    pub x1ac: u8,
    pub x1ad: u8,
    pub x1ae: [i8; 4],
    pub x1b2: u8,
    pub x1b3: [i8; 5],
    /// Told when the last is beaten (1) or the first player falls (2) (`x1B8`); cleared once
    /// it answers 1.
    pub on_end: Option<CodeRef>,
    /// Told of each joiner (`x1BC`, an event's).
    pub on_join: Option<CodeRef>,
    /// Kirby's copies' shuffled characters (`x1C0`).
    pub pool: [i8; 0x1B],
    pub x1db: [u8; 5],
}

impl Default for Opponents {
    fn default() -> Self {
        Opponents {
            kinds: [0; 3],
            colors: [0; 3],
            cpu_level: 0,
            left: 0,
            total: 0,
            at_once: 0,
            first_point: 0,
            costume_mode: 0,
            costume: 0,
            kind_mode: 0,
            copy_mode: 0,
            more_flag6: 0,
            flags: 0,
            x11: [0; 3],
            attack_ratio: 0.0,
            defense_ratio: 0.0,
            model_scale: 0.0,
            costumes: [0; LIST_LEN],
            characters: [0; LIST_LEN],
            copies: [0; LIST_LEN],
            slot_index: [0; 6],
            x1ac: 0,
            x1ad: 0,
            x1ae: [0; 4],
            x1b2: 0,
            x1b3: [0; 5],
            on_end: None,
            on_join: None,
            pool: [0; 0x1B],
            x1db: [0; 5],
        }
    }
}

/// A list's entry; the original reads on past a list's end into the next field, which no
/// setup leaves unterminated.
fn at(list: &[i8; LIST_LEN], i: usize) -> i8 {
    *list
        .get(i)
        .unwrap_or_else(|| panic!("a single-player list read past its end (entry {i})"))
}

impl Opponents {
    /// How many are still to join, `gm_80169384`.
    // Ports gm_80169384; aliased once a check reaches it.
    pub fn left(&self) -> i32 {
        i32::from(self.left)
    }

    /// `gm_80169394`: the horde count's flag.
    #[doc(alias = "gm_80169394")]
    pub fn counted(&self) -> bool {
        self.flags & COUNTED != 0
    }

    /// `fn_801693A8`: whether the match goes on once they're beaten.
    // Ports fn_801693A8; aliased once a check reaches it.
    pub fn goes_on(&self) -> bool {
        self.flags & GOES_ON != 0
    }

    /// `fn_8016A1E4`: whether the last one is beaten.
    // Ports fn_8016A1E4; aliased once a check reaches it.
    pub fn beaten(&self) -> bool {
        self.flags & BEATEN != 0
    }

    /// `gm_8016A1F8`: whether opponents join.
    #[doc(alias = "gm_8016A1F8")]
    pub fn joining(&self) -> bool {
        self.flags & JOINING != 0
    }

    /// `fn_80169434`: the callback told at the end.
    #[doc(alias = "fn_80169434")]
    pub fn set_on_end(&mut self, f: Option<CodeRef>) {
        self.on_end = f;
    }

    /// A list of `n` zeros ended at `n`, `fn_80169574` (on the costumes' list).
    #[doc(alias = "fn_80169574")]
    pub fn reset_costumes(&mut self, n: usize) {
        self.costumes[..n].fill(0);
        self.costumes[n] = END;
    }

    /// Each joiner's character by the kind mode, `fn_80169900`: the first character; either of
    /// the first two (`HSD_Randi(2)` each); or the first but every sixth from the fifth draw on,
    /// one of the other two when it's set and unlocked.
    #[doc(alias = "fn_80169900")]
    pub fn draw_characters(&mut self, game_unlocks: &super::Unlocks, rng: &mut crate::rng::Rng) {
        let n = (0..).take_while(|&i| at(&self.costumes, i) != END).count();
        match self.kind_mode {
            0 => self.characters[..n].fill(self.kinds[0]),
            1 => {
                for i in 0..n {
                    self.characters[i] = if rng.below(2) != 0 {
                        self.kinds[0]
                    } else {
                        self.kinds[1]
                    };
                }
            }
            2 => {
                let mut wait = rng.below(5);
                for i in 0..n {
                    if wait != 0 {
                        wait -= 1;
                        self.characters[i] = self.kinds[0];
                        continue;
                    }
                    wait = 5;
                    let other = if rng.below(2) != 0 {
                        self.kinds[1]
                    } else {
                        self.kinds[2]
                    };
                    self.characters[i] =
                        if other != NONE && game_unlocks.ckind_unlocked(other as u8) {
                            other
                        } else {
                            self.kinds[0]
                        };
                }
            }
            _ => {}
        }
    }

    /// The joiners of one character shuffled through its costumes, `fn_801695BC`: the costumes
    /// shuffled (`HSD_Randi(n)` each), the first player's taken out when it plays the same
    /// character, dealt in turn, then each pair of neighbours of the character swapped half the
    /// time.
    #[doc(alias = "fn_801695BC")]
    pub fn shuffle_costumes(
        &mut self,
        rng: &mut crate::rng::Rng,
        ckind: i8,
        p1_ckind: i8,
        p1_costume: i8,
    ) {
        let n = super::super::playback::costume_count(ckind as u8);
        if ckind == NONE {
            return;
        }
        let n = i32::from(n);
        let mut colors = [0i8; 6];
        for (i, c) in colors.iter_mut().enumerate().take(n as usize) {
            *c = i as i8;
        }
        if p1_ckind == ckind {
            colors[p1_costume as usize] = -1;
        }
        for i in 0..n as usize {
            let k = rng.below(n) as usize;
            colors.swap(k, i);
        }
        let mut color_i = 0i32;
        let mut i = 0;
        while at(&self.costumes, i) != END {
            if at(&self.characters, i) == ckind {
                if colors[(color_i % n) as usize] == -1 {
                    color_i += 1;
                }
                self.costumes[i] = colors[(color_i % n) as usize];
                color_i += 1;
            }
            i += 1;
        }
        let mut i = 1;
        while at(&self.costumes, i) != END {
            if at(&self.characters, i) == ckind
                && at(&self.characters, i - 1) == ckind
                && rng.below(2) != 0
            {
                self.costumes.swap(i, i - 1);
            }
            i += 1;
        }
    }

    /// The three characters' costumes shuffled, `fn_8016989C`.
    #[doc(alias = "fn_8016989C")]
    pub fn shuffle_all_costumes(
        &mut self,
        rng: &mut crate::rng::Rng,
        p1_ckind: i8,
        p1_costume: i8,
    ) {
        for k in 0..3 {
            self.shuffle_costumes(rng, self.kinds[k], p1_ckind, p1_costume);
        }
    }
    /// Every joiner in one costume, `fn_801697FC`: `costume`, or the next one when the first
    /// player wears it with the same character.
    // Ports fn_801697FC; aliased once a check reaches it.
    pub fn one_costume(&mut self, ckind: i8, costume: i8, p1_ckind: i8, p1_costume: i8) {
        let n = i32::from(super::super::playback::costume_count(ckind as u8));
        if ckind == NONE {
            return;
        }
        let mut costume = costume;
        if p1_ckind == ckind && costume == p1_costume {
            costume = ((i32::from(costume) + 1) % n) as i8;
        }
        let mut i = 0;
        while at(&self.costumes, i) != END {
            self.costumes[i] = costume;
            i += 1;
        }
    }

    /// A byte of the copies' pool (`x1C0`), read on past its end into the struct's last bytes
    /// as the original does.
    fn pool_at(&self, i: usize) -> i8 {
        match i {
            0..0x1B => self.pool[i],
            0x1B..0x20 => self.x1db[i - 0x1B] as i8,
            _ => panic!("Kirby's copies' pick read past lbl_8046B488 (entry {i})"),
        }
    }

    /// Each Kirby joiner's copy, `fn_80169A84`: none (mode 0), or (mode 1) the unlocked
    /// characters but Kirby shuffled into the pool (`HSD_Randi(27)` each), the first 16 kept,
    /// dealt in turn as fighter kinds.
    // Ports fn_80169A84; aliased once a check reaches it.
    pub fn draw_copies(&mut self, unlocks: &super::Unlocks, rng: &mut crate::rng::Rng) {
        match self.copy_mode {
            1 => {
                for i in 0..0x1A {
                    self.pool[i] = if i != 4 && unlocks.ckind_unlocked(i as u8) {
                        i as i8
                    } else {
                        -1
                    };
                }
                for i in 0..0x1A {
                    let k = rng.below(0x1B) as usize;
                    self.pool.swap(k, i);
                }
                let mut count = 0;
                for i in 0..0x1A {
                    if self.pool[i] != -1 {
                        count += 1;
                        if count > 0x10 {
                            self.pool[i] = -1;
                        }
                    }
                }
                let mut idx = 0usize;
                let mut k = 0;
                while at(&self.costumes, k) != END {
                    while self.pool_at(idx) == -1 {
                        idx = (idx + 1) % 27;
                    }
                    let c = i32::from(self.pool_at(idx));
                    self.copies[k] = super::super::pl::player::ckind_ftkind(c, 0) as i8;
                    idx += 1;
                    k += 1;
                }
            }
            0 => {
                let mut k = 0;
                while at(&self.costumes, k) != END {
                    self.copies[k] = -1;
                    k += 1;
                }
            }
            _ => {}
        }
    }

    /// `fn_8016A450`: they join metal.
    // Ports fn_8016A450; aliased once a check reaches it.
    pub fn set_metal(&mut self) {
        self.flags |= METAL;
    }

    /// `fn_8016A46C`: each joiner's `flagsAE` bit 0.
    // Ports fn_8016A46C; aliased once a check reaches it.
    pub fn set_flag_ae0(&mut self) {
        self.flags |= FLAG_AE0;
    }
}

/// A match's rules point at the joining opponents (`x54`), `gm_8016A21C`.
// Ports gm_8016A21C; aliased once a check reaches it.
pub fn point_rules(rules: &mut super::super::structs::StartMeleeRules) {
    rules.x54 = super::super::handles::RawPtr(ADDR);
}

/// `lbl_8046B488`'s address.
pub const ADDR: u32 = 0x8046_B488;

impl Game {
    /// The joining opponents' slots marked, `fn_8016A09C`: joining on, the first `at_once`
    /// empty slots flagged as joiners' (`Player_SetFlagsBit1`), every joiner's slot on team 4.
    #[doc(alias = "fn_8016A09C")]
    pub fn mark_opponent_slots(&mut self) {
        let o = &mut self.gm.opponents;
        o.flags = (o.flags | JOINING) & !BEATEN;
        let at_once = i32::from(o.at_once);
        let mut marked = 0;
        for i in 0..6 {
            if self.player_pkind(i) == PKIND_NONE {
                if marked >= at_once {
                    break;
                }
                self.set_player_flags_bit1(i);
                marked += 1;
            }
        }
        for j in 0..6 {
            if self.player_flags_bit1(j) {
                self.set_player_team(j, 4);
            }
        }
    }

    /// A joiner's costume already worn by a player of the same character, the inline
    /// `hasDuplicateCostume`.
    fn costume_worn(&self, slot: i32, costume: i32) -> bool {
        let chr = self.player_ckind(slot) as i8;
        (0..6).any(|s| {
            self.player_pkind(s) != PKIND_NONE
                && !self.player_flags_bit1(s)
                && chr == self.player_ckind(s) as i8
                && costume == self.player_costume(s) as i32
        })
    }
}

impl Running<'_> {
    /// A stage's arena brings opponents in, `gm_801674C4`: `count` of `ckind` (none other),
    /// `at_once` at a time from the stage points from `first_point`, their characters and
    /// costumes drawn as the setup's modes say (which it leaves as they were), the slots marked,
    /// and `on_end` told at the end. It sets neither the model scale nor the ratios
    /// ([Melee's bugs](../../../../docs/melee-bugs.md)).
    #[doc(alias = "gm_801674C4")]
    pub fn start_stage_event(
        &mut self,
        ckind: i8,
        count: u8,
        at_once: i8,
        first_point: i8,
        on_end: CodeRef,
    ) {
        let o = &mut self.game.gm.opponents;
        o.kinds = [ckind, NONE, NONE];
        o.left = count;
        o.total = count;
        o.at_once = at_once as u8;
        o.first_point = (i32::from(first_point) & 0xFFFF) as u8;
        o.flags |= GOES_ON;
        o.reset_costumes(usize::from(count));
        let g = &mut *self.game;
        g.gm.opponents
            .draw_characters(&g.gm.unlocks, &mut g.objs.rng);
        let p1_costume = g.player_costume(0) as i8;
        let p1_ckind = g.player_ckind(0) as i8;
        g.gm.opponents
            .shuffle_all_costumes(&mut g.objs.rng, p1_ckind, p1_costume);
        g.mark_opponent_slots();
        g.gm.opponents.set_on_end(Some(on_end));
    }

    /// A single-player match's joining opponents, `gm_8016A22C`: the setup cleared but the
    /// copies' pool, then the characters, costumes, level, counts (`at_once` at most 5), first
    /// point, flags and ratios given, the scale 1, the characters drawn, the costumes shuffled
    /// (mode 0) or one for all (mode 1), and Kirby's copies.
    // Ports gm_8016A22C; aliased once a check reaches it.
    #[allow(clippy::too_many_arguments)]
    pub fn setup_opponents(
        &mut self,
        kinds: &[i8; 3],
        colors: [i8; 3],
        costume_mode: i32,
        kind_mode: i32,
        copy_mode: u8,
        p1_ckind: u8,
        p1_costume: u8,
        cpu_level: i32,
        count: i32,
        at_once: i32,
        first_point: i32,
        goes_on: i32,
        joining: i32,
        attack: f32,
        defense: f32,
    ) {
        let g = &mut *self.game;
        let o = &mut g.gm.opponents;
        let (pool, tail) = (o.pool, o.x1db);
        *o = Opponents {
            pool,
            x1db: tail,
            ..Opponents::default()
        };
        o.kinds = *kinds;
        o.colors = colors;
        o.x1ac = p1_ckind;
        o.x1ad = NONE as u8;
        o.x1b2 = p1_costume;
        o.cpu_level = cpu_level as u8;
        o.left = count as u8;
        o.total = count as u8;
        o.at_once = at_once.min(5) as u8;
        o.first_point = first_point as u8;
        o.attack_ratio = attack;
        o.defense_ratio = defense;
        if goes_on & 1 != 0 {
            o.flags |= GOES_ON;
        }
        if joining & 1 != 0 {
            o.flags |= JOINING;
        }
        o.model_scale = 1.0;
        o.costume_mode = costume_mode as u8;
        o.kind_mode = kind_mode as u8;
        o.copy_mode = copy_mode;
        o.costume = colors[0] as u8;
        o.reset_costumes(usize::from(o.left));
        o.draw_characters(&g.gm.unlocks, &mut g.objs.rng);
        let o = &mut g.gm.opponents;
        match o.costume_mode {
            0 => {
                for k in 0..3 {
                    let c = o.kinds[k];
                    o.shuffle_costumes(&mut g.objs.rng, c, p1_ckind as i8, p1_costume as i8);
                }
            }
            1 => {
                for k in 0..3 {
                    let (c, costume) = (o.kinds[k], o.costume as i8);
                    o.one_costume(c, costume, p1_ckind as i8, p1_costume as i8);
                }
            }
            _ => {}
        }
        o.draw_copies(&g.gm.unlocks, &mut g.objs.rng);
    }

    /// The joining opponents put away, `gm_8016A164`: the match no longer a single-player
    /// one's, none left or joining, no teams, and the slots after the first emptied.
    #[doc(alias = "gm_8016A164")]
    #[doc(alias = "fn_8016B138")]
    pub fn reset_opponents(&mut self) {
        self.game.vs.state.is_singleplayer = 0;
        let o = &mut self.game.gm.opponents;
        o.total = 0;
        o.left = 0;
        o.flags &= !(JOINING | BEATEN);
        // `fn_8016B138`.
        self.game.vs.start.is_teams = 0;
        for i in 1..6 {
            self.game.set_player_pkind(i, PKIND_NONE);
            self.game.unset_player_flags_bit1(i);
        }
    }

    /// The end's callback told `arg`, `fn_80169444`: cleared when it answers 1, and whether it
    /// did.
    // Ports fn_80169444; aliased once a check reaches it.
    pub fn opponents_end(&mut self, arg: i32) -> bool {
        let Some(f) = self.game.gm.opponents.on_end else {
            return false;
        };
        if run_route_callback(self, f, arg) == 1 {
            self.game.gm.opponents.on_end = None;
            return true;
        }
        false
    }

    /// Opponents joining as slots free, `fn_8016A4C8`: each joiner's slot with no fighter
    /// takes the next of the lists (its point's position at the camera's top, a computer
    /// player of one stock, the setup's level, ratios and scale) and creates its fighters;
    /// once none is left to join and none plays, the last is beaten and the end's callback is
    /// told 1.
    // Ports fn_8016A4C8's joining; aliased once a check reaches it (every match runs its
    // early return).
    pub fn spawn_opponents(&mut self) {
        if !self.game.gm.opponents.joining() {
            return;
        }
        for slot in 0..6 {
            if !(self.game.player_flags_bit1(slot) && self.game.player_state(slot) == 0) {
                continue;
            }
            if self.game.gm.opponents.left() <= 0 {
                continue;
            }
            self.join_opponent(slot);
        }
        if self.game.gm.opponents.left == 0 {
            let g = &*self.game;
            let playing = (0..6).any(|s| {
                g.player_pkind(s) != PKIND_NONE && g.player_flags_bit1(s) && g.player_stocks(s) != 0
            });
            if !playing {
                let o = &mut self.game.gm.opponents;
                o.flags = (o.flags | BEATEN) & !JOINING;
                if let Some(f) = o.on_end
                    && run_route_callback(self, f, 1) == 1
                {
                    self.game.gm.opponents.on_end = None;
                }
            }
        }
    }

    /// The next joiner in `slot` (`fn_8016A4C8`'s loop body).
    fn join_opponent(&mut self, slot: i32) {
        {
            let o = &mut self.game.gm.opponents;
            if o.left != 0 {
                o.left -= 1;
            }
        }
        let s = &mut self.game.vs.state;
        s.unk_C = super::results_exit::add_u8_saturating(u32::from(s.unk_C), 1) as u8;
        self.game.replace_player(slot);
        let idx = self.game.gm.opponents.left();
        self.game.gm.opponents.slot_index[slot as usize] = idx as i8;
        self.game.set_player_flags_bit1(slot);
        self.game.set_player_team(slot, 4);
        let point = slot + i32::from(self.game.gm.opponents.first_point) - 1;
        let mut pos = super::super::gr::ground::point(self.game, self.data, point)
            .unwrap_or_else(|| panic!("a joiner's stage point {point} reads a stale vector"));
        pos.y = super::super::gr::stage::cam_top(self.game);
        self.game.set_player_pos(slot, pos);
        self.game.set_player_pkind(slot, PKIND_CPU);
        let i = idx as usize;
        let o = self.game.gm.opponents;
        self.game
            .set_player_ckind(slot, i32::from(at(&o.characters, i)));
        self.game.set_player_stocks(slot, 1);
        let costume = i32::from(at(&o.costumes, i));
        self.game.set_player_costume(slot, costume);
        let worn = self.game.costume_worn(slot, costume);
        self.game.set_player_sub_color(slot, i8::from(worn));
        self.game.set_player_more_flags_bit6(slot, o.more_flag6);
        self.game.set_player_more_flags_bit1(slot, 0);
        let facing = if pos.x >= 0.0 { -1.0 } else { 1.0 };
        self.game.set_player_facing(slot, facing);
        self.set_player_damage(slot, 0);
        self.game.set_player_port(slot, slot);
        self.game.set_player_rumble(slot, false);
        self.game.set_player_nametag(slot, 0x78);
        self.set_player_cpu_level(slot, i32::from(o.cpu_level));
        // The inline `roll_cpu_type`: with more than one left, a quarter of them kind 0x18.
        let cpu_type = if o.left != 1 && self.game.objs.rng.below(4) == 3 {
            0x18
        } else {
            0x17
        };
        self.set_player_cpu_type(slot, cpu_type);
        if o.flags & METAL != 0 {
            self.game.set_player_is_metal(slot, 1);
            self.set_player_cpu_type(slot, 0x1B);
        }
        self.game
            .set_player_flags_ae_bit0(slot, u8::from(o.flags & FLAG_AE0 != 0));
        self.game
            .set_player_flags_bit6(slot, u8::from(o.flags & FLAG6 != 0));
        self.game.set_player_model_scale(slot, o.model_scale);
        self.game.set_player_attack_ratio(slot, o.attack_ratio);
        self.game.set_player_defense_ratio(slot, o.defense_ratio);
        self.game
            .set_player_more_flags_bit5(slot, u8::from(o.total > 1));
        if self.game.player_ckind(slot) == CKIND_KIRBY && o.copy_mode != 0 {
            self.game.set_player_unk4d(slot, at(&o.copies, i));
            self.game.set_player_flags_ae_bit1(slot, 1);
        }
        if let Some(f) = o.on_join {
            code::call(
                self,
                f.0,
                &[
                    Val::Int(slot),
                    Val::Int(i32::from(self.game.gm.opponents.left)),
                ],
            );
        }
        self.game
            .set_player_struct_func(slot, Some(CodeRef(ON_JOINED)));
        self.player_create_fighters(slot);
        self.show_damage_display(slot);
        self.refresh_nametag(slot);
    }

    /// A joiner's fighters made, `fn_8016A488`: they read their controllers once the HUD is on.
    // Ports fn_8016A488; aliased once a check reaches it.
    pub fn opponent_joined(&mut self, slot: i32) {
        if self.game.vs.state.hud_enabled == 1 {
            self.player_enable_input(slot);
        }
    }
}

/// A single-player route's callback (`GmRouteCallback`) by address.
fn run_route_callback(r: &mut Running, f: CodeRef, arg: i32) -> i32 {
    match f.0 {
        a if a == super::super::gr::kinokoroute::ARENA_EVENT => {
            i32::from(super::super::gr::kinokoroute::arena_event(r.game, arg))
        }
        a if a == super::super::gr::icemt::END_EVENT => {
            super::super::gr::icemt::end_event(r.game, arg)
        }
        other => code::unported("a single-player route's callback", other),
    }
}
