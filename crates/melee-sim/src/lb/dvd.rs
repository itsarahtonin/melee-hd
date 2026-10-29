//! The preload cache (`melee/lb/lbdvd.c`): files a scene's mode says it will want, read from
//! the disc one at a time in the background (the highest load score first) into the
//! preload heaps ([`super::heap`]), each a block of [`super::memory`]'s, and handed to the
//! scene's loads when they ask (`lbDvd_GetPreloadedArchive`). Each scene's preparation says
//! which heaps its mode keeps (`lbDvd_80018CF4`), and what the cache should hold
//! (`lbDvd_80018254`): entries it no longer wants are let go, the rest kept.
//!
//! The cache's reads go through HAL's device command queue on the audio timeline, and their
//! callbacks run in the disc interrupt, so the cache lives with the audio library's state
//! ([`crate::audio::Audio::dvd`]), the preload heaps and their blocks with it.

use melee_data::archive::Archives;

use super::heap::{self, LbHeaps};
use super::memory::{COPY_PERIOD, CompactStep, LbMemory};
use crate::audio::Audio;
use crate::audio::devcom::cb;

/// `PreloadEntry`: a file the cache holds or wants. `state`: 0 free, 1 wanted, 2 reading,
/// 3 in, 4 in and taken. A load score below 0 marks an entry the current scene doesn't
/// want, which the next refresh lets go.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct PreloadEntry {
    pub state: i8,
    /// 0 raw bytes, 2 an archive, 3 an effect file, 4 a stage's file.
    pub kind: i8,
    pub heap: i8,
    /// 1 read and not yet set up, 2 set up (`lbDvd_GetPreloadedArchive`).
    pub load_state: i8,
    /// Which preload class asked for it (`unknown004`): 0x80 the rumble file, 0x40 the
    /// common files.
    pub class: u8,
    pub entry_num: i16,
    pub load_score: i16,
    pub size: u32,
    /// The blocks of its bytes and of its `HSD_Archive`, as `lbMemory` entries.
    pub raw_data: Option<u8>,
    pub archive: Option<u8>,
    pub effect_index: i32,
}

/// `lbDvd_803BA68C`: a free entry (`{ 0, 0, 0, 0, -1, 0, -1 }`: its class byte and entry
/// number -1).
const FREE: PreloadEntry = PreloadEntry {
    state: 0,
    kind: 0,
    heap: 0,
    load_state: 0,
    class: 0xFF,
    entry_num: -1,
    load_score: 0,
    size: 0,
    raw_data: None,
    archive: None,
    effect_index: 0,
};

impl Default for PreloadEntry {
    fn default() -> Self {
        FREE
    }
}

/// `PreloadCacheSceneEntry`: a character the scene's game cache preloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct CacheFighter {
    pub char_id: i32,
    pub color: u8,
    pub x5: u8,
}

/// `GameCache`: a mode's characters and stage the cache preloads (`lbDvdPreload_3`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct GameCache {
    pub mode_kind: u8,
    pub x1: u8,
    pub x2: u8,
    pub x3: u8,
    pub stkind: i32,
    pub entries: [CacheFighter; 8],
}

/// `PreloadedGameModeState`: whether the rumble and the common files' heaps stay, the game
/// cache, and how many times the mode's scenes changed what the cache keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct SceneCache {
    pub is_heap_persistent: [bool; 2],
    pub game_cache: GameCache,
    pub mode_scene_changes: i32,
}

/// `ChKind_None`.
const CHKIND_NONE: i32 = 0x21;
/// `preload_cache_scene`: nothing kept, no game cache (stage 0x148, every character none).
pub const SCENE_DEFAULT: SceneCache = SceneCache {
    is_heap_persistent: [false; 2],
    game_cache: GameCache {
        mode_kind: crate::game::gm::machine::GM_COUNT,
        x1: 0,
        x2: 0,
        x3: 0,
        stkind: 0x148,
        entries: [CacheFighter {
            char_id: CHKIND_NONE,
            color: 0,
            x5: 1,
        }; 8],
    },
    mode_scene_changes: 0,
};

/// `PreloadCache` (`preloadCache`).
#[derive(Clone, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct PreloadCache {
    /// The preload class the scene's mode asked for (`lbDvdPreload_0` to `_3`).
    pub persistent_heaps: i32,
    pub scene: SceneCache,
    pub new_scene: SceneCache,
    pub entries: [PreloadEntry; ENTRIES],
    /// The heap being compacted, 6 for none: no read starts meanwhile.
    pub persistent_heap: i32,
    /// Whether the mode's files are all expected preloaded (`lbDvd_80018F58`).
    pub preloaded: bool,
}

/// Entries in the cache.
pub const ENTRIES: usize = 80;

impl Default for PreloadCache {
    fn default() -> Self {
        PreloadCache {
            persistent_heaps: 0,
            scene: SCENE_DEFAULT,
            new_scene: SCENE_DEFAULT,
            entries: [FREE; ENTRIES],
            persistent_heap: 6,
            preloaded: false,
        }
    }
}

/// The preload cache with its heaps and their blocks, and the disc files' sizes it reads
/// (`DVDFastOpen`'s, from the disc's file table, which the frontend gives for the files the
/// game may preload).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Dvd {
    pub memory: LbMemory,
    pub heaps: LbHeaps,
    pub cache: PreloadCache,
    #[clone_from(if_changed)]
    pub sizes: Vec<(i32, u32)>,
    /// The disc entries of the files a game cache may preload, by name
    /// (`DVDConvertPathToEntrynum`).
    #[clone_from(if_changed)]
    pub names: Vec<(String, i32)>,
    /// The preloads' reads into main memory since the heap's image last took them up
    /// ([`crate::game::state::Running::image_preload_reads`]): each block, its length and the
    /// file's disc entry (-1 for an `HSD_Archive` block, which a parse fills). Not memory of
    /// the original's: the image's record of what the reads write there.
    #[clone_from(if_changed)]
    pub image_reads: Vec<(u32, u32, i32)>,
}

/// A file name as `lbFileGetFullName` completes it: without a leading slash, and a bare dot's
/// language extension (`usd`, English).
fn full_name(name: &str) -> String {
    let n = name.trim_start_matches('/');
    match n.strip_suffix('.') {
        Some(base) => format!("{base}.usd"),
        None => n.to_owned(),
    }
}

/// Every file a game cache can preload ([`Dvd::want_game_cache`]): each fighter kind's data,
/// costume, effect and animation files, each stage select stage's file, and Kirby's copies.
pub fn game_cache_files() -> Vec<String> {
    use crate::game::ft::data::kind_tables;
    let mut out: Vec<String> = Vec::new();
    let mut add = |name: &str| {
        let n = full_name(name);
        if !out.contains(&n) {
            out.push(n);
        }
    };
    for kind in 0..0x21 {
        let t = kind_tables(kind);
        if let Some(n) = t.dat_file {
            add(n);
        }
        for c in t.costumes.iter().take(usize::from(t.costume_count)) {
            if let Some(n) = c.file {
                add(n);
            }
        }
        if let Some(Some((n, _))) = crate::game::ef::FILES.get(usize::from(t.effect_file)) {
            add(n);
        }
        if let Some(n) = t.anim_file {
            add(n);
        }
    }
    for (n, _) in crate::game::ft::kirby::init::COPY_FILES.iter().flatten() {
        add(n);
    }
    for &k in &crate::game::ft::KIRBY_COSTUME_KINDS {
        for c in 0..6 {
            if let Some((n, _, _)) = crate::game::ft::kirby::init::copy_costume_files(k, c) {
                add(&n);
            }
        }
    }
    for e in crate::game::ft::kirby::init::COPY_EFFECTS {
        if let Some(Some((n, _))) = crate::game::ef::FILES.get(usize::from(e)) {
            add(n);
        }
    }
    for i in 0..0x1D {
        let st = crate::game::gm::opening::stage_select_kind(i);
        let gr = crate::lb::audio::ground_kind(i32::from(st)) as i32;
        for us in [true, false] {
            if let Some(n) = crate::game::gr::data::stage_file(gr, us) {
                add(n);
            }
        }
    }
    out
}

/// A file a preload class always wants: its kind, name and effect index.
struct PreloadFile {
    kind: i8,
    entry_num: i32,
    effect_index: i32,
}

impl Dvd {
    fn file_size(&self, entry_num: i32) -> u32 {
        self.sizes
            .iter()
            .find(|f| f.0 == entry_num)
            .unwrap_or_else(|| panic!("disc file {entry_num}'s size isn't known (DVDFastOpen)"))
            .1
    }

    /// `lbDvd_800178E8`: [`Dvd::want`] of the file named `name`.
    #[doc(alias = "lbDvd_800178E8")]
    #[allow(clippy::too_many_arguments)]
    fn want_named(
        &mut self,
        kind: i8,
        name: &str,
        transient: i32,
        heap: i32,
        load_state: i8,
        score: i16,
        class: u8,
        effect_index: i32,
    ) {
        let n = full_name(name);
        let entry = self
            .names
            .iter()
            .find(|e| e.0 == n)
            .unwrap_or_else(|| panic!("{n} isn't among the files a game cache preloads"))
            .1;
        self.want(
            kind,
            entry,
            transient,
            heap,
            0,
            load_state,
            score,
            class,
            effect_index,
        );
    }

    /// `efAsync_LoadAsync`: effect file `index`.
    #[doc(alias = "efAsync_LoadAsync")]
    fn want_effects(&mut self, index: u8) {
        if index >= 50 {
            return;
        }
        if let Some(Some((n, _))) = crate::game::ef::FILES.get(usize::from(index)) {
            self.want_named(3, n, 4, 4, 1, 4, 4, i32::from(index));
        }
    }

    /// `ftData_800855C8`: a fighter kind's data file, its costume `color`'s (every one's for
    /// 0xFF), its effect file and its animation file.
    #[doc(alias = "ftData_800855C8")]
    fn want_fighter(&mut self, kind: i32, color: u8) {
        let t = crate::game::ft::data::kind_tables(kind);
        let mut color = color;
        if color != 0xFF && color >= t.costume_count {
            color = 0;
        }
        if let Some(n) = t.dat_file {
            self.want_named(2, n, 4, 4, 1, 4, 2, 0);
        }
        let (lo, hi) = if color == 0xFF {
            (0, t.costume_count)
        } else {
            (color, color + 1)
        };
        for c in lo..hi {
            if let Some(n) = t.costumes[usize::from(c)].file {
                self.want_named(2, n, 4, 4, 1, 3, 1, 0);
            }
        }
        if t.effect_file != 0xFF {
            self.want_effects(t.effect_file);
        }
        if let Some(n) = t.anim_file {
            self.want_named(1, n, 5, 5, 0, 1, 8, 0);
        }
    }

    /// `ftKb_SpecialN_800EEC34` through `ftData_8008578C`: Kirby's copy of fighter kind
    /// `kind`: its file, its costumes in Kirby's colors (`color`, or every one for 0xFF), and
    /// its effect file.
    #[doc(alias = "ftData_8008578C")]
    #[doc(alias = "ftKb_SpecialN_800EEC34")]
    fn want_kirby_copy(&mut self, kind: i32, color: u8) {
        use crate::game::ft::kirby::init;
        let count = crate::game::ft::data::kind_tables(4).costume_count;
        let mut color = color;
        if color != 0xFF && color >= count {
            color = 0;
        }
        let k = kind as usize;
        if let Some(Some((n, _))) = init::COPY_FILES.get(k) {
            self.want_named(2, n, 4, 4, 1, 3, 1, 0);
        }
        if crate::game::ft::KIRBY_COSTUME_KINDS.contains(&k) {
            let (lo, hi) = if color == 0xFF {
                (0, count)
            } else {
                (color, color + 1)
            };
            for c in lo..hi {
                if let Some((n, _, _)) = init::copy_costume_files(k, usize::from(c)) {
                    self.want_named(2, &n, 4, 4, 1, 3, 1, 0);
                }
            }
        }
        if let Some(&e) = init::COPY_EFFECTS.get(k)
            && e != 0xFF
        {
            self.want_effects(e);
        }
    }

    /// `Player_80031CB0` (`Player_80031D2C` for Kirby's copies): each of a character's
    /// fighter kinds.
    #[doc(alias = "Player_80031CB0")]
    #[doc(alias = "Player_80031D2C")]
    fn want_character(&mut self, ckind: i32, color: u8, copy: bool) {
        let m = crate::game::pl::player::mapping(ckind);
        for kind in [m.internal_id, m.extra_internal_id] {
            if kind < 0 {
                continue;
            }
            if copy {
                self.want_kirby_copy(i32::from(kind), color);
            } else {
                self.want_fighter(i32::from(kind), color);
            }
        }
    }

    /// `gm_801B23F0`, Camera Mode's game cache: the snapshot's EFB copy (a 640 by 480 RGB565
    /// image), the snapshot's data and the slots' lists (`lbSnap_8001E204`,
    /// `lbSnap_8001E210`), raw buffers of the game cache's heap.
    #[doc(alias = "gm_801B23F0")]
    fn want_camera_mode(&mut self) {
        use crate::game::gm::camera_mode::{SNAP_COPY, SNAP_DATA, SNAP_SLOTS};
        let copy = crate::hsd::heap::tex_buffer_size(640, 480, 4).next_multiple_of(32);
        self.want(0, SNAP_COPY, 4, 4, copy, 0, 1, 32, 0);
        self.want(0, SNAP_DATA, 4, 4, crate::lb::snap::SNAP_SIZE, 0, 1, 32, 0);
        self.want(
            0,
            SNAP_SLOTS,
            4,
            4,
            crate::lb::snap::SLOTS_SIZE,
            0,
            1,
            32,
            0,
        );
    }

    /// `lbDvd_80017960`: the game cache's files wanted: its stage's (`Stage_802251B4`,
    /// `Ground_801C06B8`: the stage's file and its buffers), each character's, and for a Kirby
    /// every copy he can take.
    #[doc(alias = "lbDvd_80017960")]
    #[doc(alias = "Stage_802251B4")]
    #[doc(alias = "Ground_801C06B8")]
    #[doc(alias = "grIzumi_801CD2D4")]
    #[doc(alias = "grStadium_801D511C")]
    fn want_game_cache(&mut self, saved_us: bool) {
        let gc = self.cache.new_scene.game_cache;
        if gc.mode_kind == crate::game::gm::machine::GM_CAMERA_MODE {
            self.want_camera_mode();
        }
        if gc.stkind != 0x148 {
            let gr = crate::lb::audio::ground_kind(gc.stkind) as i32;
            if let Some(n) = crate::game::gr::data::stage_file(gr, saved_us) {
                self.want_named(4, n, 4, 4, 1, 7, 16, 0);
            }
            let tex = |w, h, f| crate::hsd::heap::tex_buffer_size(w, h, f).next_multiple_of(32);
            match gr {
                crate::game::gr::GR_KIND_IZUMI => {
                    self.want(0, 2001, 4, 4, tex(80, 60, 4), 0, 7, 16, 0);
                }
                crate::game::gr::GR_KIND_PSTADIUM => {
                    self.want(0, 0x7D2, 4, 4, tex(0xFA, 0xA0, 4), 0, 7, 0x10, 0);
                    self.want(0, 0x7D3, 4, 4, tex(0x280, 0x196, 4), 0, 7, 0x10, 0);
                    self.want(0, 0x7D4, 4, 4, tex(0x7C, 0x50, 4), 0, 7, 0x10, 0);
                    self.want(0, 0x7D5, 4, 4, 0x5_0000, 0, 7, 0x10, 0);
                }
                _ => {}
            }
        }
        const NONE: i32 = 0x21;
        const KIRBY: i32 = 4;
        for i in 0..8 {
            let e = gc.entries[i];
            if e.char_id != NONE {
                self.want_character(e.char_id, e.color, false);
            }
            if e.char_id == KIRBY {
                if e.x5 == 0 {
                    for kind in 0..NONE {
                        self.want_character(kind, e.color, true);
                    }
                } else {
                    for j in 0..8 {
                        let o = gc.entries[j];
                        if o.char_id == NONE {
                            continue;
                        }
                        if o.char_id == KIRBY && o.x5 == 0 {
                            for kind in 0..NONE {
                                self.want_character(kind, e.color, true);
                            }
                        }
                        self.want_character(o.char_id, e.color, true);
                    }
                }
            }
        }
    }

    /// `lbDvd_80018F68`, at boot: no class, no scene, every entry free.
    #[doc(alias = "lbDvd_80018F68")]
    pub fn init_cache(&mut self) {
        self.cache = PreloadCache::default();
    }

    /// `lbDvd_80018F58`.
    #[doc(alias = "lbDvd_80018F58")]
    pub fn set_preloaded(&mut self, value: bool) {
        self.cache.preloaded = value;
    }

    /// `lbDvd_800174E8` (`releaseEntry`): the entry's blocks back to its heap, and the entry
    /// free.
    #[doc(alias = "lbDvd_800174E8")]
    fn release(&mut self, i: usize) {
        let e = self.cache.entries[i];
        let heap = e.heap as usize;
        for b in [e.archive, e.raw_data].into_iter().flatten() {
            let addr = self.memory.entries[usize::from(b)].addr;
            self.heaps.free(heap, addr, &mut self.memory);
        }
        self.cache.entries[i] = FREE;
    }

    /// `lbDvd_80017598`: lets go every entry of `heap` but one being read, which it says to
    /// wait for (as it does a heap being compacted).
    #[doc(alias = "lbDvd_80017598")]
    pub fn release_heap(&mut self, heap: i32) -> bool {
        if self.cache.persistent_heap == heap {
            return true;
        }
        let mut busy = false;
        for i in 0..ENTRIES {
            let e = self.cache.entries[i];
            if e.state != 0 && i32::from(e.heap) == heap {
                if e.state == 2 {
                    busy = true;
                } else {
                    self.release(i);
                }
            }
        }
        busy
    }

    /// `lbDvd_80017644`: lets go of `heap`'s unwanted entries that are in and set up, unless
    /// the heap is being compacted, which it says to wait for.
    #[doc(alias = "lbDvd_80017644")]
    pub fn release_stale(&mut self, heap: i32) -> bool {
        if self.cache.persistent_heap == heap {
            return true;
        }
        for i in 0..ENTRIES {
            let e = self.cache.entries[i];
            if (e.state == 3 || e.state == 4)
                && i32::from(e.heap) == heap
                && e.load_score < 0
                && e.load_state == 2
            {
                self.release(i);
            }
        }
        false
    }

    /// `lbDvd_80017740`: the cache wants a file; an entry it has for the file in that heap is
    /// wanted again (a read one gets the top score), else a free entry takes it. The entry.
    #[doc(alias = "lbDvd_80017740")]
    #[allow(clippy::too_many_arguments)]
    pub fn want(
        &mut self,
        kind: i8,
        entry_num: i32,
        transient_heap: i32,
        heap: i32,
        size: u32,
        load_state: i8,
        load_score: i16,
        class: u8,
        effect_index: i32,
    ) -> usize {
        let mut free = None;
        for i in 0..ENTRIES {
            let e = &mut self.cache.entries[i];
            if e.state == 0 {
                free.get_or_insert(i);
            } else if i32::from(e.entry_num) == entry_num && i32::from(e.heap) == transient_heap {
                if e.state == 1 {
                    if e.load_score < 0 {
                        e.load_score = -e.load_score;
                        if e.load_score <= 0x2314 {
                            e.load_score += 10;
                        }
                    }
                } else {
                    e.load_score = 0x270F;
                }
                return i;
            }
        }
        let i = free.expect("the original's assertion failed: lbdvd.c:0x1C1");
        assert!(
            self.heaps.status(heap as usize) == heap::CREATED,
            "[LbDvd] preloading into heap {heap}, which isn't made ({entry_num})"
        );
        self.cache.entries[i] = PreloadEntry {
            state: 1,
            kind,
            heap: heap as i8,
            load_state,
            class,
            entry_num: entry_num as i16,
            load_score,
            size,
            raw_data: None,
            archive: None,
            effect_index,
        };
        i
    }

    /// `lbDvd_GetPreloadCacheScene`'s, as `lbDvd_80018CF4` sets it for a scene.
    fn set_scene_from_default(&mut self, persistent: [bool; 2], game_cache: bool) {
        let s = &mut self.cache.scene;
        if persistent[0] {
            s.is_heap_persistent[0] = SCENE_DEFAULT.is_heap_persistent[0];
        }
        if persistent[1] {
            s.is_heap_persistent[1] = SCENE_DEFAULT.is_heap_persistent[1];
        }
        if game_cache {
            s.game_cache = SCENE_DEFAULT.game_cache;
        }
    }

    /// `lbDvd_80018CF4`'s first part, a scene's preparation: the mode's preload class says
    /// which preload heaps stay (0 none, 1 `Seq`, 2 `Seq` and `Stay`, 3 all four) and resets
    /// what of the scene's cache the class doesn't keep. The heaps that go are then let go,
    /// waiting for a read in flight there ([`Dvd::release_heap`]), and the scene's heaps made
    /// ([`Dvd::make_scene_heaps`]).
    pub fn set_class(&mut self, class: i32) {
        if self.cache.persistent_heaps != class {
            self.cache.scene.mode_scene_changes = self.cache.new_scene.mode_scene_changes + 1;
        }
        self.cache.persistent_heaps = class;
        for h in [heap::SEQ, heap::STAY, heap::ALL_M, heap::ALL_A] {
            self.heaps.set_transient(h, 1);
        }
        match class {
            0 => self.set_scene_from_default([true, true], true),
            1 => {
                self.heaps.set_transient(heap::SEQ, 0);
                self.set_scene_from_default([false, true], true);
            }
            2 => {
                self.heaps.set_transient(heap::SEQ, 0);
                self.heaps.set_transient(heap::STAY, 0);
                self.set_scene_from_default([false, false], true);
            }
            3 => {
                for h in [heap::SEQ, heap::STAY, heap::ALL_M, heap::ALL_A] {
                    self.heaps.set_transient(h, 0);
                }
            }
            _ => {}
        }
    }

    /// `lbDvd_80018CF4`'s end: the scene's heaps (`lbHeap_80015900`), and the main heap's
    /// layout.
    pub fn make_scene_heaps(&mut self) -> crate::hsd::heap::Layout {
        self.heaps.make_scene_heaps(&mut self.memory)
    }

    /// `lbDvd_80018C6C`: the scene's cache keeps what the class keeps.
    #[doc(alias = "lbDvd_80018C6C")]
    pub fn keep_class(&mut self) {
        let s = &mut self.cache.scene;
        match self.cache.persistent_heaps {
            1 => s.is_heap_persistent[0] = true,
            2 => s.is_heap_persistent = [true, true],
            3 => {
                s.is_heap_persistent = [true, true];
                s.game_cache = SCENE_DEFAULT.game_cache;
            }
            _ => {}
        }
    }

    /// `lbDvd_8001823C`.
    // Ports `lbDvd_8001823C`; aliased once a check reaches it.
    pub fn note_scene_change(&mut self) {
        self.cache.scene.mode_scene_changes = self.cache.new_scene.mode_scene_changes + 1;
    }

    /// The entry whose read to start next, `lbDvd_80017CC4`'s choice: none while a heap is
    /// being compacted or a read is in flight, else the wanted entry with the highest score.
    fn next_to_read(&self) -> Option<usize> {
        if self.cache.persistent_heap != 6 {
            return None;
        }
        let mut reading = false;
        let mut best = 0;
        let mut pick = None;
        for (i, e) in self.cache.entries.iter().enumerate() {
            match e.state {
                2 => reading = true,
                1 if i32::from(e.load_score) > best => {
                    best = i32::from(e.load_score);
                    pick = Some(i);
                }
                _ => {}
            }
        }
        if reading { None } else { pick }
    }

    /// `lbDvd_800187F4`: whether the file is ready to take: 0 not in the cache, 1 not yet
    /// (wait and ask again), 2 in. Taking a read entry marks it taken, unless a wanted file
    /// still waits for room its heap's unwanted entries hold.
    #[doc(alias = "lbDvd_800187F4")]
    pub fn file_state(&mut self, entry_num: i32) -> i32 {
        for i in 0..ENTRIES {
            let e = self.cache.entries[i];
            if e.state == 0 || e.load_score <= 0 || i32::from(e.entry_num) != entry_num {
                continue;
            }
            if i32::from(e.heap) == self.cache.persistent_heap {
                return 1;
            }
            match e.state {
                1 | 2 => return 1,
                3 => {
                    if self.room_awaited(e.heap) {
                        return 1;
                    }
                    let e = &mut self.cache.entries[i];
                    e.state = 4;
                    e.load_score = 0x270F;
                    return 2;
                }
                4 => return 2,
                _ => continue,
            }
        }
        0
    }

    /// Whether a wanted file waits to be read into `heap` while unwanted entries there hold
    /// room (`lbDvd_800187F4`'s and `lbDvd_80018A2C`'s check).
    fn room_awaited(&self, heap: i8) -> bool {
        let entries = &self.cache.entries;
        let pending = entries
            .iter()
            .any(|o| o.state == 1 && o.heap == heap && o.load_score > 0);
        let stale = entries
            .iter()
            .any(|o| (o.state == 2 || o.state == 3) && o.heap == heap && o.load_score < 0);
        pending && stale
    }
}

/// `lbDvd_80017A80`, the preload cache's callback for a heap's compaction.
const HEAP_COMPACTED: u32 = 0x8001_7A80;

/// `lbDvd_80018254`'s preload classes' files: the rumble file (class 1 and up) and the common
/// files (class 2 and up), by entry number in GALE01 revision 2's file table (`LbRb.dat`;
/// `EfMnData.dat`, `EfCoData.dat`, `ItCo.usd`, `IfAll.usd`). `ItCo.` takes the disc's language
/// (`lbFileGetFullName`: English), `IfAll` the saved one: `IfAll.dat` ([`IF_ALL_JP`]) in
/// Japanese.
const RUMBLE_FILE: PreloadFile = PreloadFile {
    kind: 2,
    entry_num: 0x1EC,
    effect_index: 0,
};
/// `IfAll.dat`'s entry number.
const IF_ALL_JP: i32 = 0x1CB;
const COMMON_FILES: [PreloadFile; 4] = [
    PreloadFile {
        kind: 3,
        entry_num: 0xEF,
        effect_index: 0x1F,
    },
    PreloadFile {
        kind: 3,
        entry_num: 0xD7,
        effect_index: 0,
    },
    PreloadFile {
        kind: 2,
        entry_num: 0x1E5,
        effect_index: 0,
    },
    PreloadFile {
        kind: 2,
        entry_num: 0x1CC,
        effect_index: 0,
    },
];

impl Audio {
    fn dvd(&mut self) -> &mut Dvd {
        self.dvd.as_mut().expect("the preload cache")
    }

    /// `lbDvd_80018254`, a scene's preparation once its class is set: unless the scene's
    /// cache is what the last refresh saw, every entry is marked unwanted, the class's files
    /// wanted again, the rest of the unwanted ones not yet read let go (read ones stay until
    /// their room is needed), and the next read started.
    #[doc(alias = "lbDvd_80018254")]
    pub fn preload_refresh(&mut self, data: &Archives, saved_us: bool) {
        let d = self.dvd();
        if d.cache.new_scene == d.cache.scene {
            return;
        }
        d.cache.new_scene = d.cache.scene;
        for e in d.cache.entries.iter_mut() {
            if e.state != 0 && e.load_score > 0 {
                e.load_score = -e.load_score;
            }
        }
        let class = d.cache.persistent_heaps;
        if class >= 1 && d.cache.new_scene.is_heap_persistent[0] {
            let f = &RUMBLE_FILE;
            d.want(f.kind, f.entry_num, 2, 2, 0, 1, 9, 0x80, f.effect_index);
        }
        if class >= 2 && d.cache.new_scene.is_heap_persistent[1] {
            for f in &COMMON_FILES {
                let entry = if f.entry_num == 0x1CC && !saved_us {
                    IF_ALL_JP
                } else {
                    f.entry_num
                };
                d.want(f.kind, entry, 3, 3, 0, 1, 8, 0x40, f.effect_index);
            }
        }
        if class == 3 {
            d.want_game_cache(saved_us);
        }
        for j in 0..ENTRIES {
            let e = d.cache.entries[j];
            if e.load_score < 0 {
                if e.state == 1 {
                    d.release(j);
                } else if e.state == 4 {
                    d.cache.entries[j].state = 3;
                }
            }
        }
        self.preload_next_entry(data);
    }

    /// `lbDvd_80017CC4`: the next wanted file's read, unless one is in flight.
    #[doc(alias = "lbDvd_80017CC4")]
    pub fn preload_next_entry(&mut self, data: &Archives) {
        if let Some(i) = self.dvd().next_to_read() {
            self.cache_preloaded_file(data, i);
        }
    }

    /// `lbDvd_CachePreloadedFile`: the heap's unwanted read entries let go to make room, then
    /// the file's blocks taken from its heap and its read started at priority 2, calling back
    /// `lbDvd_80017E64`.
    #[doc(alias = "lbDvd_CachePreloadedFile")]
    fn cache_preloaded_file(&mut self, data: &Archives, index: usize) {
        let d = self.dvd();
        let heap = d.cache.entries[index].heap;
        for i in 0..ENTRIES {
            let e = d.cache.entries[i];
            if e.state == 3 && e.heap == heap && e.load_score < 0 {
                d.release(i);
            }
        }
        d.cache.persistent_heap = i32::from(heap);
        // A gap in the heap: it's compacted first, and the read waits for the callback
        // (`lbDvd_80017A80`).
        let first = d
            .heaps
            .compact(heap as usize, HEAP_COMPACTED, heap as u32, &mut d.memory);
        if let Some(b) = first {
            self.compact_step(data, Some(b));
            return;
        }
        d.cache.persistent_heap = 6;
        let e = d.cache.entries[index];
        let size = if e.size == 0 {
            d.file_size(i32::from(e.entry_num))
        } else {
            e.size
        };
        d.cache.entries[index].size = size;
        let raw = d
            .heaps
            .alloc(heap as usize, size.next_multiple_of(32), &mut d.memory);
        let archive = if matches!(e.kind, 2..=4) {
            d.heaps.alloc(heap as usize, 0x44, &mut d.memory)
        } else {
            None
        };
        let e = &mut d.cache.entries[index];
        e.raw_data = raw;
        e.archive = archive;
        let Some(raw) = raw else {
            panic!("the original's assertion failed: lbdvd.c:827 (no room to preload)");
        };
        if e.kind == 0 {
            // A buffer: in at once (`lbDvd_80017E64`), and the next read.
            e.state = 3;
            self.preload_next_entry(data);
            return;
        }
        e.state = 2;
        e.load_score = 9999;
        let entry_num = i32::from(e.entry_num);
        // `lbFile_800164A4`: the file's length from the disc's table.
        let len = d.file_size(entry_num);
        d.cache.entries[index].size = len;
        let dest = d.memory.entries[usize::from(raw)].addr;
        // Into main memory, or ARAM (the animation files' heap) through a relay buffer.
        let kind = if dest >= 0x8000_0000 { 0x21 } else { 0x23 };
        if dest >= 0x8000_0000 {
            d.image_reads
                .push((dest, len.next_multiple_of(32), entry_num));
            if let Some(a) = d.cache.entries[index].archive {
                let at = d.memory.entries[usize::from(a)].addr;
                d.image_reads.push((at, 0x44, -1));
            }
        }
        self.devcom_request(
            data,
            entry_num,
            0,
            dest,
            len.next_multiple_of(32),
            kind,
            2,
            Some(cb::PRELOADED),
            index as u32,
        );
    }

    /// `lbMemory_80015320`'s step of a heap's compaction from `block` on: the next block out
    /// of place moved (within ARAM through the device command queue, at its lowest priority;
    /// in main memory by the copy manager's alarms), each calling back here with the block
    /// after it; past the last, the compaction's callback.
    #[doc(alias = "lbMemory_80015320")]
    pub fn compact_step(&mut self, data: &Archives, block: Option<u8>) {
        match self.dvd().memory.compact_step(block) {
            CompactStep::Aram {
                src,
                dst,
                size,
                next,
            } => {
                self.devcom_request(
                    data,
                    0,
                    src,
                    dst,
                    size,
                    0x1B,
                    1,
                    Some(cb::COMPACT_STEP),
                    next,
                );
            }
            CompactStep::Ram {
                src,
                dst,
                size,
                next,
            } => {
                self.dvd().memory.copy_start(src, dst, size, next);
                self.set_copy_alarm();
            }
            CompactStep::Done { cb, arg } => {
                assert_eq!(cb, HEAP_COMPACTED, "a compaction's callback");
                self.compacted(data, arg);
            }
        }
    }

    /// `lbDvd_80017A80`, a preload heap's compaction's callback: no heap is being compacted,
    /// and the next read starts.
    #[doc(alias = "lbDvd_80017A80")]
    fn compacted(&mut self, data: &Archives, _heap: u32) {
        self.dvd().cache.persistent_heap = 6;
        self.preload_next_entry(data);
    }

    /// `fn_80015184`, the copy manager's alarm: the copy's next slice, then its callback
    /// (the compaction's next step) once it's done, else the alarm again.
    #[doc(alias = "fn_80015184")]
    pub fn copy_alarm(&mut self, data: &Archives) {
        match self.dvd().memory.copy_slice() {
            Some(next) => self.compact_step(data, crate::lb::memory::entry_at(next)),
            None => self.set_copy_alarm(),
        }
    }

    /// `OSSetAlarm` of the copy manager's alarm, [`COPY_PERIOD`] on from now: ssbm-rs stands
    /// in for the OS's alarms, and no time passes.
    fn set_copy_alarm(&mut self) {
        self.hw.copy_alarm = Some(self.hw.spent + COPY_PERIOD);
    }

    /// `lbDvd_80017E64`, a preload's read callback: the entry is in, and the next read
    /// starts.
    #[doc(alias = "lbDvd_80017E64")]
    pub fn preload_done(&mut self, data: &Archives, index: u32) {
        self.dvd().cache.entries[index as usize].state = 3;
        self.preload_next_entry(data);
    }
}

impl crate::game::state::Running<'_> {
    /// [`Audio::preload_refresh`] with the saved language the common files are named for.
    pub fn preload_refresh(&mut self) {
        let us = self.game.game_data.saved_language() == crate::game::gm::main_lib::LANG_US;
        self.game.audio.preload_refresh(self.data, us);
    }
}
